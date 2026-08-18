//! 健康提醒：连续使用电脑时长检测 + Windows 原生通知。
//!
//! 规则：
//! - 后台线程每 5 秒读取一次键盘/鼠标空闲时长（GetLastInputInfo，无需管理员）；
//! - 空闲 ≥ 休息阈值（默认 5 分钟）视为休息，连续计时归零；
//! - 连续使用 ≥ 提醒间隔（默认 60 分钟）弹 Windows 通知，随后归零重新计时；
//! - 通知用托盘气泡（Shell_NotifyIcon）实现，属于 Windows 自带通知机制，
//!   无需管理员权限、无需打包身份（AppUserModelID）。

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use timetrace_core::{AppConfig, IdleDetector, Win32IdleDetector};

const TICK_SECONDS: u64 = 5;
/// 连续时长恢复窗口：距上次保存 ≤90 秒（覆盖更新确认+重启）就接着计时，否则重新计时。
const RESUME_WINDOW_SECS: u64 = 90;
const STATE_FILE: &str = "health_state.json";

#[derive(Debug, Clone, Serialize)]
pub struct HealthSnapshotDto {
    pub enabled: bool,
    pub reminder_minutes: u64,
    pub break_minutes: u64,
    /// 当前连续使用秒数。
    pub streak_seconds: u64,
    /// 当前键盘/鼠标空闲秒数。
    pub idle_seconds: u64,
    /// 今日已提醒次数。
    pub reminders_today: u32,
    /// 距下次提醒的秒数（负数视为 0）。
    pub next_reminder_seconds: i64,
    pub last_reminder_local: Option<String>,
    pub last_break_local: Option<String>,
}

#[derive(Debug, Default)]
struct HealthState {
    streak_started: Option<Instant>,
    streak_seconds: u64,
    reminders_today: u32,
    last_reminder_local: Option<String>,
    last_break_local: Option<String>,
    day: String,
}

/// 健康提醒运行状态的持久化快照（更新/重启后恢复，短间隔接着计时）。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct HealthPersist {
    day: String,
    reminders_today: u32,
    last_reminder_local: Option<String>,
    last_break_local: Option<String>,
    streak_seconds: u64,
    saved_at_unix: u64,
}

fn state_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("TimeTrace")
        .join(STATE_FILE)
}

fn save_persist_to(path: &std::path::Path, state: &HealthState) {
    let p = HealthPersist {
        day: state.day.clone(),
        reminders_today: state.reminders_today,
        last_reminder_local: state.last_reminder_local.clone(),
        last_break_local: state.last_break_local.clone(),
        streak_seconds: state.streak_seconds,
        saved_at_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    };
    if let Ok(json) = serde_json::to_string_pretty(&p) {
        let _ = std::fs::write(path, json);
    }
}

fn load_persist_from(path: &std::path::Path) -> Option<HealthPersist> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// 连续时长恢复判定：间隔 ≤90 秒返回「保存值 + 间隔」，否则返回 0（重新计时）。
fn compute_resumed_streak(saved_streak: u64, saved_at_unix: u64, now_unix: u64) -> u64 {
    let gap = now_unix.saturating_sub(saved_at_unix);
    if gap <= RESUME_WINDOW_SECS {
        saved_streak.saturating_add(gap)
    } else {
        0
    }
}

/// 启动时加载并恢复健康提醒状态（跨天重置、短间隔续上连续时长）。
fn load_state() -> HealthState {
    let mut state = HealthState::default();
    let Some(p) = load_persist_from(&state_path()) else {
        return state;
    };
    state.day = p.day;
    state.reminders_today = p.reminders_today;
    state.last_reminder_local = p.last_reminder_local;
    state.last_break_local = p.last_break_local;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    if state.day != today {
        state.day = today;
        state.reminders_today = 0;
    }
    let now_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let resumed = compute_resumed_streak(p.streak_seconds, p.saved_at_unix, now_unix);
    if resumed > 0 {
        state.streak_started = Some(Instant::now() - Duration::from_secs(resumed));
        state.streak_seconds = resumed;
    }
    state
}

fn persist(state: &HealthState) {
    save_persist_to(&state_path(), state);
}

pub struct HealthTracker {
    state: Arc<Mutex<HealthState>>,
}

impl HealthTracker {
    /// 启动后台线程并返回追踪器（用于前端查询快照）。
    pub fn start(app: tauri::AppHandle) -> Arc<Self> {
        let tracker = Arc::new(Self {
            state: Arc::new(Mutex::new(load_state())),
        });
        let runner = tracker.clone();
        std::thread::spawn(move || {
            let idle = Win32IdleDetector::new();
            loop {
                std::thread::sleep(Duration::from_secs(TICK_SECONDS));
                runner.tick(&idle, &app);
            }
        });
        tracker
    }

