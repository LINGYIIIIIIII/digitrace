//! 数迹 Tauri 后端 API。
//!
//! 由原有 Flutter bridge（bridge/src/api.rs）机械转换而来：
//! DTO 补 serde、方法去 #[frb] 并追加 #[tauri::command] 包装。
//! 逻辑与数据路径保持不变。

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use timetrace_core::*;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

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
}

impl TimeTraceApi {
    /// Create the API, opening the DB and starting the background monitor.
    pub fn create(db_path: String) -> Result<TimeTraceApi> {
        setup_logging();
        timetrace_core::oplog::install_panic_hook();
        timetrace_core::oplog::log_event("APP", "TimeTrace bridge 启动");
        tracing::info!("TimeTrace bridge starting, db={}", db_path);
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
        };
        Ok(api)
    }

    /// One-call dashboard payload: usage split + overall stats.
    pub fn get_dashboard_data(&self, start: String, end: String) -> DashboardDataDto {
        let s = parse_date(&start);
        let e = parse_date(&end);
        let split = DataStore::get_usage_split(&*self.db, s, e);
        let active: i64 = split.iter().map(|x| x.active_seconds).sum();
        let idle: i64 = split.iter().map(|x| x.idle_seconds).sum();
        DashboardDataDto {
            apps: split
                .into_iter()
                .map(|x| AppUsageDto {
                    app_name: x.app_name,
                    active_seconds: x.active_seconds,
                    idle_seconds: x.idle_seconds,
                    exe_path: x.exe_path,
                })
                .collect(),
            active_seconds: active,
            idle_seconds: idle,
            total_seconds: DataStore::total_tracked_seconds(&*self.db),
            since: DataStore::recording_started_at(&*self.db)
                .map(|t| t.format("%Y-%m-%d").to_string()),
        }
    }

    /// Per-app active/idle split for a date range (dates as "YYYY-MM-DD").
    pub fn get_usage_split(&self, start: String, end: String) -> Vec<AppUsageDto> {
        let s = parse_date(&start);
        let e = parse_date(&end);
        DataStore::get_usage_split(&*self.db, s, e)
            .into_iter()
            .map(|x| AppUsageDto {
                app_name: x.app_name,
                active_seconds: x.active_seconds,
                idle_seconds: x.idle_seconds,
                exe_path: x.exe_path,
            })
            .collect()
    }

    /// Page-level breakdown for an app on a date.
    pub fn get_window_titles(&self, app_name: String, date: String) -> Vec<PageDto> {
        DataStore::get_window_titles(&*self.db, &app_name, parse_date(&date))
            .into_iter()
            .map(|(title, seconds)| PageDto { title, seconds })
            .collect()
    }

    /// Overall recording statistics.
    pub fn get_stats(&self, start: String, end: String) -> StatsDto {
        let s = parse_date(&start);
        let e = parse_date(&end);
        let split = DataStore::get_usage_split(&*self.db, s, e);
        let active: i64 = split.iter().map(|x| x.active_seconds).sum();
        let idle: i64 = split.iter().map(|x| x.idle_seconds).sum();
        StatsDto {
            active_seconds: active,
            idle_seconds: idle,
            total_seconds: DataStore::total_tracked_seconds(&*self.db),
            since: DataStore::recording_started_at(&*self.db)
                .map(|t| t.format("%Y-%m-%d").to_string()),
        }
    }

    /// Extract an exe icon as raw RGBA pixels.
    pub fn get_app_icon(&self, exe_path: String) -> Option<IconDto> {
        let cleaned = clean_exe_path(&exe_path).unwrap_or_else(|| exe_path.clone());
        crate::icons::extract_icon_rgba(&cleaned).map(|(w, h, rgba)| IconDto {
            width: w as i64,
            height: h as i64,
            rgba,
        })
    }

    /// Read the current user configuration.
    pub fn get_config(&self) -> ConfigDto {
        let config = AppConfig::load();
        ConfigDto {
            poll_interval_ms: config.poll_interval_ms,
            idle_threshold_minutes: config.idle_threshold_minutes,
            refresh_interval_seconds: config.refresh_interval_seconds,
            live_refresh_interval_seconds: config.live_refresh_interval_seconds,
            excluded_apps: config.excluded_apps.clone(),
            db_path: self.db_path.clone(),
            start_minimized: config.start_minimized,
            theme_mode: config.theme_mode,
            window_blur: config.window_blur,
            font_family: config.font_family,
            titlebar_items: config.titlebar_items.clone(),
            health_reminder_enabled: config.health_reminder_enabled,
            health_reminder_minutes: config.health_reminder_minutes,
            health_break_minutes: config.health_break_minutes,
            update_check_enabled: config.update_check_enabled,
            update_manifest_url: config.update_manifest_url,
            update_github_repo: config.update_github_repo,
            update_silent: config.update_silent,
            update_check_hour: config.update_check_hour,
            tray_items: config.tray_items.clone(),
            launch_show_window: config.launch_show_window,
        }
    }

    /// Persist user configuration (applies on next monitor start).
    pub fn set_config(&self, config: ConfigDto) -> Result<()> {
        let mut app_config = AppConfig::load();
        // 输入钳制：防止前端传 0 / 负值导致监控循环空转或轮询风暴。
        app_config.poll_interval_ms = config.poll_interval_ms.clamp(500, 60_000);
        app_config.idle_threshold_minutes = config.idle_threshold_minutes.clamp(1, 1440);
        app_config.refresh_interval_seconds = config.refresh_interval_seconds.clamp(1, 3600);
        app_config.live_refresh_interval_seconds =
            config.live_refresh_interval_seconds.clamp(1, 3600);
        app_config.health_reminder_minutes = config.health_reminder_minutes.clamp(5, 1440);
        app_config.health_break_minutes = config.health_break_minutes.clamp(1, 1440);
        app_config.excluded_apps = config.excluded_apps;
        app_config.start_minimized = config.start_minimized;
        app_config.theme_mode = config.theme_mode;
        app_config.window_blur = config.window_blur;
        app_config.font_family = config.font_family;
        app_config.titlebar_items = config.titlebar_items;
        app_config.health_reminder_enabled = config.health_reminder_enabled;
        app_config.update_check_enabled = config.update_check_enabled;
        let repo = config.update_github_repo.trim();
        if !repo.is_empty() && !repo.contains('/') {
            return Err(anyhow::anyhow!(
                "GitHub 仓库格式应为「所有者/仓库名」，如 LINGYIIIIIIII/digitrace"
            ));
        }
        app_config.update_github_repo = repo.to_string();
        app_config.update_silent = config.update_silent;
        if let Some(hour) = config.update_check_hour {
            if hour > 23 {
                return Err(anyhow::anyhow!("检查时刻必须在 0-23 之间"));
            }
            app_config.update_check_hour = Some(hour);
        } else {
            app_config.update_check_hour = None;
        }
        let url = config.update_manifest_url.trim();
        if !url.is_empty() && !url.starts_with("https://") {
            return Err(anyhow::anyhow!("更新地址必须使用 HTTPS（安全要求）"));
        }
        app_config.update_manifest_url = url.to_string();
        app_config.tray_items = config.tray_items;
        app_config.launch_show_window = config.launch_show_window;
        app_config
            .save()
            .map_err(|e| anyhow::anyhow!(e.to_string()))
    }

    /// 网络实时快照。
    pub fn get_network_snapshot(&self) -> NetworkSnapshotDto {
        let s = self.monitor_core.network_snapshot();
        NetworkSnapshotDto {
            upload_bytes_per_sec: s.upload_bytes_per_sec,
            download_bytes_per_sec: s.download_bytes_per_sec,
            session_upload_bytes: s.session_upload_bytes,
            session_download_bytes: s.session_download_bytes,
            adapter_count: s.adapters.len() as i64,
        }
    }

    /// 按应用网络快照（字节模式或连接模式，自动探测）。
    pub fn get_net_apps(&self) -> NetAppsSnapshotDto {
        let mut monitor = self.net_apps.lock().unwrap_or_else(|p| p.into_inner());
        let snap = monitor.snapshot();

        // ETW 内核网络事件：管理员下提供实时按应用流量（合计，不分方向）。
        // 在 ESTATS 不可用的机器上作为字节来源。
        if let Some(rates) = self.monitor_core.etw_rates() {
            let session = self.monitor_core.etw_session_bytes().unwrap_or_default();
            let mut sys_guard = self.etw_sys.lock().unwrap_or_else(|p| p.into_inner());
            let sys = sys_guard.get_or_insert_with(sysinfo::System::new);
            sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
            let mut apps: Vec<NetAppUsageDto> = Vec::new();
            for (pid, rate) in rates {
                let mut app_name = String::new();
                let mut exe_path = String::new();
                if let Some(p) = sys.process(sysinfo::Pid::from_u32(pid)) {
                    app_name = p.name().to_string_lossy().to_string();
                    exe_path = p
                        .exe()
                        .map(|e| e.to_string_lossy().to_string())
                        .unwrap_or_default();
                }
                if app_name.is_empty() {
                    continue;
                }
                // 补充连接数（按应用名匹配连接模式的结果）。
                let (active, total) = snap
                    .apps
                    .iter()
                    .find(|a| a.app_name == app_name)
                    .map(|a| (a.active_connections, a.total_connections))
                    .unwrap_or((0, 0));
                apps.push(NetAppUsageDto {
                    app_name,
                    exe_path,
                    download_bps: rate,
                    upload_bps: 0.0,
                    session_download: session.get(&pid).copied().unwrap_or(0),
                    session_upload: 0,
                    active_connections: active,
                    total_connections: total,
                });
            }
            apps.sort_by(|a, b| {
                b.download_bps
                    .partial_cmp(&a.download_bps)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            return NetAppsSnapshotDto {
                bytes_available: true,
                etw_mode: true,
                apps,
            };
        }

        NetAppsSnapshotDto {
            bytes_available: snap.bytes_available,
            etw_mode: false,
            apps: snap
                .apps
                .into_iter()
                .map(|u| NetAppUsageDto {
                    app_name: u.app_name,
                    exe_path: u.exe_path,
                    download_bps: u.download_bps,
                    upload_bps: u.upload_bps,
                    session_download: u.session_download,
                    session_upload: u.session_upload,
                    active_connections: u.active_connections,
                    total_connections: u.total_connections,
                })
                .collect(),
        }
    }

    /// 硬件快照（CPU / 内存 / 磁盘）。
    pub fn get_hardware_snapshot(&self) -> HardwareSnapshotDto {
        let snap = self
            .hardware
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .snapshot();
        HardwareSnapshotDto {
            cpu_percent: snap.cpu_percent,
            memory_total_bytes: snap.memory_total_bytes,
            memory_used_bytes: snap.memory_used_bytes,
            disks: snap
                .disks
                .into_iter()
                .map(|d| DiskSnapshotDto {
                    drive: d.drive,
                    total_bytes: d.total_bytes,
                    available_bytes: d.available_bytes,
                })
                .collect(),
        }
    }

    /// 温度快照（CPU / GPU / 磁盘）。
    pub fn get_temperature_snapshot(&self) -> TemperatureSnapshotDto {
        let snap = self
            .temperature
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .snapshot();
        TemperatureSnapshotDto {
            cpu: CpuTemperatureDto {
                available: snap.cpu.available,
                temp_celsius: snap.cpu.temp_celsius,
                package_celsius: snap.cpu.package_celsius,
                per_core: snap.cpu.per_core,
                source: snap.cpu.source,
                driver_installed: snap.cpu.driver_installed,
                driver_running: snap.cpu.driver_running,
                driver_version: snap.cpu.driver_version,
                needs_admin: snap.cpu.needs_admin,
                message: snap.cpu.message,
            },
            gpus: snap
                .gpus
                .into_iter()
                .map(|g| GpuTemperatureDto {
                    name: g.name,
                    temp_celsius: g.temp_celsius,
                    usage_percent: g.usage_percent,
                })
                .collect(),
            disks: snap
                .disks
                .into_iter()
                .map(|d| DiskTemperatureDto {
                    drive: d.drive,
                    model: d.model,
                    temp_celsius: d.temp_celsius,
                })
                .collect(),
        }
    }

    /// 磁盘健康快照（状态 / 温度 / 磨损 / 通电时长 / 读写错误）。
    /// `force=true` 跳过 24 小时缓存强制刷新（手动刷新按钮）。
    pub fn get_disk_health(&self, force: bool) -> Vec<DiskHealthDto> {
        timetrace_core::query_disk_health(force)
            .into_iter()
            .map(|d| DiskHealthDto {
                name: d.name,
                status: d.status,
                media_type: d.media_type,
                temp_celsius: d.temp_celsius,
                wear_percent: d.wear_percent,
                power_on_hours: d.power_on_hours,
                read_errors: d.read_errors,
                write_errors: d.write_errors,
            })
            .collect()
    }

    /// 网络历史曲线（最近 N 天，分钟级）。
    pub fn get_network_history(&self, mode: &str) -> Vec<HistoryPointDto> {
        self.monitor_core
            .network_history(mode)
            .into_iter()
            .map(|s| HistoryPointDto {
                day: s.day,
                minute: s.minute as i64,
                avg: s.avg,
                max: s.max,
            })
            .collect()
    }

    /// 上传方向历史（分钟级 avg/max）。
    pub fn get_network_history_up(&self, mode: &str) -> Vec<HistoryPointDto> {
        self.monitor_core
            .network_history_up(mode)
            .into_iter()
            .map(|s| HistoryPointDto {
                day: s.day,
                minute: s.minute as i64,
                avg: s.avg,
                max: s.max,
            })
            .collect()
    }

    /// 运行日志文件路径。
    pub fn get_log_path(&self) -> String {
        timetrace_core::oplog::log_path().display().to_string()
    }

    /// Whether TimeTrace is registered to auto-start at logon
    /// （HKCU Run 或管理员计划任务，任一存在即视为已开启）。
    pub fn is_auto_start(&self) -> bool {
        is_autostart_enabled() || is_elevated_autostart_enabled()
    }

    /// 是否处于「管理员权限自启」模式（计划任务已注册）。
    pub fn is_elevated_auto_start(&self) -> bool {
        is_elevated_autostart_enabled()
    }

    /// Enable/disable silent auto-start: registers (or removes) the HKCU
    /// Run entry / 管理员计划任务，并翻转 `start_minimized`。
    pub fn set_auto_start(&self, enabled: bool) -> Result<()> {
        let mut config = AppConfig::load();
        if enabled {
            if config.autostart_elevated || is_elevated_autostart_enabled() {
                // 管理员模式或计划任务已存在：只保留最高权限任务（首次弹一次 UAC），
                // 移除注册表项避免开机重复启动两个实例。
                if !is_elevated_autostart_enabled() {
                    enable_elevated_autostart().map_err(anyhow::Error::msg)?;
                }
                disable_autostart().ok();
            } else {
                enable_autostart().map_err(anyhow::Error::msg)?;
            }
            config.start_minimized = true;
        } else {
            disable_autostart().map_err(anyhow::Error::msg)?;
            // 管理员计划任务一并清理（若存在会弹一次 UAC）。
            disable_elevated_autostart().ok();
            config.start_minimized = false;
            config.autostart_elevated = false;
        }
        config.save().map_err(|e| anyhow::anyhow!(e.to_string()))
    }

    /// 切换「开机自启以管理员权限运行」。
    /// 开启：注册最高权限计划任务（弹一次 UAC）、移除注册表项，开机静默提权；
    /// 关闭：删除计划任务；若之前处于自启状态则恢复普通注册表自启。
    pub fn set_elevated_auto_start(&self, enabled: bool) -> Result<()> {
        let mut config = AppConfig::load();
        if enabled {
            enable_elevated_autostart().map_err(anyhow::Error::msg)?;
            disable_autostart().ok();
            config.autostart_elevated = true;
            config.start_minimized = true;
        } else {
            disable_elevated_autostart().map_err(anyhow::Error::msg)?;
            if config.start_minimized {
                enable_autostart().map_err(anyhow::Error::msg)?;
            }
            config.autostart_elevated = false;
        }
        config.save().map_err(|e| anyhow::anyhow!(e.to_string()))
    }

    /// Active seconds for this week (Mon→today) and last week (full).
    pub fn get_week_totals(&self) -> (i64, i64) {
        let today = chrono::Local::now().date_naive();
        let weekday = chrono::Datelike::weekday(&today).num_days_from_monday() as i64;
        let this_monday = today - chrono::Duration::days(weekday);
        let last_monday = this_monday - chrono::Duration::days(7);
        let this_week = DataStore::total_tracked_in_range(&*self.db, this_monday, today);
        let last_week = DataStore::total_tracked_in_range(
            &*self.db,
            last_monday,
            this_monday - chrono::Duration::days(1),
        );
        (this_week, last_week)
    }

    /// Full day detail: active/idle totals, session timeline, diary.
    pub fn get_day_detail(&self, date: String) -> DayDetailDto {
        let d = parse_date(&date);
        let sessions = DataStore::get_day_sessions(&*self.db, d);
        let mut active = 0i64;
        let mut idle = 0i64;
        let mut dtos = Vec::with_capacity(sessions.len());
        for (app, is_idle, dur, started) in sessions {
            if is_idle {
                idle += dur;
            } else {
                active += dur;
            }
            dtos.push(DaySessionDto {
                app_name: app,
                is_idle,
                duration_secs: dur,
                started_at: started,
            });
        }
        DayDetailDto {
            date,
            active_seconds: active,
            idle_seconds: idle,
            session_count: dtos.len() as i64,
            diary: DataStore::get_diary(&*self.db, d).unwrap_or_default(),
            sessions: dtos,
        }
    }

    /// Hourly active-seconds for a day (24 buckets) — for the heatmap.
    pub fn get_day_hourly(&self, date: String) -> Vec<i64> {
        DataStore::get_day_hourly(&*self.db, parse_date(&date))
    }

    /// 全年每日活跃秒数（热力图数据，按天聚合，轻量查询）。
    pub fn get_year_heatmap(&self, year: i32) -> Vec<(String, i64)> {
        let start = chrono::NaiveDate::from_ymd_opt(year, 1, 1)
            .unwrap_or_else(|| chrono::Local::now().date_naive());
        let end = chrono::NaiveDate::from_ymd_opt(year, 12, 31).unwrap_or(start);
        DataStore::get_active_by_date(&*self.db, start, end)
    }

    /// Apps active within a specific hour of a date (seconds per app).
    pub fn get_hour_apps(&self, date: String, hour: u32) -> Vec<AppUsageDto> {
        DataStore::get_hour_apps(&*self.db, parse_date(&date), hour)
            .into_iter()
            .map(|(app_name, secs)| AppUsageDto {
                app_name,
                active_seconds: secs,
                idle_seconds: 0,
                exe_path: String::new(),
            })
            .collect()
    }

    /// Hourly active-seconds for one app on a date (24 buckets).
    pub fn get_app_hourly(&self, app_name: String, date: String) -> Vec<i64> {
        DataStore::get_app_hourly(&*self.db, &app_name, parse_date(&date))
    }

    /// Clear ALL tracked usage data (sessions + page visits).
    pub fn clear_data(&self) {
        tracing::info!("Clearing all usage data");
        DataStore::clear_all_data(&*self.db);
    }

    /// Export usage data for a date range as CSV.
    /// Returns the CSV text (app, date, active_secs, idle_secs).
    pub fn export_csv(&self, start: String, end: String) -> String {
        let s = parse_date(&start);
        let e = parse_date(&end);
        let rows = DataStore::export_rows(&*self.db, s, e);
        let mut csv = String::from("app,date,active_secs,idle_secs\n");
        for (app, date, active, idle) in rows {
            csv.push_str(&format!(
                "{},{},{},{}\n",
                csv_escape(&app),
                csv_escape(&date),
                active,
                idle
            ));
        }
        csv
    }

    /// 导出敏感数据明文（日记 + 窗口标题）到 export 目录，供用户自行备份。
    /// 文件含明文内容，仅在本机生成，位置会提示用户保管。
    pub fn export_plaintext(&self) -> ExportResultDto {
        let dir = dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("TimeTrace")
            .join("export");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return ExportResultDto {
                ok: false,
                path: None,
                message: Some(format!("无法创建导出目录：{e}")),
            };
        }
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let path = dir.join(format!("数迹明文导出-{stamp}.json"));
        let payload = serde_json::json!({
            "exported_at": chrono::Local::now().to_rfc3339(),
            "app": "数迹",
            "note": "此文件为明文备份，包含日记与窗口标题，请妥善保管。",
            "data": self.db.dump_sensitive_plaintext(),
        });
        match serde_json::to_string_pretty(&payload) {
            Ok(json) => match std::fs::write(&path, json) {
                Ok(_) => ExportResultDto {
                    ok: true,
                    path: Some(path.to_string_lossy().to_string()),
                    message: None,
                },
                Err(e) => ExportResultDto {
                    ok: false,
                    path: None,
                    message: Some(format!("写入导出文件失败：{e}")),
                },
            },
            Err(e) => ExportResultDto {
                ok: false,
                path: None,
                message: Some(format!("生成导出内容失败：{e}")),
            },
        }
    }
}

