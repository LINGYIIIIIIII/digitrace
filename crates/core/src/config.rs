//! Configuration management.
//!
//! Reads/writes `%APPDATA%\TimeTrace\config.json`.
//! 分组说明：实时/健康/更新三组字段在代码里按 `serde(flatten)` 分组，
//! 但磁盘 JSON 的键名保持扁平不变（兼容既有配置文件，零迁移）。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::warn;

/// 实时数据相关配置（刷新间隔、实时曲线保留窗口）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LiveConfig {
    /// 实时页面（硬件监控 / 网络监控）的刷新间隔（秒，可到 1 秒）。
    #[serde(default = "default_live_refresh_interval")]
    pub live_refresh_interval_seconds: u64,

    /// 实时网络曲线的内存留存窗口（秒）：秒级样本在内存环形缓冲保留该时长，
    /// 实时速率曲线在任何时刻都显示最近 N 秒（默认 5 分钟，重启清零）。
    #[serde(default = "default_network_live_window")]
    pub network_live_window_seconds: u64,
}

/// 健康提醒配置（连续使用提醒）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HealthConfig {
    /// 健康提醒：是否启用「连续使用电脑」提醒。
    #[serde(default = "default_true")]
    pub health_reminder_enabled: bool,

    /// 健康提醒：连续使用多少分钟提醒一次。
    #[serde(default = "default_health_reminder_minutes")]
    pub health_reminder_minutes: u64,

    /// 健康提醒：离开电脑多少分钟算休息（连续计时归零）。
    #[serde(default = "default_health_break_minutes")]
    pub health_break_minutes: u64,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            health_reminder_enabled: true,
            health_reminder_minutes: default_health_reminder_minutes(),
            health_break_minutes: default_health_break_minutes(),
        }
    }
}

/// 自动更新配置（更新源、静默模式、检查时刻）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateConfig {
    /// 自动更新：启动后是否自动检查新版本（每天最多一次）。
    #[serde(default = "default_true")]
    pub update_check_enabled: bool,

    /// 自动更新：更新清单地址（JSON）。为空表示未配置更新源，跳过检查。
    #[serde(default)]
    pub update_manifest_url: String,

    /// 自动更新：GitHub 公开仓库（`所有者/仓库名`）。设置后直接读取该仓库
    /// 最新 Release 进行更新（类似 THRM），优先于 update_manifest_url。
    #[serde(default)]
    pub update_github_repo: String,

    /// 自动更新：静默模式。开启后后台自动下载新版本，退出应用时静默替换，
    /// 下次启动即为新版，全程不弹确认框、不弹通知。
    #[serde(default)]
    pub update_silent: bool,

    /// 自动更新：固定检查时刻（0-23 点）。None=不固定
    /// （启动后约 6 秒检查一次，之后每 6 小时轮询，按自然日去重）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_check_hour: Option<u32>,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            update_check_enabled: true,
            update_manifest_url: String::new(),
            update_github_repo: String::new(),
            update_silent: false,
            update_check_hour: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Polling interval in milliseconds.
    #[serde(default = "default_poll_interval")]
    pub poll_interval_ms: u64,

    /// Idle threshold in minutes.
    #[serde(default = "default_idle_threshold")]
    pub idle_threshold_minutes: u64,

    /// Dashboard auto-refresh interval in seconds (shared by future
    /// features that poll local data).
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval_seconds: u64,

    /// 实时数据（刷新间隔 / 实时曲线保留窗口）。磁盘键名保持扁平。
    #[serde(default, flatten)]
    pub live: LiveConfig,

    /// 健康提醒。磁盘键名保持扁平。
    #[serde(default, flatten)]
    pub health: HealthConfig,

    /// 自动更新。磁盘键名保持扁平。
    #[serde(default, flatten)]
    pub update: UpdateConfig,

    /// Whether to minimize to system tray on close.
    #[serde(default = "default_true")]
    pub minimize_to_tray: bool,

    /// Whether to start minimized.
    #[serde(default)]
    pub start_minimized: bool,

    /// Whether to auto-start tracking on launch.
    #[serde(default = "default_true")]
    pub auto_start_tracking: bool,

    /// Applications to exclude from tracking (by exe name).
    #[serde(default)]
    pub excluded_apps: Vec<String>,

    /// 界面主题：system / light / dark。
    #[serde(default = "default_theme_mode")]
    pub theme_mode: String,

    /// 窗口材质：auto / mica / acrylic / tabbed / off（重启生效）。
    #[serde(default = "default_window_blur")]
    pub window_blur: String,

    /// 界面字体：system / harmonyos / noto（思源黑体）。
    #[serde(default = "default_font_family")]
    pub font_family: String,

    /// 顶栏显示的徽章项：monitoring / tray / active / idle。
    #[serde(default = "default_titlebar_items")]
    pub titlebar_items: Vec<String>,

    /// 日期时区：system（跟随系统）/ utc+8（东八区固定）。
    /// 所有"日历日"统计（今天/本周/本月、历史窗口）统一按它计算。
    #[serde(default = "default_timezone")]
    pub timezone: String,

    /// 托盘菜单显示的数据行：cpu / memory / network / active / temp。
    #[serde(default = "default_tray_items")]
    pub tray_items: Vec<String>,

    /// 启动时显示主界面（关闭则隐藏到托盘）。
    #[serde(default = "default_true")]
    pub launch_show_window: bool,

    /// 开机自启是否以管理员权限运行（计划任务 /rl highest，开机静默提权）。
    #[serde(default)]
    pub autostart_elevated: bool,
}

