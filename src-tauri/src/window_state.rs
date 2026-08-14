//! 主窗口状态记忆：大小、位置、最大化状态。
//!
//! 数据保存到 %APPDATA%\TimeTrace\window_state.json。
//! 只在窗口非最大化时更新位置/大小（最大化时保留上次还原尺寸），
//! 最大化状态单独记录，下次启动按原样恢复。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

const STATE_FILE: &str = "window_state.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowState {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub maximized: bool,
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
    serde_json::from_str(&text).unwrap_or_default()
}

fn save(state: &WindowState) {
    if let Ok(json) = serde_json::to_string_pretty(state) {
        let _ = std::fs::write(state_path(), json);
    }
}

/// 从窗口读取当前状态：仅非最大化时更新位置/大小，最大化单独记录。
fn capture(window: &WebviewWindow, mut current: WindowState) -> WindowState {
    let maximized = window.is_maximized().unwrap_or(false);
    if !maximized {
        if let Ok(size) = window.outer_size() {
            current.width = Some(size.width);
            current.height = Some(size.height);
        }
        if let Ok(pos) = window.outer_position() {
            current.x = Some(pos.x);
            current.y = Some(pos.y);
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
pub fn apply(window: &WebviewWindow) {
    let state = load();
    if state.maximized {
        let _ = window.maximize();
        return;
    }
    if let (Some(w), Some(h)) = (state.width, state.height) {
        if w >= 100 && h >= 100 {
            let _ = window.set_size(PhysicalSize::new(w, h));
        }
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
fn position_visible(x: i32, y: i32, w: u32, h: u32, window: &WebviewWindow) -> bool {
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
