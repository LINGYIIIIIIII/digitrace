//! 自 api.rs 按域拆分（纯搬迁，行为不变）。
use super::*;
use timetrace_core::*;
impl TimeTraceApi {
    /// Extract an exe icon as raw RGBA pixels.
    /// 有界缓存（64 个，FIFO 淘汰）：应用列表反复请求图标时避免重复提取
    /// （Win32 提取 + RGBA 分配开销大），同时防止缓存无限增长。
    pub fn get_app_icon(&self, exe_path: String) -> Option<IconDto> {
        use std::sync::Mutex;
        use std::sync::OnceLock;

        static CACHE: OnceLock<Mutex<std::collections::VecDeque<(String, IconDto)>>> =
            OnceLock::new();
        const CAP: usize = 64;

        let cache = CACHE.get_or_init(|| Mutex::new(std::collections::VecDeque::new()));
        if let Ok(mut guard) = cache.lock() {
            if let Some(pos) = guard.iter().position(|(p, _)| *p == exe_path) {
                let hit = guard.remove(pos).unwrap();
                return Some(hit.1);
            }
        }
        let cleaned = clean_exe_path(&exe_path).unwrap_or_else(|| exe_path.clone());
        let icon = crate::icons::extract_icon_rgba(&cleaned).map(|(w, h, rgba)| IconDto {
            width: w as i64,
            height: h as i64,
            rgba,
        });
        if icon.is_some() {
            if let Ok(mut guard) = cache.lock() {
                if guard.len() >= CAP {
                    guard.pop_front();
                }
                if let Some(ic) = icon.clone() {
                    guard.push_back((exe_path, ic));
                }
            }
        }
        icon
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
}
