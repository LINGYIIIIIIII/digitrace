//! ETW 内核网络事件采集（需管理员权限）：订阅 Microsoft-Windows-Kernel-Network，
//! 实时累加每个进程的 TCP/UDP 收发字节，用于 ESTATS 不可用机器上的
//! 实时按应用流量来源。
//!
//! 事件数据布局（KERNEL_NETWORK_TCPIP_V4/V6、UDPIP_V4/V6，UserData 起点，
//! 实测为 7×u32，共 28 字节）：
//!   offset 0: PID (u32)
//!   offset 4: size (u32，本包字节数)
//! 事件 ID：0x10 TCPv4 / 0x11 TCPv6 / 0x12 UDPv4 / 0x13 UDPv6。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use windows::Win32::System::Diagnostics::Etw::{
    CONTROLTRACE_HANDLE, CloseTrace, ControlTraceW, EVENT_CONTROL_CODE_ENABLE_PROVIDER,
    EVENT_RECORD, EVENT_TRACE_CONTROL_STOP, EVENT_TRACE_LOGFILEW, EVENT_TRACE_LOGFILEW_0,
    EVENT_TRACE_LOGFILEW_1, EVENT_TRACE_PROPERTIES, EVENT_TRACE_REAL_TIME_MODE, EnableTraceEx2,
    OpenTraceW, PROCESS_TRACE_MODE_EVENT_RECORD, PROCESS_TRACE_MODE_REAL_TIME, PROCESSTRACE_HANDLE,
    ProcessTrace, StartTraceW, WNODE_FLAG_TRACED_GUID,
};
use windows::core::{GUID, PCWSTR, PWSTR};

/// Microsoft-Windows-Kernel-Network 提供程序 GUID。
const KERNEL_NETWORK_GUID: GUID = GUID::from_u128(0x7DD42A49_5329_4832_8DFD_43D979153A88);
const SESSION_NAME: &str = "Digitrace-Net-Etw";
const LOG_INTERVAL_SECS: u64 = 30;
const ERROR_ALREADY_EXISTS: u32 = 183;

#[derive(Default)]
struct EtwStats {
    events: u64,
    bytes: HashMap<u32, u64>,
    /// 最近事件的 UserData 长度与开头 8 字节（调试布局用）。
    sample_len: u16,
    sample_raw: [u8; 8],
}

/// windows crate 0.61 的事件回调签名不带 Context 参数，这里用全局状态传递。
static ETW_STATS: OnceLock<Mutex<EtwStats>> = OnceLock::new();

pub struct EtwNetMonitor {
    stop: Arc<AtomicBool>,
    /// 上次速率快照（用于计算每秒增量）。
    last_snapshot: Mutex<(Instant, HashMap<u32, u64>)>,
}

unsafe extern "system" fn event_cb(record: *mut EVENT_RECORD) {
    let Some(stats) = ETW_STATS.get() else {
        return;
    };
    let rec = unsafe { &*record };
    let id = rec.EventHeader.EventDescriptor.Id;
    if !(0x10..=0x13).contains(&id) {
        return;
    }
    let len = rec.UserDataLength as usize;
    if len < 8 {
        return;
    }
    let data = unsafe { std::slice::from_raw_parts(rec.UserData.cast::<u8>(), len) };
    // 记录样本供调试（事件 ID 与原始字节）。
    if let Ok(mut s) = stats.lock() {
        s.sample_len = rec.UserDataLength;
        let n = data.len().min(8);
        s.sample_raw[..n].copy_from_slice(&data[..n]);
    }
    // 长度 >= 8 已在上方保证，直接用索引构造，避免 try_into().unwrap() 的 panic 面。
    let pid = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let size = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as u64;
    if pid == 0 {
        return;
    }
    if let Ok(mut s) = stats.lock() {
        *s.bytes.entry(pid).or_insert(0) += size;
        s.events += 1;
    }
}

