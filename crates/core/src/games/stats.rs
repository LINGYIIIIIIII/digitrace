//! 游戏时长统计：按日期范围取会话 → 内存匹配游戏条目 → 聚合。
//!
//! 不修改写路径、不新增索引列：统计在 Rust 侧解密后完成，
//! 今天/本周/本月毫秒级；全量统计建议放后台线程。

use chrono::{Local, NaiveDate};

use crate::contracts::{DataStore, GameRow};

/// 一个游戏的时长统计。
#[derive(Debug, Clone)]
pub struct GameStat {
    pub title: String,
    /// 活跃秒数（不含 idle）。
    pub seconds: i64,
}

/// 日期范围内每个游戏的活跃秒数（降序）。
pub fn game_stats_in_range(
    db: &dyn DataStore,
    games: &[GameRow],
    start: NaiveDate,
    end: NaiveDate,
) -> Vec<GameStat> {
    let sessions = db.get_sessions_by_range(start, end);
    let mut acc: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for s in sessions {
        if s.is_idle {
            continue;
        }
        let Some(d) = s.duration_secs else {
            continue;
        };
        if d <= 0 {
            continue;
        }
        if let Some(row) = match_game(games, &s.app_path, &s.app_name) {
            *acc.entry(row.title.clone()).or_insert(0) += d;
        }
    }
    let mut out: Vec<GameStat> = acc
        .into_iter()
        .map(|(title, seconds)| GameStat { title, seconds })
        .collect();
    out.sort_by_key(|s| std::cmp::Reverse(s.seconds));
    out
}

/// 今日游戏时长统计。
pub fn game_stats_today(db: &dyn DataStore, games: &[GameRow]) -> Vec<GameStat> {
    let today = Local::now().date_naive();
    game_stats_in_range(db, games, today, today)
}

/// 全部历史游戏时长统计（供「总时长」列；数据量大时放后台线程）。
pub fn game_stats_all(db: &dyn DataStore, games: &[GameRow]) -> Vec<GameStat> {
    let start = db
        .recording_started_at()
        .map(|t| t.date_naive())
        .unwrap_or_else(|| Local::now().date_naive());
    game_stats_in_range(db, games, start, Local::now().date_naive())
}

/// 当前前台会话命中的游戏（用于「当前游戏」显示与连续提醒）。
pub fn current_game<'a>(games: &'a [GameRow], db: &dyn DataStore) -> Option<&'a GameRow> {
    let session = db.get_active_session()?;
    if session.is_idle {
        return None;
    }
    match_game(games, &session.app_path, &session.app_name)
}

fn match_game<'a>(games: &'a [GameRow], app_path: &str, app_name: &str) -> Option<&'a GameRow> {
    games
        .iter()
        .find(|g| crate::games::game_row_matches(g, app_path, app_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::SessionRecord;
    use crate::storage::sqlite::MemoryStore;
    use chrono::{DateTime, Utc};

    fn session(
        app_path: &str,
        app_name: &str,
        dur: i64,
        is_idle: bool,
        date: NaiveDate,
    ) -> SessionRecord {
        SessionRecord {
            id: 0,
            app_path: app_path.to_string(),
            app_name: app_name.to_string(),
            window_title: None,
            started_at: DateTime::<Utc>::from_timestamp(0, 0).unwrap(),
            ended_at: None,
            duration_secs: Some(dur),
            is_idle,
            date,
        }
    }

    #[test]
    fn aggregates_matching_games_only() {
        let db = MemoryStore::new();
        let today = Local::now().date_naive();
        db.insert_session(&session(
            r"D:\Steam\common\ELDEN RING\Game\eldenring.exe",
            "eldenring",
            3600,
            false,
            today,
        ));
        db.insert_session(&session(
            r"D:\Games\StarRail\StarRail.exe",
            "starrail",
            7200,
            false,
            today,
        ));
        // 非游戏应用不应计入
        db.insert_session(&session(
            r"C:\Users\x\AppData\Local\chrome.exe",
            "Edge",
            500,
            false,
            today,
        ));
        // idle 不应计入
        db.insert_session(&session(
            r"D:\Steam\common\ELDEN RING\Game\eldenring.exe",
            "eldenring",
            9999,
            true,
            today,
        ));

        let games = vec![
            GameRow {
                id: 1,
                title: "艾尔登法环".into(),
                exe_path: r"D:\Steam\common\ELDEN RING\Game\eldenring.exe".into(),
                app_name: "eldenring".into(),
                source: "steam".into(),
                appid: None,
            },
            GameRow {
                id: 2,
                title: "崩坏：星穹铁道".into(),
                exe_path: "starrail".into(),
                app_name: "starrail".into(),
                source: "known".into(),
                appid: None,
            },
        ];

        let stats = game_stats_in_range(&db, &games, today, today);
        assert_eq!(stats.len(), 2);
        assert_eq!(stats[0].title, "崩坏：星穹铁道");
        assert_eq!(stats[0].seconds, 7200);
        assert_eq!(stats[1].title, "艾尔登法环");
        assert_eq!(stats[1].seconds, 3600);
    }

    #[test]
    fn current_game_detects_active_session() {
        let db = MemoryStore::new();
        let today = Local::now().date_naive();
        let mut s = session(r"D:\Game\a.exe", "a", 10, false, today);
        s.ended_at = None;
        db.insert_session(&s);

        let games = vec![GameRow {
            id: 1,
            title: "A".into(),
            exe_path: r"D:\Game\a.exe".into(),
            app_name: "a".into(),
            source: "manual".into(),
            appid: None,
        }];
        assert!(current_game(&games, &db).is_some());
    }

    #[test]
    fn excludes_closed_sessions_from_current() {
        let db = MemoryStore::new();
        let today = Local::now().date_naive();
        let mut s = session(r"D:\Game\a.exe", "a", 10, false, today);
        s.ended_at = Some(DateTime::<Utc>::from_timestamp(100, 0).unwrap());
        db.insert_session(&s);
        let games = vec![GameRow {
            id: 1,
            title: "A".into(),
            exe_path: r"D:\Game\a.exe".into(),
            app_name: "a".into(),
            source: "manual".into(),
            appid: None,
        }];
        assert!(current_game(&games, &db).is_none());
    }

    #[test]
    fn excludes_idle_sessions_from_current() {
        let db = MemoryStore::new();
        let today = Local::now().date_naive();
        let s = session(r"D:\Game\a.exe", "a", 10, true, today);
        db.insert_session(&s);
        let games = vec![GameRow {
            id: 1,
            title: "A".into(),
            exe_path: r"D:\Game\a.exe".into(),
            app_name: "a".into(),
            source: "manual".into(),
            appid: None,
        }];
        assert!(current_game(&games, &db).is_none());
    }
}
