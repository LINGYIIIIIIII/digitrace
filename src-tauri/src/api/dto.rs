// ── DTOs exposed to Dart ──

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppUsageDto {
    pub app_name: String,
    pub active_seconds: i64,
    pub idle_seconds: i64,
    pub exe_path: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PageDto {
    pub title: String,
    pub seconds: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatsDto {
    pub active_seconds: i64,
    pub idle_seconds: i64,
    pub total_seconds: i64,
    pub since: Option<String>,
}

/// Raw RGBA icon pixels for rendering in Flutter.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IconDto {
    pub width: i64,
    pub height: i64,
    pub rgba: Vec<u8>,
}

/// A single day's session record (for the daily log).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DaySessionDto {
    pub app_name: String,
    pub is_idle: bool,
    pub duration_secs: i64,
    pub started_at: String,
}

/// 秒级网络样本（实时曲线缓冲返回项）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NetSampleDto {
    /// Unix 毫秒。
    pub ts: i64,
    pub down: u64,
    pub up: u64,
}

/// 日历日仪表盘：某日历日的分钟级指标点（avg）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DayMetricPointDto {
    /// 当日分钟序号 0..1439。
    pub minute: u32,
    pub avg: f64,
}

/// 日历日仪表盘：硬件/温度/网络分钟级序列（按配置时区的"日"）。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DayMetricsDto {
    pub cpu_percent: Vec<DayMetricPointDto>,
    pub mem_percent: Vec<DayMetricPointDto>,
    pub cpu_temp_c: Vec<DayMetricPointDto>,
    pub gpu_usage_percent: Vec<DayMetricPointDto>,
    pub gpu_temp_c: Vec<DayMetricPointDto>,
    pub net_down_bps: Vec<DayMetricPointDto>,
    pub net_up_bps: Vec<DayMetricPointDto>,
}

/// A day's detail: summary + sessions + diary.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DayDetailDto {
    pub date: String,
    pub active_seconds: i64,
    pub idle_seconds: i64,
    pub session_count: i64,
    pub diary: String,
    pub sessions: Vec<DaySessionDto>,
}

/// Combined dashboard payload (one FFI call instead of two).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DashboardDataDto {
    pub apps: Vec<AppUsageDto>,
    pub active_seconds: i64,
    pub idle_seconds: i64,
    pub total_seconds: i64,
    pub since: Option<String>,
}

/// User configuration (persisted in AppConfig.json).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConfigDto {
    pub poll_interval_ms: u64,
    pub idle_threshold_minutes: u64,
    /// Dashboard auto-refresh interval in seconds (generic, reusable by
    /// future features that poll local data).
    pub refresh_interval_seconds: u64,
    /// 实时页面（硬件监控 / 网络监控）的刷新间隔（秒）。
    pub live_refresh_interval_seconds: u64,
    /// 实时网络曲线的内存留存窗口（秒）：秒级样本保留时长（默认 300 = 5 分钟）。
    pub network_live_window_seconds: u64,
    pub excluded_apps: Vec<String>,
    pub db_path: String,
    /// Launch directly to the system tray (no window) — used by silent
    /// auto-start.
    pub start_minimized: bool,
    /// 界面主题：system / light / dark。
    pub theme_mode: String,
    /// 窗口材质：auto / mica / acrylic / tabbed / off（重启生效）。
    pub window_blur: String,
    /// 界面字体：system / harmonyos / noto（思源黑体）。
    pub font_family: String,
    /// 顶栏显示的徽章项：monitoring / tray / active / idle。
    pub titlebar_items: Vec<String>,
    /// 健康提醒：是否启用「连续使用电脑」提醒。
    pub health_reminder_enabled: bool,
    /// 健康提醒：连续使用多少分钟提醒一次。
    pub health_reminder_minutes: u64,
    /// 健康提醒：离开电脑多少分钟算休息（连续计时归零）。
    pub health_break_minutes: u64,
    /// 自动更新：是否自动检查新版本。
    pub update_check_enabled: bool,
    /// 自动更新：更新清单地址（JSON）。为空表示未配置更新源。
    pub update_manifest_url: String,
    /// 自动更新：GitHub 公开仓库（`所有者/仓库名`），优先于清单地址。
    pub update_github_repo: String,
    /// 自动更新：静默模式（后台下载，退出时静默替换，无弹窗）。
    pub update_silent: bool,
    /// 自动更新：固定检查时刻（0-23 点）；null=不固定。
    pub update_check_hour: Option<u32>,
    /// 托盘菜单显示的数据行：cpu / memory / network / active。
    pub tray_items: Vec<String>,
    /// 启动时显示主界面（关闭则隐藏到托盘）。
    pub launch_show_window: bool,
}

