//! 数迹 · 独立监控进程（digitrace-monitor）
//!
//! 无界面常驻进程：每秒采集硬件（CPU/内存）、温度（CPU/GPU/磁盘）、网络速率，
//! 写入共享内存 `%APPDATA%\TimeTrace\metrics.map`，供 Lite 查看器 / 完整版 /
//! 任何语言程序零拷贝读取。与完整版并发写共享内存是安全的（metrics 内部互斥）。
//!
//! 用法：
//! - `digitrace-monitor`（启动，无窗口常驻）
//! - `digitrace-monitor --stop`（通知运行中的实例退出）
//!
//! 单实例：重复启动（无 `--stop`）时直接静默退出，不干扰已运行实例。

#![cfg(windows)]
#![windows_subsystem = "windows"]

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

    // ── 采集器 ──
    let mut hw = HardwareMonitor::new();
    let mut temp = TemperatureMonitor::new();
    let mut net = WindowsCollector::new();

    // ── 分钟级历史落库（与完整版共用 %APPDATA%\TimeTrace\monitor.db；
    //    独立监控常驻时，硬件/温度/网络历史也能 24/7 积累，供日历日面板查询） ──
    let db_dir = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("TimeTrace");
    let mut store =
        timetrace_core::monitor::store::MetricStore::open(db_dir.join("monitor.db"), 90)
            .unwrap_or_else(|_| {
                timetrace_core::monitor::store::MetricStore::open(
                    std::env::temp_dir().join("digitrace_monitor.db"),
                    90,
                )
                .expect("fallback monitor db")
            });
    let mut last_flush = std::time::Instant::now();
    let mut timezone = timetrace_core::AppConfig::load().timezone;
    let mut last_tz_check = std::time::Instant::now();

    let Some(mut publisher) = metrics::MetricsPublisher::open() else {
        return; // 共享内存不可用（目录不可写等），静默退出。
    };

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
        let hs = hw.snapshot();
        let ts = temp.snapshot();
        let ns = net.poll();

        let mem_total = hs.memory_total_bytes.max(1) as f64;
        let cpu_temp = ts.cpu.temp_celsius.unwrap_or(-1.0);
        let gpu = ts.gpus.first();
        let gpu_usage = gpu.and_then(|g| g.usage_percent).unwrap_or(-1.0);
        let gpu_temp = gpu.and_then(|g| g.temp_celsius).unwrap_or(-1.0);
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
        publisher.publish(snap);

        // 分钟级历史：按配置时区记录；时区每 60s 重读；每 60s 落盘一次。
        if last_tz_check.elapsed() >= std::time::Duration::from_secs(60) {
            timezone = timetrace_core::AppConfig::load().timezone;
            last_tz_check = std::time::Instant::now();
        }
        let now_fixed = timetrace_core::time_util::now_in_for(&timezone);
        store.record(&now_fixed, "cpu_percent", hs.cpu_percent);
        store.record(&now_fixed, "mem_percent", mem_percent);
        store.record(
            &now_fixed,
            "mem_used_mb",
            hs.memory_used_bytes as f64 / 1_048_576.0,
        );
        if cpu_temp >= 0.0 {
            store.record(&now_fixed, "cpu_temp_c", cpu_temp);
        }
        if gpu_usage >= 0.0 {
            store.record(&now_fixed, "gpu_usage_percent", gpu_usage);
        }
        if gpu_temp >= 0.0 {
            store.record(&now_fixed, "gpu_temp_c", gpu_temp);
        }
        store.record(&now_fixed, "net_down_bps", ns.download_bytes_per_sec as f64);
        store.record(&now_fixed, "net_up_bps", ns.upload_bytes_per_sec as f64);
        if last_flush.elapsed() >= std::time::Duration::from_secs(60) {
            let _ = store.flush();
            last_flush = std::time::Instant::now();
        }

        // 等待 1 秒（或 stop 事件触发）。事件句柄异常时退化为固定 1s 节奏。
        let wait = unsafe { WaitForSingleObject(stop_event, 1000) };
        if wait == WAIT_OBJECT_0 {
            break;
        }
    }

    let _ = store.flush();

    unsafe {
        if !stop_event.is_null() {
            CloseHandle(stop_event);
        }
    }
}
