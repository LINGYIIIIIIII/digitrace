pub mod aggregator;
pub mod app_identity;
pub mod autostart;
pub mod hardware;
pub mod idle_win32;
pub mod monitor;
pub mod net_apps;
pub mod startup_win32;
pub mod temperature;
pub mod window_win32;

pub use aggregator::SessionAggregator;
pub use autostart::{
    disable_autostart, disable_elevated_autostart, enable_autostart, enable_elevated_autostart,
    heal_autostart, is_autostart_enabled, is_elevated_autostart_enabled,
};
pub use hardware::{DiskSnapshot, HardwareMonitor, HardwareSnapshot};
pub use idle_win32::Win32IdleDetector;
pub use monitor::run_monitor_loop;
pub use net_apps::{NetAppMonitor, NetAppUsage};
pub use startup_win32::WindowsStartupScanner;
pub use temperature::{
    CpuTemperature, DiskHealthInfo, DiskTemperature, DriverActionResult, GpuTemperature,
    TemperatureMonitor, TemperatureSnapshot, install_pawnio_driver, is_elevated, pawnio_status,
    query_disk_health, restart_elevated, uninstall_pawnio_driver,
};
pub use window_win32::Win32WindowResolver;