/// 网络快照（桥接给 UI）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NetworkSnapshotDto {
    pub upload_bytes_per_sec: u64,
    pub download_bytes_per_sec: u64,
    pub session_upload_bytes: u64,
    pub session_download_bytes: u64,
    pub adapter_count: i64,
}

/// 历史曲线点（分钟级 avg/max）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistoryPointDto {
    pub day: String,
    pub minute: i64,
    pub avg: f64,
    pub max: f64,
}

/// 硬件快照（CPU / 内存 / 磁盘）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiskSnapshotDto {
    pub drive: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HardwareSnapshotDto {
    pub cpu_percent: f64,
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
    pub disks: Vec<DiskSnapshotDto>,
}

/// CPU 温度快照（PawnIO 内核驱动读取 MSR，需要管理员权限）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CpuTemperatureDto {
    pub available: bool,
    pub temp_celsius: Option<f64>,
    pub package_celsius: Option<f64>,
    pub per_core: Vec<f64>,
    pub source: String,
    pub driver_installed: bool,
    pub driver_running: bool,
    pub driver_version: Option<String>,
    pub needs_admin: bool,
    pub message: Option<String>,
}

/// GPU 温度快照（NVML）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GpuTemperatureDto {
    pub name: String,
    pub temp_celsius: Option<f64>,
    pub usage_percent: Option<f64>,
}

/// 物理磁盘温度快照。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiskTemperatureDto {
    pub drive: String,
    pub model: String,
    pub temp_celsius: Option<f64>,
}

/// 物理磁盘健康快照。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiskHealthDto {
    pub name: String,
    pub status: String,
    pub media_type: String,
    pub temp_celsius: Option<f64>,
    pub wear_percent: Option<f64>,
    pub power_on_hours: Option<u64>,
    pub read_errors: Option<u64>,
    pub write_errors: Option<u64>,
}

/// 温度整体快照。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TemperatureSnapshotDto {
    pub cpu: CpuTemperatureDto,
    pub gpus: Vec<GpuTemperatureDto>,
    pub disks: Vec<DiskTemperatureDto>,
}

/// 驱动安装/卸载/提权重启等操作的结果。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DriverActionDto {
    pub ok: bool,
    pub message: String,
}

/// 按应用网络流量快照（每进程实时速率 + 会话累计）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NetAppUsageDto {
    pub app_name: String,
    pub exe_path: String,
    pub download_bps: f64,
    pub upload_bps: f64,
    pub session_download: u64,
    pub session_upload: u64,
    /// 当前活跃（ESTABLISHED）TCP 连接数。
    pub active_connections: u32,
    /// 会话累计连接数（去重）。
    pub total_connections: u64,
}

/// 按应用网络快照：是否处于字节模式 + 应用列表。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NetAppsSnapshotDto {
    /// true=字节模式（管理员运行，有真实流量）；false=连接模式（免管理员）。
    pub bytes_available: bool,
    /// true=ETW 合计模式（实时总流量，不分下载/上传；ESTATS 不可用时的兜底）。
    pub etw_mode: bool,
    pub apps: Vec<NetAppUsageDto>,
}

/// Windows 官方按应用流量（免管理员、非实时累计字节），实现见 attributed.rs。
pub use crate::attributed::AttributedUsageResult;

/// 明文导出结果（设置 → 数据 → 明文导出）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExportResultDto {
    pub ok: bool,
    pub path: Option<String>,
    pub message: Option<String>,
}
