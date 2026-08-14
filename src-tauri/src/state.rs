use crate::api::TimeTraceApi;

/// Tauri 全局状态：持有数迹核心 API 单例。
/// `TimeTraceApi` 内部含 rusqlite `Connection`（Send 但非 Sync），
/// 不满足 `tauri::State` 的 `Send + Sync` 要求，因此用 `Mutex` 包一层。
pub struct AppState {
    pub api: std::sync::Mutex<TimeTraceApi>,
    /// 启动后是否需要显示主窗口（前端就绪后由 mark_ui_ready 触发，避免首帧黑闪）。
    pub should_show: std::sync::atomic::AtomicBool,
}