unsafe extern "system" fn buffer_cb(_logfile: *mut EVENT_TRACE_LOGFILEW) -> u32 {
    1 // TRUE：继续处理
}

impl EtwNetMonitor {
    /// 启动 ETW 会话线程（非管理员会失败并记录日志，静默降级）。
    pub fn start() -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let _ = std::thread::Builder::new()
            .name("digitrace-etw-net".to_string())
            .spawn({
                let stop = stop.clone();
                move || run_session(stop)
            });
        Self {
            stop,
            last_snapshot: Mutex::new((Instant::now(), HashMap::new())),
        }
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// 每进程实时速率（B/s，合计下载+上传；两次调用间隔计算）。
    pub fn rates(&self) -> Option<HashMap<u32, f64>> {
        let stats = ETW_STATS.get()?;
        let s = stats.lock().ok()?;
        if s.events == 0 {
            return None;
        }
        let now = Instant::now();
        let mut last = self.last_snapshot.lock().unwrap();
        let dt = (now - last.0).as_secs_f64().max(0.5);
        let mut out = HashMap::new();
        for (&pid, &bytes) in &s.bytes {
            let prev = last.1.get(&pid).copied().unwrap_or(0);
            let rate = bytes.saturating_sub(prev) as f64 / dt;
            if rate > 0.0 {
                out.insert(pid, rate);
            }
        }
        last.0 = now;
        last.1 = s.bytes.clone();
        Some(out)
    }

    /// 每进程会话累计字节（本次启动内，合计）。
    pub fn session_bytes(&self) -> Option<HashMap<u32, u64>> {
        let stats = ETW_STATS.get()?;
        let s = stats.lock().ok()?;
        Some(s.bytes.clone())
    }
}