fn default_poll_interval() -> u64 {
    3000
}
fn default_idle_threshold() -> u64 {
    5
}
fn default_refresh_interval() -> u64 {
    10
}
fn default_live_refresh_interval() -> u64 {
    1
}
fn default_network_live_window() -> u64 {
    300
}
fn default_true() -> bool {
    true
}
fn default_theme_mode() -> String {
    "system".to_string()
}
fn default_window_blur() -> String {
    "auto".to_string()
}
fn default_font_family() -> String {
    "system".to_string()
}
fn default_titlebar_items() -> Vec<String> {
    vec!["monitoring".to_string(), "tray".to_string()]
}

fn default_timezone() -> String {
    "system".to_string()
}
fn default_health_reminder_minutes() -> u64 {
    60
}
fn default_health_break_minutes() -> u64 {
    5
}
fn default_tray_items() -> Vec<String> {
    vec![
        "cpu".to_string(),
        "memory".to_string(),
        "network".to_string(),
        "active".to_string(),
        "temp".to_string(),
    ]
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: 3000,
            idle_threshold_minutes: 5,
            refresh_interval_seconds: 10,
            live: LiveConfig::default(),
            health: HealthConfig::default(),
            update: UpdateConfig::default(),
            minimize_to_tray: true,
            start_minimized: false,
            auto_start_tracking: true,
            excluded_apps: Vec::new(),
            theme_mode: default_theme_mode(),
            window_blur: default_window_blur(),
            font_family: default_font_family(),
            titlebar_items: default_titlebar_items(),
            timezone: default_timezone(),
            tray_items: default_tray_items(),
            launch_show_window: true,
            autostart_elevated: false,
        }
    }
}

impl AppConfig {
    /// Load config from the default path, or return defaults.
    pub fn load() -> Self {
        let path = Self::config_path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => {
                // 一次性清理历史 DeepSeek 字段（不留痕迹）。
                Self::purge_deepseek_fields(&path, &contents);
                serde_json::from_str(&contents).unwrap_or_else(|e| {
                    warn!("Failed to parse config, using defaults: {e}");
                    Self::default()
                })
            }
            Err(_) => {
                // No config file yet — create with defaults
                let config = Self::default();
                let _ = config.save();
                config
            }
        }
    }

    /// 从配置文件里移除所有 `deepseek_*` 旧字段（幂等，不留痕迹）。
    fn purge_deepseek_fields(path: &std::path::Path, contents: &str) {
        let Ok(mut v) = serde_json::from_str::<serde_json::Value>(contents) else {
            return;
        };
        let Some(obj) = v.as_object_mut() else {
            return;
        };
        let mut changed = false;
        obj.retain(|k, _| {
            if k.starts_with("deepseek_") {
                changed = true;
                false
            } else {
                true
            }
        });
        if changed && let Ok(s) = serde_json::to_string(&v) {
            let _ = std::fs::write(path, s);
        }
    }

    /// Save config to the default path.
    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }

    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("TimeTrace")
            .join("config.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 磁盘 JSON 键名必须保持扁平（flatten 分组不改变持久化格式）。
    #[test]
    fn flatten_groups_keep_flat_json_keys() {
        let cfg = AppConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let obj = v.as_object().unwrap();
        // 分组字段仍是顶层扁平键
        for key in [
            "live_refresh_interval_seconds",
            "network_live_window_seconds",
            "health_reminder_enabled",
            "health_reminder_minutes",
            "health_break_minutes",
            "update_check_enabled",
            "update_manifest_url",
            "update_github_repo",
            "update_silent",
        ] {
            assert!(obj.contains_key(key), "缺少扁平键 {key}");
        }
        // 不出现分组名
        assert!(!obj.contains_key("live"));
        assert!(!obj.contains_key("health"));
        assert!(!obj.contains_key("update"));
    }

    /// 旧扁平 JSON 能正确解析到分组字段（向后兼容）。
    #[test]
    fn flat_json_round_trips_into_groups() {
        let json = r#"{
            "live_refresh_interval_seconds": 2,
            "network_live_window_seconds": 120,
            "health_reminder_enabled": false,
            "health_reminder_minutes": 30,
            "health_break_minutes": 10,
            "update_silent": true,
            "update_github_repo": "LINGYIIIIIIII/digitrace"
        }"#;
        let cfg: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.live.live_refresh_interval_seconds, 2);
        assert_eq!(cfg.live.network_live_window_seconds, 120);
        assert!(!cfg.health.health_reminder_enabled);
        assert_eq!(cfg.health.health_reminder_minutes, 30);
        assert!(cfg.update.update_silent);
        assert_eq!(cfg.update.update_github_repo, "LINGYIIIIIIII/digitrace");
    }
}
