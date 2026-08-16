//! 自 api.rs 按域拆分（纯搬迁，行为不变）。
use super::*;
use timetrace_core::*;
impl TimeTraceApi {
    /// Read the current user configuration.
    pub fn get_config(&self) -> ConfigDto {
        let config = AppConfig::load();
        ConfigDto {
            poll_interval_ms: config.poll_interval_ms,
            idle_threshold_minutes: config.idle_threshold_minutes,
            refresh_interval_seconds: config.refresh_interval_seconds,
            live_refresh_interval_seconds: config.live.live_refresh_interval_seconds,
            network_live_window_seconds: config.live.network_live_window_seconds,
            excluded_apps: config.excluded_apps.clone(),
            db_path: self.db_path.clone(),
            start_minimized: config.start_minimized,
            theme_mode: config.theme_mode,
            window_blur: config.window_blur,
            font_family: config.font_family,
            titlebar_items: config.titlebar_items.clone(),
            health_reminder_enabled: config.health.health_reminder_enabled,
            health_reminder_minutes: config.health.health_reminder_minutes,
            health_break_minutes: config.health.health_break_minutes,
            update_check_enabled: config.update.update_check_enabled,
            update_manifest_url: config.update.update_manifest_url,
            update_github_repo: config.update.update_github_repo,
            update_silent: config.update.update_silent,
            update_check_hour: config.update.update_check_hour,
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
        app_config.live.live_refresh_interval_seconds =
            config.live_refresh_interval_seconds.clamp(1, 3600);
        app_config.live.network_live_window_seconds =
            config.network_live_window_seconds.clamp(60, 600);
        self.monitor_core
            .set_live_window(config.network_live_window_seconds);
        app_config.health.health_reminder_minutes = config.health_reminder_minutes.clamp(5, 1440);
        app_config.health.health_break_minutes = config.health_break_minutes.clamp(1, 1440);
        app_config.excluded_apps = config.excluded_apps;
        app_config.start_minimized = config.start_minimized;
        app_config.theme_mode = config.theme_mode;
        app_config.window_blur = config.window_blur;
        app_config.font_family = config.font_family;
        app_config.titlebar_items = config.titlebar_items;
        app_config.health.health_reminder_enabled = config.health_reminder_enabled;
        app_config.update.update_check_enabled = config.update_check_enabled;
        let repo = config.update_github_repo.trim();
        if !repo.is_empty() && !repo.contains('/') {
            return Err(anyhow::anyhow!(
                "GitHub 仓库格式应为「所有者/仓库名」，如 LINGYIIIIIIII/digitrace"
            ));
        }
        app_config.update.update_github_repo = repo.to_string();
        app_config.update.update_silent = config.update_silent;
        if let Some(hour) = config.update_check_hour {
            if hour > 23 {
                return Err(anyhow::anyhow!("检查时刻必须在 0-23 之间"));
            }
            app_config.update.update_check_hour = Some(hour);
        } else {
            app_config.update.update_check_hour = None;
        }
        let url = config.update_manifest_url.trim();
        if !url.is_empty() && !url.starts_with("https://") {
            return Err(anyhow::anyhow!("更新地址必须使用 HTTPS（安全要求）"));
        }
        app_config.update.update_manifest_url = url.to_string();
        app_config.tray_items = config.tray_items;
        app_config.launch_show_window = config.launch_show_window;
        app_config
            .save()
            .map_err(|e| anyhow::anyhow!(e.to_string()))
    }
}