fn run_session(stop: Arc<AtomicBool>) {
    let _ = ETW_STATS.get_or_init(|| Mutex::new(EtwStats::default()));
    unsafe {
        // 会话名（结构后追加 UTF-16 字符串）。
        let name: Vec<u16> = SESSION_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let props_len = std::mem::size_of::<EVENT_TRACE_PROPERTIES>();
        let mut buf = vec![0u8; props_len + name.len() * 2];
        let props = buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;
        (*props).Wnode.BufferSize = buf.len() as u32;
        (*props).Wnode.Flags = WNODE_FLAG_TRACED_GUID;
        (*props).LogFileMode = EVENT_TRACE_REAL_TIME_MODE;
        (*props).LoggerNameOffset = props_len as u32;
        (*props).MinimumBuffers = 2;
        (*props).MaximumBuffers = 16;
        (*props).FlushTimer = 1;
        std::ptr::copy_nonoverlapping(
            name.as_ptr(),
            buf.as_mut_ptr().add(props_len).cast::<u16>(),
            name.len(),
        );

        // 清理上次可能残留的同名会话（忽略错误）。
        let _ = ControlTraceW(
            CONTROLTRACE_HANDLE::default(),
            PCWSTR(name.as_ptr()),
            props,
            EVENT_TRACE_CONTROL_STOP,
        );

        let mut session = CONTROLTRACE_HANDLE::default();
        let ret = StartTraceW(&mut session, PCWSTR(name.as_ptr()), props);
        if ret.0 != 0 && ret.0 != ERROR_ALREADY_EXISTS {
            crate::oplog::log_event("ETW", &format!("会话创建失败: err={}", ret.0));
            return;
        }
        if ret.0 == ERROR_ALREADY_EXISTS {
            // 抢占旧会话后重试一次。
            let _ = ControlTraceW(
                session,
                PCWSTR(name.as_ptr()),
                props,
                EVENT_TRACE_CONTROL_STOP,
            );
            let ret = StartTraceW(&mut session, PCWSTR(name.as_ptr()), props);
            if ret.0 != 0 {
                crate::oplog::log_event("ETW", &format!("会话重试失败: err={}", ret.0));
                return;
            }
        }

        let enable = EnableTraceEx2(
            session,
            &KERNEL_NETWORK_GUID,
            EVENT_CONTROL_CODE_ENABLE_PROVIDER.0,
            4, // TRACE_LEVEL_INFORMATION
            0xFFFF_FFFF,
            0,
            0,
            None,
        );
        if enable.0 != 0 {
            crate::oplog::log_event("ETW", &format!("启用提供程序失败: err={}", enable.0));
            let _ = ControlTraceW(
                session,
                PCWSTR(name.as_ptr()),
                props,
                EVENT_TRACE_CONTROL_STOP,
            );
            return;
        }

        let mut logfile: EVENT_TRACE_LOGFILEW = std::mem::zeroed();
        logfile.LoggerName = PWSTR(name.as_ptr() as *mut u16);
        logfile.Anonymous1 = EVENT_TRACE_LOGFILEW_0 {
            ProcessTraceMode: PROCESS_TRACE_MODE_REAL_TIME | PROCESS_TRACE_MODE_EVENT_RECORD,
        };
        logfile.BufferCallback = Some(buffer_cb);
        logfile.Anonymous2 = EVENT_TRACE_LOGFILEW_1 {
            EventRecordCallback: Some(event_cb),
        };

        let trace = OpenTraceW(&mut logfile);
        if trace == PROCESSTRACE_HANDLE::default() {
            crate::oplog::log_event("ETW", "OpenTraceW 失败");
            let _ = ControlTraceW(
                session,
                PCWSTR(name.as_ptr()),
                props,
                EVENT_TRACE_CONTROL_STOP,
            );
            return;
        }
        crate::oplog::log_event("ETW", "内核网络事件采集已启动（实时会话）");

        let proc_thread = std::thread::spawn(move || {
            let _ = ProcessTrace(&[trace], None, None);
        });

        let mut last = Instant::now();
        let mut last_events = 0u64;
        let mut last_bytes: HashMap<u32, u64> = HashMap::new();
        while !stop.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_secs(1));
            if last.elapsed().as_secs() < LOG_INTERVAL_SECS {
                continue;
            }
            last = Instant::now();
            if let Ok(s) = ETW_STATS.get().unwrap().lock() {
                let mut top: Vec<(u32, u64)> = s.bytes.iter().map(|(&k, &v)| (k, v)).collect();
                top.sort_by_key(|x| std::cmp::Reverse(x.1));
                let bytes_total: u64 = s.bytes.values().sum();
                let mut line = format!(
                    "ETW 摘要: events={} (+{}) bytes_total={} sample_len={} raw={:02X?}",
                    s.events,
                    s.events.saturating_sub(last_events),
                    bytes_total,
                    s.sample_len,
                    s.sample_raw,
                );
                for (pid, total) in top.iter().take(3) {
                    let prev = last_bytes.get(pid).copied().unwrap_or(0);
                    let rate = total.saturating_sub(prev) as f64 / LOG_INTERVAL_SECS as f64;
                    line.push_str(&format!(" pid{}={:.0}B/s", pid, rate));
                }
                last_events = s.events;
                last_bytes = top.iter().take(5).map(|&(k, v)| (k, v)).collect();
                crate::oplog::log_event("ETW", &line);
            }
        }

        let _ = ControlTraceW(
            session,
            PCWSTR(name.as_ptr()),
            props,
            EVENT_TRACE_CONTROL_STOP,
        );
        let _ = CloseTrace(trace);
        let _ = proc_thread.join();
        crate::oplog::log_event("ETW", "内核网络事件采集已停止");
    }
}

/// 供调试：当前是否有 ETW 统计可用（events > 0 即收到过内核事件）。
pub fn has_events() -> bool {
    ETW_STATS
        .get()
        .and_then(|s| s.lock().ok())
        .map(|s| s.events > 0)
        .unwrap_or(false)
}
