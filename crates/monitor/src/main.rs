//! 数迹 · 独立监控进程（digitrace-monitor）
//!
//! 无界面常驻进程：每秒采集硬件（CPU/内存）、温度（CPU/GPU/磁盘）、网络速率，
//! 写入共享内存 `%APPDATA%\TimeTrace\metrics.map`，供 Lite 查看器 / 完整版 /
//! 任何语言程序零拷贝读取。与完整版并发写共享内存是安全的（metrics 内部互斥）。
//!
//! 用法：
//! - `digitrace-monitor`（启动，无窗口常驻）
//! - `digitrace-monitor --stop`（通知运行中的实例退出）
//! - `\\.\pipe\DigitraceMetricsV1`（只读 JSON，一次连接返回一帧）
//!
//! 单实例：重复启动（无 `--stop`）时直接静默退出，不干扰已运行实例。

#![cfg(windows)]
#![windows_subsystem = "windows"]

mod pipe;

use std::sync::{Arc, Mutex};

use timetrace_core::{HardwareMonitor, TemperatureMonitor, WindowsCollector};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, WAIT_OBJECT_0,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, CreateMutexW, OpenEventW, SetEvent, WaitForSingleObject,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW};

/// 单实例互斥体：已存在即说明已有监控实例在跑。
const SINGLE_MUTEX: &str = r"Local\DigitraceMonitorSingle";
/// 停止事件：`--stop` 时 SetEvent，运行中的实例轮询到后退出。
const STOP_EVENT: &str = r"Local\DigitraceMonitorStop";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 取前台窗口标题作为「当前活动应用」（尽力而为，失败返回空）。
fn foreground_app() -> String {
    unsafe {
        let hwnd: HWND = GetForegroundWindow();
        if hwnd.is_null() {
            return String::new();
        }
        let mut buf = [0u16; 256];
        let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        if n <= 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buf[..n as usize])
    }
}

