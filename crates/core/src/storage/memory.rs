//! 测试用内存 DataStore（从 sqlite.rs 拆出，行为不变）。

use std::sync::Mutex;

use chrono::{DateTime, NaiveDate, Utc};

use crate::contracts::{
    AppMetaRecord, AppUsageSplit, AppUsageSummary, DataStore, GameRow, SessionRecord,
    StartupEntryRecord,
};
// ── In-memory store for testing ──

/// In-memory implementation of `DataStore` for unit tests and UI prototyping.
pub struct MemoryStore {
    sessions: Mutex<Vec<SessionRecord>>,
    /// 汇总缓存（当前实现不使用，保留以对齐 SqliteStore 语义）。
    #[allow(dead_code)]
    summaries: Mutex<Vec<(String, NaiveDate, i64, i64)>>, // app_name, date, seconds, count
    startups: Mutex<Vec<StartupEntryRecord>>,
    metas: Mutex<Vec<AppMetaRecord>>,
    games: Mutex<Vec<GameRow>>,
    next_id: Mutex<i64>,
    /// 关注游戏 title 列表（测试用，与 SqliteStore.watched_games 表语义对齐）。
    watched: Mutex<Vec<String>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self {
            sessions: Mutex::new(Vec::new()),
            summaries: Mutex::new(Vec::new()),
            startups: Mutex::new(Vec::new()),
            metas: Mutex::new(Vec::new()),
            games: Mutex::new(Vec::new()),
            next_id: Mutex::new(1),
            watched: Mutex::new(Vec::new()),
        }
    }
}

impl DataStore for MemoryStore {
    fn insert_session(&self, session: &SessionRecord) -> i64 {
        let mut sessions = self.sessions.lock().unwrap();
        let mut next_id = self.next_id.lock().unwrap();
        let id = *next_id;
        *next_id += 1;
        let mut s = session.clone();
        s.id = id;
        sessions.push(s);
        id
    }

    fn close_session(&self, id: i64, end_time: DateTime<Utc>) {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(s) = sessions.iter_mut().find(|s| s.id == id) {
            s.ended_at = Some(end_time);
            s.duration_secs = Some((end_time - s.started_at).num_seconds());
        }
    }

    fn cleanup_dangling_sessions(&self) {
        let mut sessions = self.sessions.lock().unwrap();
        sessions.retain(|s| s.duration_secs.is_some());
    }