    fn tick(&self, idle_detector: &Win32IdleDetector, app: &tauri::AppHandle) {
        let config = AppConfig::load();
        let idle_secs = idle_detector.idle_duration().as_secs();
        let reminder_secs = config.health.health_reminder_minutes.clamp(1, 480) * 60;
        let break_secs = config.health.health_break_minutes.clamp(1, 60) * 60;

        let mut state = match self.state.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        let now = chrono::Local::now();
        let today = now.format("%Y-%m-%d").to_string();
        if state.day != today {
            state.day = today;
            state.reminders_today = 0;
            persist(&state);
        }

        if !config.health.health_reminder_enabled {
            state.streak_seconds = 0;
            state.streak_started = None;
            persist(&state);
            return;
        }

        // 空闲足够久 → 视为休息，连续计时归零。
        if idle_secs >= break_secs {
            if state.streak_seconds > 0 || state.streak_started.is_some() {
                state.last_break_local = Some(now.format("%H:%M").to_string());
            }
            state.streak_seconds = 0;
            state.streak_started = None;
            persist(&state);
            return;
        }

        let streak = state
            .streak_started
            .as_ref()
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0);
        state.streak_seconds = streak;

        if streak >= reminder_secs {
            drop(state);
            show_toast(
                app,
                "数迹 · 健康提醒",
                &format!(
                    "你已经连续使用电脑 {} 分钟了，起来活动一下、看看远处吧。",
                    config.health.health_reminder_minutes.clamp(1, 480)
                ),
            );
            if let Ok(mut state) = self.state.lock() {
                state.reminders_today += 1;
                state.last_reminder_local = Some(now.format("%H:%M").to_string());
                state.streak_seconds = 0;
                state.streak_started = Some(Instant::now());
                persist(&state);
            }
            return;
        } else if state.streak_started.is_none() {
            state.streak_started = Some(Instant::now());
        }
        persist(&state);
    }

    /// 立即保存当前状态（应用退出前兜底，避免最后几秒的数据丢失）。
    pub fn persist_now(&self) {
        if let Ok(state) = self.state.lock() {
            persist(&state);
        }
    }

    /// 前端快照：连续时长 / 空闲时长 / 今日提醒次数 / 下次提醒倒计时。
    pub fn snapshot(&self) -> HealthSnapshotDto {
        let config = AppConfig::load();
        let idle_detector = Win32IdleDetector::new();
        let idle_secs = idle_detector.idle_duration().as_secs();
        let reminder_minutes = config.health.health_reminder_minutes.clamp(1, 480);
        let state = match self.state.lock() {
            Ok(s) => s,
            Err(_) => {
                return HealthSnapshotDto {
                    enabled: config.health.health_reminder_enabled,
                    reminder_minutes,
                    break_minutes: config.health.health_break_minutes.clamp(1, 60),
                    streak_seconds: 0,
                    idle_seconds: idle_secs,
                    reminders_today: 0,
                    next_reminder_seconds: reminder_minutes as i64 * 60,
                    last_reminder_local: None,
                    last_break_local: None,
                }
            }
        };
        let streak = state
            .streak_started
            .as_ref()
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(state.streak_seconds);
        let next = (reminder_minutes as i64 * 60)
            .saturating_sub(streak as i64)
            .max(0);
        HealthSnapshotDto {
            enabled: config.health.health_reminder_enabled,
            reminder_minutes,
            break_minutes: config.health.health_break_minutes.clamp(1, 60),
            streak_seconds: streak,
            idle_seconds: idle_secs,
            reminders_today: state.reminders_today,
            next_reminder_seconds: next,
            last_reminder_local: state.last_reminder_local.clone(),
            last_break_local: state.last_break_local.clone(),
        }
    }
}

#[tauri::command]
pub fn get_health_snapshot(state: tauri::State<'_, Arc<HealthTracker>>) -> HealthSnapshotDto {
    state.snapshot()
}

/// 立即发送一条测试通知（设置页/健康页「测试」按钮）。
#[tauri::command]
pub fn test_health_notification(app: tauri::AppHandle) -> Result<(), String> {
    let ok = show_toast(
        &app,
        "数迹 · 健康提醒",
        "这是测试通知：连续使用提醒已就绪。",
    );
    if ok {
        Ok(())
    } else {
        Err("系统未接受通知请求".to_string())
    }
}

// ── Windows 托盘气泡通知 ────────────────────────────────────────

