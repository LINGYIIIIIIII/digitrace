//! # TimeTrace Core
//!
//! Shared library crate containing all business logic.
//! Used by both `timetrace-tui` (terminal) and `timetrace-gui` (desktop).

pub mod config;
pub mod contracts;
pub mod engine;
pub mod error;
pub mod etw_net;
pub mod monitor;
pub mod oplog;
pub mod security;
pub mod storage;
pub mod time_util;

pub use config::AppConfig;
pub use contracts::events::{AppInfo, EventSink, EventSource, EventSourceHandle, TrackedEvent};
pub use contracts::idle::IdleDetector;
pub use contracts::startup::{DisableResult, StartupEntryRecord, StartupScanner};
pub use contracts::storage::{
    AppMetaRecord, AppUsageSplit, AppUsageSummary, DataStore, SessionRecord,
};
pub use contracts::window::WindowResolver;
pub use engine::{
    CpuTemperature, DiskHealthInfo, DiskTemperature, DriverActionResult, GpuTemperature,
    TemperatureMonitor, TemperatureSnapshot, install_pawnio_driver, is_elevated, pawnio_status,
    query_disk_health, restart_elevated, uninstall_pawnio_driver,
};
pub use engine::{
    DiskSnapshot, HardwareMonitor, HardwareSnapshot, NetAppMonitor, NetAppUsage, SessionAggregator,
    Win32IdleDetector, Win32WindowResolver, WindowsStartupScanner, disable_autostart,
    disable_elevated_autostart, enable_autostart, enable_elevated_autostart, heal_autostart,
    is_autostart_enabled, is_elevated_autostart_enabled, run_monitor_loop,
};
pub use error::AppError;
pub use monitor::{MonitorCore, NetworkSnapshot};
pub use oplog::{clear, log_event, log_path, tail_lines};
pub use storage::SqliteStore;
