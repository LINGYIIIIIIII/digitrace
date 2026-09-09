//! 游戏识别与连续游戏提醒。
//!
//! - 游戏库：Steam / Epic / WeGame / 米哈游扫描 + 内置名单 + 手动条目（见 timetrace-core::games）；
//! - 连续游戏提醒：后台线程 5 秒 tick，读当前未关闭会话 → 命中游戏且未空闲 → 累计连续时长，
//!   到阈值弹 Windows 原生通知并归零。纯内存状态（重启重新计时），失败静默，不影响主流程。

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager, State};
use timetrace_core::games::stats;
use timetrace_core::{AppConfig, DataStore};

use crate::api::{GameEntryDto, GameLibraryResultDto, GameSnapshotDto, WatchedGameDto};
use crate::health::show_toast;
use crate::state::AppState;

const TICK_SECONDS: u64 = 5;

#[derive(Debug, Default)]
struct GameState {
    streak_started: Option<Instant>,
    streak_seconds: u64,
    reminders_today: u32,
    day: String,
}

pub struct GameTracker {
    state: Arc<Mutex<GameState>>,
}

impl GameTracker {
    /// 启动后台提醒线程并返回追踪器（供前端查询快照）。
    pub fn start(app: AppHandle) -> Arc<Self> {
        let tracker = Arc::new(Self {
            state: Arc::new(Mutex::new(GameState::default())),
        });
        let runner = tracker.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(TICK_SECONDS));
            runner.tick(&app);
        });
        tracker
    }

    fn tick(&self, app: &AppHandle) {
        let config = AppConfig::load();
        let reminder_minutes = config.games.games_reminder_minutes.clamp(15, 1440);
        let mut state = match self.state.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        let now = chrono::Local::now();
        let today = now.format("%Y-%m-%d").to_string();
        if state.day != today {
            state.day = today;
            state.reminders_today = 0;
        }
        if !config.games.games_reminder_enabled {
            state.streak_seconds = 0;
            state.streak_started = None;
            return;
        }

        let playing = {
            let Some(app_state) = app.try_state::<AppState>() else {
                return;
            };
            let Ok(api) = app_state.api.lock() else {
                return;
            };
            let games = visible_game_entries(&api.db());
            stats::current_game(&games, &*api.db()).is_some()
        };
        if !playing {
            // 没在玩游戏：连续计时归零。
            if state.streak_seconds > 0 || state.streak_started.is_some() {
                state.streak_seconds = 0;
                state.streak_started = None;
            }
            return;
        }

        let streak = state
            .streak_started
            .as_ref()
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0);
        state.streak_seconds = streak;

        if streak >= reminder_minutes * 60 {
            drop(state);
            show_toast(
                app,
                "数迹 · 游戏提醒",
                &format!("你已经连续游戏 {reminder_minutes} 分钟了，休息一下吧。"),
            );
            if let Ok(mut state) = self.state.lock() {
                state.reminders_today += 1;
                state.streak_seconds = 0;
                state.streak_started = Some(Instant::now());
            }
        } else if state.streak_started.is_none() {
            state.streak_started = Some(Instant::now());
        }
    }

    /// 前端快照：当前游戏 / 连续时长 / 今日游戏时长 / 下次提醒倒计时。
    pub fn snapshot(&self, app: &AppHandle) -> GameSnapshotDto {
        let config = AppConfig::load();
        let reminder_minutes = config.games.games_reminder_minutes.clamp(15, 1440);
        let (current_game, today_seconds) = {
            let Some(app_state) = app.try_state::<AppState>() else {
                return GameSnapshotDto {
                    enabled: config.games.games_reminder_enabled,
                    reminder_minutes,
                    current_game: None,
                    streak_seconds: 0,
                    today_seconds: 0,
                    reminders_today: 0,
                    next_reminder_seconds: reminder_minutes as i64 * 60,
                };
            };
            let Ok(api) = app_state.api.lock() else {
                return GameSnapshotDto {
                    enabled: config.games.games_reminder_enabled,
                    reminder_minutes,
                    current_game: None,
                    streak_seconds: 0,
                    today_seconds: 0,
                    reminders_today: 0,
                    next_reminder_seconds: reminder_minutes as i64 * 60,
                };
            };
            let games = visible_game_entries(&api.db());
            let current = stats::current_game(&games, &*api.db()).map(|g| g.title.clone());
            let today_total: i64 = stats::game_stats_today(&*api.db(), &games)
                .iter()
                .map(|s| s.seconds)
                .sum();
            (current, today_total)
        };
        let state = match self.state.lock() {
            Ok(s) => s,
            Err(_) => {
                return GameSnapshotDto {
                    enabled: config.games.games_reminder_enabled,
                    reminder_minutes,
                    current_game,
                    streak_seconds: 0,
                    today_seconds,
                    reminders_today: 0,
                    next_reminder_seconds: reminder_minutes as i64 * 60,
                };
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
        GameSnapshotDto {
            enabled: config.games.games_reminder_enabled,
            reminder_minutes,
            current_game,
            streak_seconds: streak,
            today_seconds,
            reminders_today: state.reminders_today,
            next_reminder_seconds: next,
        }
    }
}

#[tauri::command]
pub fn get_game_snapshot(app: AppHandle, tracker: State<'_, Arc<GameTracker>>) -> GameSnapshotDto {
    tracker.snapshot(&app)
}

/// 从 AppState 克隆数据库句柄（短锁，守卫在语句结束即释放）。
fn clone_db(app: &AppHandle) -> Option<Arc<timetrace_core::SqliteStore>> {
    app.try_state::<AppState>()?
        .api
        .lock()
        .ok()
        .map(|api| api.db())
}

/// 忽略旧版本无条件写入的 `source=known` 条目。
///
/// 内置名单现在只用于识别别名，不再代表已安装游戏；过滤兼容旧数据库，
/// 用户点击“刷新游戏库”后，`replace_non_manual_games` 会正式清理这些旧条目。
fn visible_game_entries(db: &timetrace_core::SqliteStore) -> Vec<timetrace_core::GameRow> {
    db.game_entries()
        .into_iter()
        .filter(|game| game.source != "known")
        .collect()
}

/// 游戏库列表（含今日/周/月/年/总时长）。统计在后台线程执行，不阻塞界面。
#[tauri::command]
pub async fn get_games_library(app: AppHandle) -> Vec<GameEntryDto> {
    let Some(db) = clone_db(&app) else {
        return Vec::new();
    };
    tauri::async_runtime::spawn_blocking(move || {
        let games = visible_game_entries(&db);
        let periods = stats::game_stats_periods(&*db, &games);
        games
            .into_iter()
            .map(|g| GameEntryDto {
                id: g.id,
                title: g.title.clone(),
                exe_path: g.exe_path,
                source: g.source,
                today_seconds: periods.today.get(&g.title).copied().unwrap_or(0),
                week_seconds: periods.week.get(&g.title).copied().unwrap_or(0),
                month_seconds: periods.month.get(&g.title).copied().unwrap_or(0),
                year_seconds: periods.year.get(&g.title).copied().unwrap_or(0),
                total_seconds: periods.total.get(&g.title).copied().unwrap_or(0),
                watched: g.watched,
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

/// 重新扫描平台游戏库（Steam/Epic/WeGame/米哈游 + 内置名单），保留手动条目。
/// 扫描在后台线程执行。
#[tauri::command]
pub async fn refresh_game_library(app: AppHandle) -> GameLibraryResultDto {
    let Some(db) = clone_db(&app) else {
        return GameLibraryResultDto {
            ok: false,
            found: 0,
            message: Some("数据库不可用".to_string()),
        };
    };
    tauri::async_runtime::spawn_blocking(move || {
        let found = timetrace_core::games::scan_all_platforms();
        let entries: Vec<(String, String, String, String, Option<String>)> = found
            .iter()
            .map(|g| {
                (
                    g.title.clone(),
                    g.exe_path.clone(),
                    g.app_name.clone(),
                    g.source.to_string(),
                    g.appid.clone(),
                )
            })
            .collect();
        let written = db.replace_non_manual_games(&entries);
        GameLibraryResultDto {
            ok: true,
            found: written,
            message: Some(format!("扫描完成：发现 {} 个游戏（保留手动条目）", written)),
        }
    })
    .await
    .unwrap_or_else(|_| GameLibraryResultDto {
        ok: false,
        found: 0,
        message: Some("扫描线程异常".to_string()),
    })
}

/// 手动添加一个游戏（名称 + exe 路径）。
#[tauri::command]
pub fn add_game_manual(
    state: State<'_, AppState>,
    title: String,
    exe_path: String,
) -> GameLibraryResultDto {
    let title = title.trim().to_string();
    let exe_path = exe_path.trim().to_string();
    if title.is_empty() || exe_path.is_empty() {
        return GameLibraryResultDto {
            ok: false,
            found: 0,
            message: Some("游戏名称与路径不能为空".to_string()),
        };
    }
    let api = crate::api::lock(&state);
    let id = api
        .db()
        .insert_game_entry(&title, &exe_path, "", "manual", None);
    if id > 0 {
        GameLibraryResultDto {
            ok: true,
            found: 1,
            message: None,
        }
    } else {
        GameLibraryResultDto {
            ok: false,
            found: 0,
            message: Some("写入失败".to_string()),
        }
    }
}

/// 移除一个游戏条目（含手动条目）。
#[tauri::command]
pub fn remove_game(state: State<'_, AppState>, id: i64) -> GameLibraryResultDto {
    let api = crate::api::lock(&state);
    if api.db().delete_game_entry(id) {
        GameLibraryResultDto {
            ok: true,
            found: 0,
            message: None,
        }
    } else {
        GameLibraryResultDto {
            ok: false,
            found: 0,
            message: Some("未找到该条目".to_string()),
        }
    }
}

/// 关注/取消关注一个游戏（按 game id 定位 title，关注状态跨刷新保留）。
#[tauri::command]
pub fn set_game_watched(state: State<'_, AppState>, id: i64, watched: bool) -> bool {
    let api = crate::api::lock(&state);
    let title = api
        .db()
        .game_entries()
        .into_iter()
        .find(|g| g.id == id)
        .map(|g| g.title.clone());
    if let Some(title) = title {
        api.db().set_game_watched(&title, watched);
        true
    } else {
        false
    }
}

/// 关注的游戏今日状态：是否启动 + 今日游玩秒数。
/// 在后台线程聚合（避免阻塞界面）。
#[tauri::command]
pub async fn get_watched_games_today(app: AppHandle) -> Vec<WatchedGameDto> {
    let Some(db) = clone_db(&app) else {
        return Vec::new();
    };
    tauri::async_runtime::spawn_blocking(move || {
        let games = visible_game_entries(&db);
        let periods = stats::game_stats_periods(&*db, &games);
        games
            .into_iter()
            .filter(|g| g.watched)
            .map(|g| {
                let today = periods.today.get(&g.title).copied().unwrap_or(0);
                WatchedGameDto {
                    title: g.title.clone(),
                    exe_path: g.exe_path,
                    launched_today: today > 0,
                    today_seconds: today,
                }
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}
