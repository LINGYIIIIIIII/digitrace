//! 跨进程文件信号协议（同机、跨提权边界可用，走 %APPDATA%\TimeTrace\ 文件系统）。
//!
//! 1. `pending_takeover` —— 新版启动时检测到旧版在跑，写入「待切换」标记；
//!    旧版（单实例回调或轮询线程）原子认领（rename）后弹窗询问切换。
//! 2. `show_request` —— 双击启动时检测到同路径实例在跑（常见提权实例，
//!    单实例回调被完整性级别隔离拦截），写「显示窗口」请求，由运行中实例
//!    轮询消费并唤出主窗口。
//!
//! 约定：JSON 序列化；带 TTL 的请求由消费方判断新鲜度（陈旧标记直接清除，
//! 防止残留误触发）。所有读写失败一律静默（信号属尽力而为的增强机制）。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

const PENDING_TAKEOVER_FILE: &str = "pending_takeover.json";
/// 认领用的临时文件名：用 rename 原子地「认领」接管请求，
/// 避免单实例回调和轮询线程同时处理导致重复弹窗。
const PENDING_TAKEOVER_CLAIM: &str = "pending_takeover.claim";
const SHOW_REQUEST_FILE: &str = "show_request.json";
/// 显示请求的新鲜度窗口（秒）：只处理该时限内的请求，防止陈旧标记误触发。
const SHOW_REQUEST_TTL_SECS: u64 = 15;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingTakeover {
    pub exe_path: String,
    pub version: String,
    /// 新版是否以管理员身份运行：旧版认领后需用 RunAs 拉起，避免权限丢失。
    #[serde(default)]
    pub elevated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShowRequest {
    pub exe_path: String,
    /// 写入时间戳（Unix 秒），用于陈旧性判断。
    pub written_at: u64,
}

fn data_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("TimeTrace")
}

fn pending_takeover_path() -> PathBuf {
    data_dir().join(PENDING_TAKEOVER_FILE)
}

fn show_request_path() -> PathBuf {
    data_dir().join(SHOW_REQUEST_FILE)
}

/// 新版进程启动时写入「待切换」标记（检测到旧版正在运行）。
/// 旧版收到单实例唤醒后读取并删除它，再在自己界面里询问用户。
pub fn write_pending_takeover(exe_path: &str, elevated: bool) {
    let p = PendingTakeover {
        exe_path: exe_path.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        elevated,
    };
    if let Ok(json) = serde_json::to_string(&p) {
        let _ = std::fs::write(pending_takeover_path(), json);
    }
}

/// 原子认领待切换标记（旧版单实例回调 / 轮询线程共用）。
/// rename 成功的一方获得处理权，另一方返回 None。
pub fn consume_pending_takeover() -> Option<PendingTakeover> {
    let src = pending_takeover_path();
    let dst = data_dir().join(PENDING_TAKEOVER_CLAIM);
    if std::fs::rename(&src, &dst).is_err() {
        return None;
    }
    let content = std::fs::read_to_string(&dst).ok();
    let _ = std::fs::remove_file(&dst);
    content.and_then(|c| serde_json::from_str(&c).ok())
}

/// 待切换标记是否仍然存在（新版进程轮询它判断旧版是否已接管）。
pub fn pending_takeover_exists() -> bool {
    pending_takeover_path().exists()
}

/// 启动时清理可能残留的待切换标记（上一次交接未完成时）。
pub fn clear_stale_pending_takeover() {
    let _ = std::fs::remove_file(pending_takeover_path());
}

/// 写入「显示窗口」请求（检测到同路径实例在跑、且非自启 `--tray` 时调用）。
pub fn write_show_request(exe_path: &str) {
    let req = ShowRequest {
        exe_path: exe_path.to_string(),
        written_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    };
    if let Ok(json) = serde_json::to_string(&req) {
        let _ = std::fs::write(show_request_path(), json);
    }
}

/// 消费「显示窗口」请求：只处理新鲜窗口（≤15s）内的请求，陈旧标记直接清除。
pub fn take_show_request() -> Option<ShowRequest> {
    let path = show_request_path();
    let modified = std::fs::metadata(&path).ok()?.modified().ok()?;
    let age = std::time::SystemTime::now()
        .duration_since(modified)
        .unwrap_or_default()
        .as_secs();
    if age > SHOW_REQUEST_TTL_SECS {
        let _ = std::fs::remove_file(&path);
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    let _ = std::fs::remove_file(&path);
    serde_json::from_str(&content).ok()
}

/// 启动时清理残留的显示请求（自启 `--tray` 场景调用，保证开机静默）。
pub fn clear_stale_show_request() {
    let _ = std::fs::remove_file(show_request_path());
}
