//! 主窗口状态记忆：大小、位置、最大化状态。
//!
//! 数据保存到 %APPDATA%\TimeTrace\window_state.json。
//! 只在窗口非最大化时更新位置/大小（最大化时保留上次还原尺寸），
//! 最大化状态单独记录，下次启动按原样恢复。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewWindow};

const STATE_FILE: &str = "window_state.json";
const MIN_WIDTH: u32 = 900;
const MIN_HEIGHT: u32 = 620;
const MAX_WIDTH: u32 = 8192;
const MAX_HEIGHT: u32 = 8192;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowState {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub maximized: bool,
}

fn sanitize(mut state: WindowState) -> WindowState {
    // Older builds could persist the hidden WebView placeholder (14x14 or a
    // taskbar sentinel position). Never feed those values back into startup.
    if !state
        .width
        .is_some_and(|value| (MIN_WIDTH..=MAX_WIDTH).contains(&value))
    {
        state.width = None;
    }
    if !state
        .height
        .is_some_and(|value| (MIN_HEIGHT..=MAX_HEIGHT).contains(&value))
    {
        state.height = None;
    }
    if state.x.is_some_and(|value| value <= -16_000)
        || state.y.is_some_and(|value| value <= -16_000)
    {
        state.x = None;
        state.y = None;
    }
    state
}

static SAVE_PENDING: AtomicBool = AtomicBool::new(false);

fn state_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("TimeTrace")
        .join(STATE_FILE)
}

pub fn load() -> WindowState {
    let Ok(text) = std::fs::read_to_string(state_path()) else {
        return WindowState::default();
    };
    sanitize(serde_json::from_str(&text).unwrap_or_default())
}

fn save(state: &WindowState) {
    if let Ok(json) = serde_json::to_string_pretty(state) {
        let _ = std::fs::write(state_path(), json);
    }
}

/// 从窗口读取当前状态：仅非最大化时更新位置/大小，最大化单独记录。
fn capture<R: Runtime>(window: &WebviewWindow<R>, mut current: WindowState) -> WindowState {
    let maximized = window.is_maximized().unwrap_or(false);
    if !maximized {
        if let Ok(size) = window.outer_size() {
            if (MIN_WIDTH..=MAX_WIDTH).contains(&size.width)
                && (MIN_HEIGHT..=MAX_HEIGHT).contains(&size.height)
            {
                current.width = Some(size.width);
                current.height = Some(size.height);
            }
        }
        if current.width.is_some() && current.height.is_some() {
            if let Ok(pos) = window.outer_position() {
                if pos.x > -16_000 && pos.y > -16_000 {
                    current.x = Some(pos.x);
                    current.y = Some(pos.y);
                }
            }
        }
    }
    current.maximized = maximized;
    current
}

/// 窗口移动/缩放后延迟合并保存（800ms 防抖，拖动时不会频繁写文件）。
pub fn schedule_save(app: &AppHandle) {
    if SAVE_PENDING.swap(true, Ordering::Relaxed) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(800));
        SAVE_PENDING.store(false, Ordering::Relaxed);
        if let Some(window) = app.get_webview_window("main") {
            let merged = capture(&window, load());
            save(&merged);
        }
    });
}

/// 启动时恢复窗口状态（大小 / 位置 / 最大化）。
pub fn apply<R: Runtime>(window: &WebviewWindow<R>) {
    let state = load();
    if state.maximized {
        let _ = window.maximize();
        return;
    }
    if let (Some(w), Some(h)) = (state.width, state.height) {
        let _ = window.set_size(PhysicalSize::new(w, h));
    }
    if let (Some(x), Some(y)) = (state.x, state.y) {
        let w = state.width.unwrap_or(0);
        let h = state.height.unwrap_or(0);
        if position_visible(x, y, w, h, window) {
            let _ = window.set_position(PhysicalPosition::new(x, y));
        }
    }
}

/// 粗略检查窗口中心是否落在任一显示器工作区内（避免恢复后跑到屏幕外）。
fn position_visible<R: Runtime>(x: i32, y: i32, w: u32, h: u32, window: &WebviewWindow<R>) -> bool {
    let Ok(monitors) = window.available_monitors() else {
        return true;
    };
    let cx = x + (w as i32) / 2;
    let cy = y + (h as i32) / 2;
    monitors.iter().any(|m| {
        let area = m.work_area();
        cx >= area.position.x
            && cx <= area.position.x + area.size.width as i32
            && cy >= area.position.y
            && cy <= area.position.y + area.size.height as i32
    })
}

#[cfg(test)]
mod tests {
    use super::{sanitize, WindowState};

    #[test]
    fn rejects_placeholder_size_and_offscreen_sentinel() {
        let state = sanitize(WindowState {
            x: Some(-32_000),
            y: Some(-32_000),
            width: Some(276),
            height: Some(45),
            maximized: false,
        });
        assert_eq!(state.x, None);
        assert_eq!(state.y, None);
        assert_eq!(state.width, None);
        assert_eq!(state.height, None);
    }

    #[test]
    fn preserves_valid_window_state() {
        let state = sanitize(WindowState {
            x: Some(120),
            y: Some(80),
            width: Some(1100),
            height: Some(760),
            maximized: false,
        });
        assert_eq!(state.x, Some(120));
        assert_eq!(state.y, Some(80));
        assert_eq!(state.width, Some(1100));
        assert_eq!(state.height, Some(760));
    }
}