/// 定位 Tauri 主托盘图标的原生窗口和 Shell ID。
///
/// tray-icon 将原生 ID 保持为私有字段，但 Windows 后端使用进程内的
/// `tray_icon_app` 窗口类。找到当前进程的窗口后，逐个探测已注册 ID，
/// 便可用 `NIM_MODIFY` 更新已有图标，而不再调用 `NIM_ADD` 创建第二个图标。
fn main_tray_target() -> Option<(windows_sys::Win32::Foundation::HWND, u32)> {
    use windows_sys::Win32::Foundation::{GetLastError, S_OK};
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    use windows_sys::Win32::UI::Shell::{Shell_NotifyIconGetRect, NOTIFYICONIDENTIFIER};
    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowExW, GetWindowThreadProcessId};

    let current_process = unsafe { GetCurrentProcessId() };
    let mut after = std::ptr::null_mut();
    let hwnd = loop {
        let candidate = unsafe {
            FindWindowExW(
                std::ptr::null_mut(),
                after,
                windows_sys::core::w!("tray_icon_app"),
                std::ptr::null(),
            )
        };
        if candidate.is_null() {
            timetrace_core::oplog::log_event(
                "TOAST",
                &format!("主托盘窗口未找到 err={}", unsafe { GetLastError() }),
            );
            return None;
        }
        let mut process_id = 0;
        unsafe { GetWindowThreadProcessId(candidate, &mut process_id) };
        if process_id == current_process {
            break candidate;
        }
        after = candidate;
    };

    // tray-icon 当前从 1 开始递增；探测有限范围避免依赖该私有实现细节。
    for uid in 1..=256 {
        let identifier = NOTIFYICONIDENTIFIER {
            cbSize: std::mem::size_of::<NOTIFYICONIDENTIFIER>() as u32,
            hWnd: hwnd,
            uID: uid,
            guidItem: unsafe { std::mem::zeroed() },
        };
        let mut rect = unsafe { std::mem::zeroed() };
        if unsafe { Shell_NotifyIconGetRect(&identifier, &mut rect) } == S_OK {
            return Some((hwnd, uid));
        }
    }
    timetrace_core::oplog::log_event("TOAST", "主托盘窗口没有已注册的 Shell 图标 ID");
    None
}

fn copy_wchar(dst: &mut [u16], s: &str) {
    let units: Vec<u16> = s.encode_utf16().collect();
    let n = units.len().min(dst.len().saturating_sub(1));
    dst[..n].copy_from_slice(&units[..n]);
    dst[n] = 0;
}

fn build_info_nid(
    hwnd: windows_sys::Win32::Foundation::HWND,
    uid: u32,
    title: &str,
    body: &str,
) -> windows_sys::Win32::UI::Shell::NOTIFYICONDATAW {
    use windows_sys::Win32::UI::Shell::{NIF_INFO, NIIF_INFO, NOTIFYICONDATAW, NOTIFYICONDATAW_0};

    let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = uid;
    nid.uFlags = NIF_INFO;
    copy_wchar(&mut nid.szInfoTitle, title);
    copy_wchar(&mut nid.szInfo, body);
    nid.Anonymous = NOTIFYICONDATAW_0 { uTimeout: 10000 };
    nid.dwInfoFlags = NIIF_INFO;
    nid
}

/// 显示一条 Windows 原生通知（托盘气泡，系统渲染，贴近右下角）。
/// 用户指定使用系统自带样式，不使用自绘窗口。
pub fn show_toast(_app: &tauri::AppHandle, title: &str, body: &str) -> bool {
    fallback_balloon(title, body)
}

/// Windows 托盘气泡：修改现有主托盘图标，避免增加第二个图标。
fn fallback_balloon(title: &str, body: &str) -> bool {
    use windows_sys::Win32::Foundation::{GetLastError, TRUE};
    use windows_sys::Win32::UI::Shell::{Shell_NotifyIconW, NIM_MODIFY};

    let Some((hwnd, uid)) = main_tray_target() else {
        return false;
    };

    // NIM_MODIFY targets the already registered Tauri tray icon. Windows owns
    // the balloon timeout; no temporary icon needs to be removed afterwards.
    let nid = build_info_nid(hwnd, uid, title, body);
    let ok = unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid) } == TRUE;
    let err = unsafe { GetLastError() };
    timetrace_core::oplog::log_event(
        "TOAST",
        &format!(
            "气泡通知: NIM_MODIFY={} err={} hwnd={} uid={} title={}",
            ok, err, hwnd as isize, uid, title
        ),
    );
    ok
}

/// 保留退出钩子接口；通知现在复用主托盘，不需要删除独立图标。
pub fn cleanup_toast_icon() {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn persist_roundtrip_preserves_fields() {
        let path = std::env::temp_dir().join(format!(
            "tt_health_test_{}_{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let state = HealthState {
            day: "2026-08-11".to_string(),
            reminders_today: 3,
            last_reminder_local: Some("14:30".to_string()),
            last_break_local: Some("12:00".to_string()),
            streak_seconds: 120,
            streak_started: None,
        };
        save_persist_to(&path, &state);
        let loaded = load_persist_from(&path).unwrap();
        assert_eq!(loaded.day, "2026-08-11");
        assert_eq!(loaded.reminders_today, 3);
        assert_eq!(loaded.last_reminder_local.as_deref(), Some("14:30"));
        assert_eq!(loaded.last_break_local.as_deref(), Some("12:00"));
        assert_eq!(loaded.streak_seconds, 120);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn resume_streak_within_window() {
        assert_eq!(compute_resumed_streak(100, 1000, 1030), 130);
        assert_eq!(compute_resumed_streak(0, 1000, 1030), 30);
    }

    #[test]
    fn resume_streak_resets_after_window() {
        assert_eq!(compute_resumed_streak(100, 1000, 1090), 190); // 边界：恰好 90 秒
        assert_eq!(compute_resumed_streak(100, 1000, 1091), 0);
        assert_eq!(compute_resumed_streak(100, 1000, 1200), 0);
    }
}