fn main() {
    // ── 单实例检查（互斥体句柄在进程退出时自动释放） ──
    let _single = unsafe { CreateMutexW(std::ptr::null(), 0, wide(SINGLE_MUTEX).as_ptr()) };
    let already_running = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    let args: Vec<String> = std::env::args().collect();
    let want_stop = args.iter().any(|a| a == "--stop");

    if already_running {
        if want_stop {
            // 通知运行中的实例退出。
            unsafe {
                let ev = OpenEventW(
                    0x0002, /* EVENT_MODIFY_STATE */
                    0,
                    wide(STOP_EVENT).as_ptr(),
                );
                if !ev.is_null() {
                    let _ = SetEvent(ev);
                    CloseHandle(ev);
                }
            }
        }
        // 无论 stop 还是重复启动，本实例都立即退出。
        return;
    }
    if want_stop {
        // 没有实例在跑，--stop 无意义。
        return;
    }

    // ── 停止事件（本实例监听） ──
    let stop_event: HANDLE =
        unsafe { CreateEventW(std::ptr::null(), 0, 0, wide(STOP_EVENT).as_ptr()) };

    // ── 采集职责租约 ──
    // 已有完整版或另一份 monitor 时，本进程仍保持只读服务（Named Pipe），
    // 但不再采样、不发布共享内存，也不写入 monitor.db。
    let collector_lease = metrics::CollectorLease::acquire();
    let is_collector = collector_lease.is_some();

    // ── 采集器（仅租约持有者初始化） ──
    let mut hw = collector_lease.as_ref().map(|_| HardwareMonitor::new());
    let mut temp = collector_lease.as_ref().map(|_| TemperatureMonitor::new());
    let mut net = collector_lease.as_ref().map(|_| WindowsCollector::new());

    // ── 分钟级历史落库（与完整版共用 %APPDATA%\TimeTrace\monitor.db；
    //    独立监控常驻时，硬件/温度/网络历史也能 24/7 积累，供日历日面板查询） ──
    let db_dir = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("TimeTrace");
    let mut store = is_collector.then(|| {
        timetrace_core::monitor::store::MetricStore::open(db_dir.join("monitor.db"), 90)
            .unwrap_or_else(|_| {
                timetrace_core::monitor::store::MetricStore::open(
                    std::env::temp_dir().join("digitrace_monitor.db"),
                    90,
                )
                .expect("fallback monitor db")
            })
    });
    let mut last_flush = std::time::Instant::now();
    let mut timezone = timetrace_core::AppConfig::load().timezone;
    let mut last_tz_check = std::time::Instant::now();

    let mut publisher = is_collector.then(metrics::MetricsPublisher::open).flatten();
    if is_collector && publisher.is_none() {
        return; // 采集者无法打开共享内存时不启动半只读实例，避免误写历史。
    }
    let mut reader = if is_collector {
        None
    } else {
        metrics::MetricsReader::open()
    };

    let latest = Arc::new(Mutex::new(pipe::Snapshot::default()));
    pipe::spawn(latest.clone());

    // ── 主采集循环：每秒一帧；stop 事件触发时退出 ──
    // 启动约 5 秒（各组件热身、SQLite 连接就绪）后收缩一次工作集：
    // 无界面常驻进程没有用户交互，把空闲代码页/缓存页还给系统，
    // 任务管理器里的占用（工作集）可明显下降（缺页时按需自动换回）。
    let trim_at = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut trimmed = false;
    loop {
        if !trimmed && std::time::Instant::now() >= trim_at {
            timetrace_core::mem::trim_working_set();
            trimmed = true;
        }
        let (published, gpu_power) = if let (Some(hw), Some(temp), Some(net), Some(publisher)) =
            (hw.as_mut(), temp.as_mut(), net.as_mut(), publisher.as_mut())
        {
            let hs = hw.snapshot();
            let ts = temp.snapshot();
            let ns = net.poll();
            let mem_total = hs.memory_total_bytes.max(1) as f64;
            let cpu_temp = ts.cpu.temp_celsius.unwrap_or(-1.0);
            let gpu = ts.gpus.first();
            let gpu_usage = gpu.and_then(|g| g.usage_percent).unwrap_or(-1.0);
            let gpu_temp = gpu.and_then(|g| g.temp_celsius).unwrap_or(-1.0);
            let gpu_power = gpu.and_then(|g| g.power_watts).unwrap_or(-1.0);
            let mem_percent = hs.memory_used_bytes as f64 / mem_total * 100.0;
            let mut snap = metrics::MetricsSnapshot {
                cpu_total_percent: hs.cpu_percent,
                cpu_temp_c: cpu_temp,
                gpu_usage_percent: gpu_usage,
                gpu_temp_c: gpu_temp,
                mem_used_mb: hs.memory_used_bytes as f64 / 1_048_576.0,
                mem_percent,
                net_down_bps: ns.download_bytes_per_sec as f64,
                net_up_bps: ns.upload_bytes_per_sec as f64,
                fps: -1.0, // 帧率预留，未实现
                ..metrics::MetricsSnapshot::default()
            };
            snap.set_active_app(&foreground_app());
            let published = publisher.publish(snap);
            if let Ok(mut latest) = latest.lock() {
                *latest = pipe::Snapshot {
                    metrics: published,
                    gpu_power_watts: gpu_power,
                };
            }
            (published, gpu_power)
        } else {
            // 跟随者只读共享内存；若持有者尚未发布，则周期性重开映射。
            let snapshot = reader
                .as_ref()
                .and_then(|r| r.read())
                .or_else(|| {
                    reader = metrics::MetricsReader::open();
                    reader.as_ref().and_then(|r| r.read())
                })
                .unwrap_or_default();
            if let Ok(mut latest) = latest.lock() {
                *latest = pipe::Snapshot {
                    metrics: snapshot,
                    gpu_power_watts: -1.0,
                };
            }
            (snapshot, -1.0)
        };

        // 分钟级历史：按配置时区记录；时区每 60s 重读；每 60s 落盘一次。
        if is_collector && last_tz_check.elapsed() >= std::time::Duration::from_secs(60) {
            timezone = timetrace_core::AppConfig::load().timezone;
            last_tz_check = std::time::Instant::now();
        }
        let now_fixed = timetrace_core::time_util::now_in_for(&timezone);
        if let Some(store) = store.as_mut() {
            store.record(&now_fixed, "cpu_percent", published.cpu_total_percent);
            store.record(&now_fixed, "mem_percent", published.mem_percent);
            store.record(&now_fixed, "mem_used_mb", published.mem_used_mb);
            if published.cpu_temp_c >= 0.0 {
                store.record(&now_fixed, "cpu_temp_c", published.cpu_temp_c);
            }
            if published.gpu_usage_percent >= 0.0 {
                store.record(&now_fixed, "gpu_usage_percent", published.gpu_usage_percent);
            }
            if published.gpu_temp_c >= 0.0 {
                store.record(&now_fixed, "gpu_temp_c", published.gpu_temp_c);
            }
            if gpu_power >= 0.0 {
                store.record(&now_fixed, "gpu_power_watts", gpu_power);
            }
            store.record(&now_fixed, "net_down_bps", published.net_down_bps);
            store.record(&now_fixed, "net_up_bps", published.net_up_bps);
            if last_flush.elapsed() >= std::time::Duration::from_secs(60) {
                let _ = store.flush();
                last_flush = std::time::Instant::now();
            }
        }

        // 等待 1 秒（或 stop 事件触发）。事件句柄异常时退化为固定 1s 节奏。
        let wait = unsafe { WaitForSingleObject(stop_event, 1000) };
        if wait == WAIT_OBJECT_0 {
            break;
        }
    }

    if let Some(store) = store.as_mut() {
        let _ = store.flush();
    }

    unsafe {
        if !stop_event.is_null() {
            CloseHandle(stop_event);
        }
    }
}
