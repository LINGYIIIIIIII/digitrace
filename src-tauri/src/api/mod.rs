//! 数迹 Tauri 后端 API（模块化：DTO / 命令 / 按域方法实现）。
//!
//! 由原有单文件 api.rs 拆分而来：行为与数据路径保持不变。

mod commands;
mod dto;
mod impl_config;
mod impl_dashboard;
mod impl_hardware;
mod impl_network;
mod impl_system;

pub use commands::*;
pub use dto::*;

use crate::state::AppState;
use tauri::State;

// 共享依赖：子模块经 `use super::*` 直接取用。
pub use anyhow::Result;
#[cfg(windows)]
pub use std::os::windows::process::CommandExt;
pub use std::path::PathBuf;
pub use std::sync::Arc;
pub use std::time::Duration;
use timetrace_core::{
    run_monitor_loop, AppConfig, EventSink, SessionAggregator, SqliteStore, Win32IdleDetector,
    Win32WindowResolver,
};
/// Set up file logging at %APPDATA%/TimeTrace/timetrace.log
fn setup_logging() {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("TimeTrace");
    let _ = std::fs::create_dir_all(&dir);
    let log_path = dir.join("timetrace.log");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path);
    if let Ok(file) = file {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_writer(file)
            .with_ansi(false)
            .try_init();
    }
}

// ── Main API ──

pub struct TimeTraceApi {
    db: Arc<SqliteStore>,
    db_path: String,
    monitor_core: timetrace_core::MonitorCore,
    hardware: std::sync::Mutex<timetrace_core::HardwareMonitor>,
    temperature: std::sync::Mutex<timetrace_core::TemperatureMonitor>,
    net_apps: std::sync::Mutex<timetrace_core::NetAppMonitor>,
    /// 缓存的进程表（get_net_apps 每 2 秒复用，避免频繁大块分配）。
    etw_sys: std::sync::Mutex<Option<sysinfo::System>>,
    /// 共享内存实时指标发布方（供外部工具零拷贝读取）。
    metrics: Option<metrics::MetricsPublisher>,
}

impl TimeTraceApi {
    /// Create the API, opening the DB and starting the background monitor.
    pub fn create(db_path: String) -> Result<TimeTraceApi> {
        setup_logging();
        timetrace_core::oplog::install_panic_hook();
        timetrace_core::oplog::log_event("APP", "数迹后端启动");
        tracing::info!("数迹后端启动, db={}", db_path);
        // 自愈自启记录：旧版本路径迁移到当前 exe、清理遗留值名。
        timetrace_core::heal_autostart();
        let db = Arc::new(SqliteStore::open(PathBuf::from(&db_path))?);

        // Start background monitor (thread persists for process lifetime:
        // EventSourceHandle has no Drop, so not storing it is intentional).
        let config = AppConfig::load();
        let sink: Box<dyn EventSink> = Box::new(SessionAggregator::new(
            db.clone(),
            config.excluded_apps.clone(),
        ));
        let _handle = run_monitor_loop(
            Win32WindowResolver,
            Win32IdleDetector::new(),
            Duration::from_millis(config.poll_interval_ms),
            Duration::from_secs(config.idle_threshold_minutes * 60),
            sink,
        );

        // 监控子系统（网络），monitor.db 与 time.db 同目录。
        let monitor_db = PathBuf::from(&db_path)
            .parent()
            .map(|p| p.join("monitor.db"))
            .unwrap_or_else(|| PathBuf::from("monitor.db"));
        let monitor_core = timetrace_core::MonitorCore::start(monitor_db);
        let hardware = timetrace_core::HardwareMonitor::new();
        let temperature = timetrace_core::TemperatureMonitor::new();
        let net_apps = timetrace_core::NetAppMonitor::new();

        let api = TimeTraceApi {
            db,
            db_path,
            monitor_core,
            hardware: std::sync::Mutex::new(hardware),
            temperature: std::sync::Mutex::new(temperature),
            net_apps: std::sync::Mutex::new(net_apps),
            etw_sys: std::sync::Mutex::new(None),
            metrics: metrics::MetricsPublisher::open(),
        };
        Ok(api)
    }
}

pub(crate) fn lock<'a>(state: &'a State<'a, AppState>) -> std::sync::MutexGuard<'a, TimeTraceApi> {
    state.api.lock().unwrap_or_else(|p| p.into_inner())
}

pub(crate) fn parse_date(s: &str) -> chrono::NaiveDate {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .unwrap_or_else(|_| chrono::Local::now().date_naive())
}

/// Extract a clean, env-expanded exe path from a startup command line.
/// Handles: quoted paths, trailing args, %VAR% env vars, double backslashes.
pub(crate) fn clean_exe_path(cmd: &str) -> Option<String> {
    let lower = cmd.to_lowercase();
    let idx = lower.find(".exe").or_else(|| lower.find(".lnk"))?;
    // ".exe" 与 ".lnk" 均为 4 字符。
    let end = idx + 4;
    if end > cmd.len() {
        return None;
    }
    let before = &cmd[..end];
    // The exe path itself may contain spaces (e.g. "C:\\Program Files\\...").
    // Only a quoted command lets us trim leading tokens; otherwise the whole
    // prefix up to ".exe" IS the path (arguments can only follow ".exe").
    let start = before.rfind('"').map(|q| q + 1).unwrap_or(0);
    if start >= end {
        return None;
    }
    let raw = &cmd[start..end];

    // Normalize double backslashes from registry escaping: \\ → \
    // (only when the path otherwise parses — a single backslash stays)
    let raw = raw.replace("\\\\", "\\");

    // Expand %VAR% using process environment (windir, SystemRoot, etc.)
    let mut expanded = raw.to_string();
    for (k, v) in std::env::vars() {
        expanded = expanded.replace(&format!("%{}%", k), &v);
    }
    // Fallback for common vars if somehow not in env
    let common = [
        ("windir", "C:\\Windows"),
        ("SystemRoot", "C:\\Windows"),
        ("ProgramFiles", "C:\\Program Files"),
        ("ProgramFiles(x86)", "C:\\Program Files (x86)"),
        ("SystemDrive", "C:"),
    ];
    for (k, v) in common {
        expanded = expanded.replace(&format!("%{}%", k), v);
    }

    if expanded.contains("%") {
        return None; // unresolved env var — can't iconify
    }
    Some(expanded)
}

/// Quote a CSV field when it contains a comma, quote, or newline.
pub(crate) fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
#[cfg(test)]
mod tests {
    use crate::api::{clean_exe_path, csv_escape};

    #[test]
    fn spaced_unquoted_path_kept_intact() {
        let p = clean_exe_path(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe")
            .unwrap();
        assert_eq!(
            p,
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"
        );
    }

    #[test]
    fn quoted_path_strips_quotes() {
        let p = clean_exe_path(r#""C:\Program Files\App\app.exe" --flag"#).unwrap();
        assert_eq!(p, r"C:\Program Files\App\app.exe");
    }

    #[test]
    fn no_space_path_unchanged() {
        let p = clean_exe_path(r"C:\Tools\App\app.exe").unwrap();
        assert_eq!(p, r"C:\Tools\App\app.exe");
    }

    #[test]
    fn csv_escape_quotes_commas() {
        assert_eq!(csv_escape("plain"), "plain");
        assert_eq!(csv_escape("a,b"), r#""a,b""#);
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
    }
}