    fn get_active_session(&self) -> Option<SessionRecord> {
        self.sessions
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|s| s.ended_at.is_none())
            .cloned()
    }

    fn get_sessions_by_date(&self, date: NaiveDate) -> Vec<SessionRecord> {
        self.sessions
            .lock()
            .unwrap()
            .iter()
            .filter(|s| s.date == date)
            .cloned()
            .collect()
    }

    fn get_sessions_by_range(&self, start: NaiveDate, end: NaiveDate) -> Vec<SessionRecord> {
        self.sessions
            .lock()
            .unwrap()
            .iter()
            .filter(|s| s.date >= start && s.date <= end)
            .cloned()
            .collect()
    }

    fn get_playable_sessions_by_range(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Vec<SessionRecord> {
        self.sessions
            .lock()
            .unwrap()
            .iter()
            .filter(|s| {
                s.date >= start && s.date <= end && !s.is_idle && s.duration_secs.unwrap_or(0) > 0
            })
            .cloned()
            .collect()
    }

    fn get_daily_summary(&self, _date: NaiveDate) -> Vec<AppUsageSummary> {
        vec![] // Simplified for testing; real logic in SqliteStore
    }

    fn get_top_apps(
        &self,
        _start: NaiveDate,
        _end: NaiveDate,
        _limit: usize,
    ) -> Vec<AppUsageSummary> {
        vec![]
    }

    fn get_usage_split(&self, _start: NaiveDate, _end: NaiveDate) -> Vec<AppUsageSplit> {
        vec![]
    }

    fn start_page_visit(
        &self,
        _session_id: i64,
        _app_name: &str,
        _title: Option<&str>,
        _date: NaiveDate,
    ) -> i64 {
        -1
    }

    fn close_page_visit(&self, _visit_id: i64, _end_time: DateTime<Utc>) {}

    fn get_window_titles(&self, app_name: &str, _date: NaiveDate) -> Vec<(String, i64)> {
        self.sessions
            .lock()
            .unwrap()
            .iter()
            .filter(|s| s.app_name == app_name && !s.is_idle)
            .filter_map(|s| {
                s.duration_secs
                    .map(|d| (s.window_title.clone().unwrap_or_default(), d))
            })
            .fold(std::collections::HashMap::new(), |mut acc, (title, dur)| {
                *acc.entry(title).or_insert(0) += dur;
                acc
            })
            .into_iter()
            .collect()
    }

    fn upsert_startup_entries(&self, entries: &[StartupEntryRecord]) {
        let mut startups = self.startups.lock().unwrap();
        for e in entries {
            startups.push(e.clone());
        }
    }

    fn get_all_startup_entries(&self) -> Vec<StartupEntryRecord> {
        self.startups.lock().unwrap().clone()
    }

    fn set_startup_enabled(
        &self,
        id: i64,
        enabled: bool,
        backup: Option<&str>,
        backup_path: Option<&str>,
    ) {
        let mut startups = self.startups.lock().unwrap();
        if let Some(e) = startups.iter_mut().find(|e| e.id == id) {
            e.enabled = enabled;
            if let Some(v) = backup {
                e.backup_value = Some(v.to_string());
            }
            if let Some(p) = backup_path {
                e.backup_path = Some(p.to_string());
            }
        }
    }

    fn get_app_meta(&self, exe_path: &str) -> Option<AppMetaRecord> {
        self.metas
            .lock()
            .unwrap()
            .iter()
            .find(|m| m.app_path == exe_path)
            .cloned()
    }

    fn set_app_meta(&self, meta: &AppMetaRecord) {
        let mut metas = self.metas.lock().unwrap();
        if let Some(existing) = metas.iter_mut().find(|m| m.app_path == meta.app_path) {
            *existing = meta.clone();
        } else {
            metas.push(meta.clone());
        }
    }

    fn game_entries(&self) -> Vec<GameRow> {
        let games = self.games.lock().unwrap();
        let watched = self.watched.lock().unwrap();
        games
            .iter()
            .map(|g| {
                let mut g = g.clone();
                g.watched = watched.iter().any(|w| w == &g.title);
                g
            })
            .collect()
    }

    fn set_game_watched(&self, title: &str, watched: bool) {
        let mut list = self.watched.lock().unwrap();
        if watched {
            if !list.iter().any(|w| w == title) {
                list.push(title.to_string());
            }
        } else {
            list.retain(|w| w != title);
        }
    }

    fn watched_game_titles(&self) -> Vec<String> {
        self.watched.lock().unwrap().clone()
    }

    fn insert_game_entry(
        &self,
        title: &str,
        exe_path: &str,
        app_name: &str,
        source: &str,
        appid: Option<&str>,
    ) -> i64 {
        let mut games = self.games.lock().unwrap();
        let mut next_id = self.next_id.lock().unwrap();
        let id = *next_id;
        *next_id += 1;
        games.push(GameRow {
            id,
            title: title.to_string(),
            exe_path: exe_path.to_string(),
            app_name: app_name.to_string(),
            source: source.to_string(),
            appid: appid.map(|s| s.to_string()),
            watched: false,
        });
        id
    }

    fn delete_game_entry(&self, id: i64) -> bool {
        let mut games = self.games.lock().unwrap();
        let before = games.len();
        games.retain(|g| g.id != id);
        games.len() != before
    }

    fn replace_non_manual_games(
        &self,
        entries: &[(String, String, String, String, Option<String>)],
    ) -> usize {
        let mut games = self.games.lock().unwrap();
        let mut next_id = self.next_id.lock().unwrap();
        games.retain(|g| g.source == "manual");
        let mut written = 0usize;
        for (title, exe_path, app_name, source, appid) in entries {
            let id = *next_id;
            *next_id += 1;
            games.push(GameRow {
                id,
                title: title.clone(),
                exe_path: exe_path.clone(),
                app_name: app_name.clone(),
                source: source.clone(),
                appid: appid.clone(),
                watched: false,
            });
            written += 1;
        }
        written
    }

    fn recording_started_at(&self) -> Option<DateTime<Utc>> {
        self.sessions
            .lock()
            .unwrap()
            .iter()
            .min_by_key(|s| s.started_at)
            .map(|s| s.started_at)
    }

    fn total_tracked_seconds(&self) -> i64 {
        self.sessions
            .lock()
            .unwrap()
            .iter()
            .filter(|s| !s.is_idle)
            .filter_map(|s| s.duration_secs)
            .sum()
    }

    fn total_tracked_in_range(&self, start: NaiveDate, end: NaiveDate) -> i64 {
        self.sessions
            .lock()
            .unwrap()
            .iter()
            .filter(|s| !s.is_idle && s.date >= start && s.date <= end)
            .filter_map(|s| s.duration_secs)
            .sum()
    }

    fn cleanup_old_sessions(&self, before: NaiveDate) {
        self.sessions.lock().unwrap().retain(|s| s.date >= before);
    }

    fn vacuum(&self) {}

    fn clear_all_data(&self) {
        self.sessions.lock().unwrap().clear();
    }

    fn export_rows(&self, _start: NaiveDate, _end: NaiveDate) -> Vec<(String, String, i64, i64)> {
        vec![]
    }

    fn get_active_by_date(&self, start: NaiveDate, end: NaiveDate) -> Vec<(String, i64)> {
        let mut out: Vec<(String, i64)> = Vec::new();
        let mut last = String::new();
        let mut acc: i64 = 0;
        let sessions = self.sessions.lock().unwrap();
        let mut sorted: Vec<&SessionRecord> = sessions
            .iter()
            .filter(|s| !s.is_idle && s.date >= start && s.date <= end)
            .collect();
        sorted.sort_by_key(|s| s.date);
        for s in sorted {
            let d = s.date.to_string();
            let dur = s.duration_secs.unwrap_or(0);
            if d == last {
                acc += dur;
            } else {
                if !last.is_empty() {
                    out.push((last, acc));
                }
                last = d;
                acc = dur;
            }
        }
        if !last.is_empty() {
            out.push((last, acc));
        }
        out
    }

    fn get_diary_entries(&self, _start: NaiveDate, _end: NaiveDate) -> Vec<(String, String)> {
        vec![]
    }

    fn get_diary(&self, _date: NaiveDate) -> Option<String> {
        None
    }

    fn set_diary(&self, _date: NaiveDate, content: &str) -> String {
        content.to_string()
    }

    fn get_diary_entries_detailed(
        &self,
        _start: NaiveDate,
        _end: NaiveDate,
    ) -> Vec<(i64, String, String, String)> {
        vec![]
    }

    fn save_diary_draft(&self, _date: NaiveDate, _content: &str) -> i64 {
        1
    }

    fn publish_diary(&self, _date: NaiveDate, _content: &str) -> i64 {
        1
    }

    fn get_diary_draft(&self, _date: NaiveDate) -> Option<String> {
        None
    }

    fn add_diary_entry(&self, _date: NaiveDate, _content: &str) -> i64 {
        1
    }

    fn update_diary_entry(&self, _id: i64, _content: &str) -> Result<(), String> {
        Ok(())
    }

    fn delete_diary_entry(&self, _id: i64) -> Result<(), String> {
        Ok(())
    }

    fn get_day_sessions(&self, _date: NaiveDate) -> Vec<(String, bool, i64, String)> {
        vec![]
    }

    fn get_day_hourly(&self, _date: NaiveDate) -> Vec<i64> {
        vec![0; 24]
    }

    fn get_hour_apps(&self, _date: NaiveDate, _hour: u32) -> Vec<(String, i64)> {
        vec![]
    }

    fn get_day_hour_apps(&self, _date: NaiveDate) -> Vec<Vec<(String, i64)>> {
        vec![vec![]; 24]
    }

    fn get_app_hourly(&self, _app_name: &str, _date: NaiveDate) -> Vec<i64> {
        vec![0; 24]
    }

    fn get_diary_images(&self, _start: NaiveDate, _end: NaiveDate) -> Vec<(String, String)> {
        vec![]
    }

    fn get_diary_images_detailed(
        &self,
        _start: NaiveDate,
        _end: NaiveDate,
    ) -> Vec<(String, Option<i64>, String)> {
        vec![]
    }

    fn add_diary_image(&self, _date: NaiveDate, path: &str) -> String {
        path.to_string()
    }

    fn set_diary_image_entry(&self, _path: &str, _entry_id: i64) -> Result<(), String> {
        Ok(())
    }

    fn get_diary_images_for_entry(&self, _entry_id: i64) -> Vec<String> {
        vec![]
    }

    fn remove_diary_image(&self, _path: &str) {}
}