// ── Tauri commands ──

use crate::state::AppState;
use tauri::State;

fn lock<'a>(state: &'a State<'a, AppState>) -> std::sync::MutexGuard<'a, TimeTraceApi> {
    state.api.lock().unwrap_or_else(|p| p.into_inner())
}

#[tauri::command]
pub fn get_dashboard_data(
    state: State<'_, AppState>,
    start: String,
    end: String,
) -> DashboardDataDto {
    lock(&state).get_dashboard_data(start, end)
}

#[tauri::command]
pub fn get_usage_split(state: State<'_, AppState>, start: String, end: String) -> Vec<AppUsageDto> {
    lock(&state).get_usage_split(start, end)
}

#[tauri::command]
pub fn get_window_titles(
    state: State<'_, AppState>,
    app_name: String,
    date: String,
) -> Vec<PageDto> {
    lock(&state).get_window_titles(app_name, date)
}

#[tauri::command]
pub fn get_stats(state: State<'_, AppState>, start: String, end: String) -> StatsDto {
    lock(&state).get_stats(start, end)
}

#[tauri::command]
pub fn get_app_icon(state: State<'_, AppState>, exe_path: String) -> Option<IconDto> {
    lock(&state).get_app_icon(exe_path)
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> ConfigDto {
    lock(&state).get_config()
}

#[tauri::command]
pub fn set_config(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    config: ConfigDto,
) -> Result<(), String> {
    lock(&state).set_config(config).map_err(|e| e.to_string())?;
    // 托盘数据行配置可能变化，保存后立即重建托盘菜单。
    let _ = crate::tray::rebuild(&app);
    Ok(())
}

#[tauri::command]
pub fn get_network_snapshot(state: State<'_, AppState>) -> NetworkSnapshotDto {
    lock(&state).get_network_snapshot()
}

#[tauri::command]
pub fn get_net_apps(state: State<'_, AppState>) -> NetAppsSnapshotDto {
    lock(&state).get_net_apps()
}

#[tauri::command]
pub fn get_network_history(state: State<'_, AppState>, mode: String) -> Vec<HistoryPointDto> {
    lock(&state).get_network_history(&mode)
}

#[tauri::command]
pub fn get_network_history_up(state: State<'_, AppState>, mode: String) -> Vec<HistoryPointDto> {
    lock(&state).get_network_history_up(&mode)
}

#[tauri::command]
pub fn get_hardware_snapshot(state: State<'_, AppState>) -> HardwareSnapshotDto {
    lock(&state).get_hardware_snapshot()
}

#[tauri::command]
pub fn get_temperature_snapshot(state: State<'_, AppState>) -> TemperatureSnapshotDto {
    lock(&state).get_temperature_snapshot()
}

#[tauri::command]
pub fn get_disk_health(state: State<'_, AppState>, force: bool) -> Vec<DiskHealthDto> {
    lock(&state).get_disk_health(force)
}

/// 可选安装 PawnIO 内核驱动（弹 UAC，安装动作写入运行日志）。
#[tauri::command]
pub fn install_pawnio_driver() -> DriverActionDto {
    let r = timetrace_core::install_pawnio_driver();
    DriverActionDto {
        ok: r.ok,
        message: r.message,
    }
}

/// 卸载 PawnIO 内核驱动（弹 UAC，卸载动作写入运行日志）。
#[tauri::command]
pub fn uninstall_pawnio_driver() -> DriverActionDto {
    let r = timetrace_core::uninstall_pawnio_driver();
    DriverActionDto {
        ok: r.ok,
        message: r.message,
    }
}

/// 以管理员身份重新启动数迹（读取 CPU 温度需要）。
#[tauri::command]
pub fn restart_elevated(app: tauri::AppHandle) -> DriverActionDto {
    let r = timetrace_core::restart_elevated();
    if r.ok {
        // 延迟重启器会在 1.5 秒后以管理员身份拉起新实例；
        // 当前进程立即干净退出，释放单实例锁，避免新实例被拦截。
        app.exit(0);
    }
    DriverActionDto {
        ok: r.ok,
        message: r.message,
    }
}

#[tauri::command]
pub fn get_log_path(state: State<'_, AppState>) -> String {
    lock(&state).get_log_path()
}

#[tauri::command]
pub fn is_auto_start(state: State<'_, AppState>) -> bool {
    lock(&state).is_auto_start()
}

#[tauri::command]
pub fn set_auto_start(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    lock(&state)
        .set_auto_start(enabled)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn is_elevated_auto_start(state: State<'_, AppState>) -> bool {
    lock(&state).is_elevated_auto_start()
}

#[tauri::command]
pub fn set_elevated_auto_start(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    lock(&state)
        .set_elevated_auto_start(enabled)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_week_totals(state: State<'_, AppState>) -> (i64, i64) {
    lock(&state).get_week_totals()
}

#[tauri::command]
pub fn get_day_detail(state: State<'_, AppState>, date: String) -> DayDetailDto {
    lock(&state).get_day_detail(date)
}

#[tauri::command]
pub fn get_day_hourly(state: State<'_, AppState>, date: String) -> Vec<i64> {
    lock(&state).get_day_hourly(date)
}

#[tauri::command]
pub fn get_year_heatmap(state: State<'_, AppState>, year: i32) -> Vec<(String, i64)> {
    lock(&state).get_year_heatmap(year)
}

#[tauri::command]
pub fn get_hour_apps(state: State<'_, AppState>, date: String, hour: u32) -> Vec<AppUsageDto> {
    lock(&state).get_hour_apps(date, hour)
}

#[tauri::command]
pub fn get_app_hourly(state: State<'_, AppState>, app_name: String, date: String) -> Vec<i64> {
    lock(&state).get_app_hourly(app_name, date)
}

#[tauri::command]
pub fn clear_data(state: State<'_, AppState>) {
    lock(&state).clear_data();
}

#[tauri::command]
pub fn export_csv(state: State<'_, AppState>, start: String, end: String) -> String {
    lock(&state).export_csv(start, end)
}

/// 重启应用（窗口材质等配置重启后生效）。
/// 实现：先启动一个独立延迟重启器（1 秒后拉起新实例），再干净退出当前进程，
/// 确保单实例锁与窗口资源先释放，避免新实例被旧实例拦截。
#[tauri::command]
pub fn restart_app(app: tauri::AppHandle) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // PowerShell 延迟重启器：等待 1 秒后拉起新实例；当前进程随后干净退出。
    // 不用 cmd timeout（无输入重定向时会报错退出），也避免 start 的弹窗问题。
    let exe_str = exe.to_string_lossy().replace('\'', "''");
    let ps_script = format!(
        "Start-Sleep -Seconds 1; Start-Process -FilePath '{}' -ArgumentList '--show-window'",
        exe_str
    );
    let spawned = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &ps_script,
        ])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .spawn();
    tracing::info!("restart helper spawned: {:?}", spawned.is_ok());
    if spawned.is_err() {
        return Err(format!("无法启动重启器: {}", spawned.err().unwrap()));
    }
    app.exit(0);
    Ok(())
}

/// Windows 官方按应用流量查询（免管理员、非实时累计字节）。
/// 跨进程调用可能耗时数百毫秒，放到阻塞线程执行，避免卡住界面。
#[tauri::command]
pub async fn get_attributed_usage(days: u64) -> AttributedUsageResult {
    tauri::async_runtime::spawn_blocking(move || crate::attributed::query(days))
        .await
        .unwrap_or_else(|e| crate::attributed::unavailable(format!("查询线程异常：{e}")))
}

/// 导出敏感数据明文（日记 + 窗口标题）。
#[tauri::command]
pub fn export_plaintext(state: State<'_, AppState>) -> ExportResultDto {
    lock(&state).export_plaintext()
}

/// 在资源管理器中定位某个文件（导出/日志等）。
#[tauri::command]
pub fn reveal_in_explorer(path: String) -> Result<(), String> {
    std::process::Command::new("explorer.exe")
        .arg(format!("/select,{path}"))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// 用系统默认浏览器打开外部链接（关于页仓库链接等）。
/// 用 ShellExecuteW 而非 cmd start：避免命令注入面与控制台窗口。
#[tauri::command]
pub fn open_external_url(url: String) -> Result<(), String> {
    let wide: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
    let result = unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            windows_sys::core::w!("open"),
            wide.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        )
    };
    // ShellExecuteW 返回值 > 32 表示成功（失败时是错误码）。
    let code = result as isize;
    if code > 32 {
        Ok(())
    } else {
        Err(format!("无法打开链接（错误码 {code}）"))
    }
}

/// 前端首帧渲染完成后调用：按启动参数决定是否显示主窗口。
/// 延迟到此时再显示，避免 WebView 尚未加载完成时的纯黑首帧闪烁。
#[tauri::command]
pub fn mark_ui_ready(app: tauri::AppHandle, state: State<'_, AppState>) {
    use std::sync::atomic::Ordering;
    if state.should_show.load(Ordering::Relaxed) {
        crate::show_main_window(&app);
    }
}

fn parse_date(s: &str) -> chrono::NaiveDate {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .unwrap_or_else(|_| chrono::Local::now().date_naive())
}

/// Extract a clean, env-expanded exe path from a startup command line.
/// Handles: quoted paths, trailing args, %VAR% env vars, double backslashes.
fn clean_exe_path(cmd: &str) -> Option<String> {
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
fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
#[cfg(test)]
mod tests {
    use super::{clean_exe_path, csv_escape};

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
