//! 自 api.rs 按域拆分（纯搬迁，行为不变）。
use super::*;
use timetrace_core::*;
impl TimeTraceApi {
    /// One-call dashboard payload: usage split + overall stats.
    pub fn get_dashboard_data(&self, start: String, end: String) -> DashboardDataDto {
        let s = parse_date(&start);
        let e = parse_date(&end);
        let split = DataStore::get_usage_split(&*self.db, s, e);
        let active: i64 = split.iter().map(|x| x.active_seconds).sum();
        let idle: i64 = split.iter().map(|x| x.idle_seconds).sum();
        DashboardDataDto {
            apps: split
                .into_iter()
                .map(|x| AppUsageDto {
                    app_name: x.app_name,
                    active_seconds: x.active_seconds,
                    idle_seconds: x.idle_seconds,
                    exe_path: x.exe_path,
                })
                .collect(),
            active_seconds: active,
            idle_seconds: idle,
            total_seconds: DataStore::total_tracked_seconds(&*self.db),
            since: DataStore::recording_started_at(&*self.db)
                .map(|t| t.format("%Y-%m-%d").to_string()),
        }
    }

    /// Per-app active/idle split for a date range (dates as "YYYY-MM-DD").
    pub fn get_usage_split(&self, start: String, end: String) -> Vec<AppUsageDto> {
        let s = parse_date(&start);
        let e = parse_date(&end);
        DataStore::get_usage_split(&*self.db, s, e)
            .into_iter()
            .map(|x| AppUsageDto {
                app_name: x.app_name,
                active_seconds: x.active_seconds,
                idle_seconds: x.idle_seconds,
                exe_path: x.exe_path,
            })
            .collect()
    }

    /// Page-level breakdown for an app on a date.
    pub fn get_window_titles(&self, app_name: String, date: String) -> Vec<PageDto> {
        DataStore::get_window_titles(&*self.db, &app_name, parse_date(&date))
            .into_iter()
            .map(|(title, seconds)| PageDto { title, seconds })
            .collect()
    }

    /// Overall recording statistics.
    pub fn get_stats(&self, start: String, end: String) -> StatsDto {
        let s = parse_date(&start);
        let e = parse_date(&end);
        let split = DataStore::get_usage_split(&*self.db, s, e);
        let active: i64 = split.iter().map(|x| x.active_seconds).sum();
        let idle: i64 = split.iter().map(|x| x.idle_seconds).sum();
        StatsDto {
            active_seconds: active,
            idle_seconds: idle,
            total_seconds: DataStore::total_tracked_seconds(&*self.db),
            since: DataStore::recording_started_at(&*self.db)
                .map(|t| t.format("%Y-%m-%d").to_string()),
        }
    }

    /// Active seconds for this week (Mon→today) and last week (full).
    pub fn get_week_totals(&self) -> (i64, i64) {
        let today = chrono::Local::now().date_naive();
        let weekday = chrono::Datelike::weekday(&today).num_days_from_monday() as i64;
        let this_monday = today - chrono::Duration::days(weekday);
        let last_monday = this_monday - chrono::Duration::days(7);
        let this_week = DataStore::total_tracked_in_range(&*self.db, this_monday, today);
        let last_week = DataStore::total_tracked_in_range(
            &*self.db,
            last_monday,
            this_monday - chrono::Duration::days(1),
        );
        (this_week, last_week)
    }

    /// Full day detail: active/idle totals, session timeline, diary.
    pub fn get_day_detail(&self, date: String) -> DayDetailDto {
        let d = parse_date(&date);
        let sessions = DataStore::get_day_sessions(&*self.db, d);
        let mut active = 0i64;
        let mut idle = 0i64;
        let mut dtos = Vec::with_capacity(sessions.len());
        for (app, is_idle, dur, started) in sessions {
            if is_idle {
                idle += dur;
            } else {
                active += dur;
            }
            dtos.push(DaySessionDto {
                app_name: app,
                is_idle,
                duration_secs: dur,
                started_at: started,
            });
        }
        DayDetailDto {
            date,
            active_seconds: active,
            idle_seconds: idle,
            session_count: dtos.len() as i64,
            diary: DataStore::get_diary(&*self.db, d).unwrap_or_default(),
            sessions: dtos,
        }
    }

    /// Hourly active-seconds for a day (24 buckets) — for the heatmap.
    pub fn get_day_hourly(&self, date: String) -> Vec<i64> {
        DataStore::get_day_hourly(&*self.db, parse_date(&date))
    }

    /// 全年每日活跃秒数（热力图数据，按天聚合，轻量查询）。
    pub fn get_year_heatmap(&self, year: i32) -> Vec<(String, i64)> {
        let start = chrono::NaiveDate::from_ymd_opt(year, 1, 1)
            .unwrap_or_else(|| chrono::Local::now().date_naive());
        let end = chrono::NaiveDate::from_ymd_opt(year, 12, 31).unwrap_or(start);
        DataStore::get_active_by_date(&*self.db, start, end)
    }

    /// 当前进行中的活跃会话已持续秒数（无活跃会话或处于空闲时返回 0），
    /// 供仪表盘「使用时间」秒级跳动；空闲/离开时自动停止增长。
    pub fn get_active_session_elapsed(&self) -> i64 {
        if let Some(s) = DataStore::get_active_session(&*self.db) {
            if !s.is_idle {
                return (chrono::Utc::now() - s.started_at).num_seconds().max(0);
            }
        }
        0
    }

    /// Apps active within a specific hour of a date (seconds per app).
    pub fn get_hour_apps(&self, date: String, hour: u32) -> Vec<AppUsageDto> {
        DataStore::get_hour_apps(&*self.db, parse_date(&date), hour)
            .into_iter()
            .map(|(app_name, secs)| AppUsageDto {
                app_name,
                active_seconds: secs,
                idle_seconds: 0,
                exe_path: String::new(),
            })
            .collect()
    }

    /// Hourly active-seconds for one app on a date (24 buckets).
    pub fn get_app_hourly(&self, app_name: String, date: String) -> Vec<i64> {
        DataStore::get_app_hourly(&*self.db, &app_name, parse_date(&date))
    }

    /// Clear ALL tracked usage data (sessions + page visits).
    pub fn clear_data(&self) {
        tracing::info!("Clearing all usage data");
        DataStore::clear_all_data(&*self.db);
    }

    /// Export usage data for a date range as CSV.
    /// Returns the CSV text (app, date, active_secs, idle_secs).
    pub fn export_csv(&self, start: String, end: String) -> String {
        let s = parse_date(&start);
        let e = parse_date(&end);
        let rows = DataStore::export_rows(&*self.db, s, e);
        let mut csv = String::from("app,date,active_secs,idle_secs\n");
        for (app, date, active, idle) in rows {
            csv.push_str(&format!(
                "{},{},{},{}\n",
                csv_escape(&app),
                csv_escape(&date),
                active,
                idle
            ));
        }
        csv
    }

    /// 导出敏感数据明文（日记 + 窗口标题）到 export 目录，供用户自行备份。
    /// 文件含明文内容，仅在本机生成，位置会提示用户保管。
    pub fn export_plaintext(&self) -> ExportResultDto {
        let dir = dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("TimeTrace")
            .join("export");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return ExportResultDto {
                ok: false,
                path: None,
                message: Some(format!("无法创建导出目录：{e}")),
            };
        }
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let path = dir.join(format!("数迹明文导出-{stamp}.json"));
        let payload = serde_json::json!({
            "exported_at": chrono::Local::now().to_rfc3339(),
            "app": "数迹",
            "note": "此文件为明文备份，包含日记与窗口标题，请妥善保管。",
            "data": self.db.dump_sensitive_plaintext(),
        });
        match serde_json::to_string_pretty(&payload) {
            Ok(json) => match std::fs::write(&path, json) {
                Ok(_) => ExportResultDto {
                    ok: true,
                    path: Some(path.to_string_lossy().to_string()),
                    message: None,
                },
                Err(e) => ExportResultDto {
                    ok: false,
                    path: None,
                    message: Some(format!("写入导出文件失败：{e}")),
                },
            },
            Err(e) => ExportResultDto {
                ok: false,
                path: None,
                message: Some(format!("生成导出内容失败：{e}")),
            },
        }
    }

    /// 导出使用数据 CSV（应用名/日期/活跃秒/空闲秒，全量）到 export 目录。
    /// 不含日记与窗口标题（那些走明文导出），CSV 本身不落盘敏感字段。
    pub fn export_usage_csv(&self) -> ExportResultDto {
        let dir = dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("TimeTrace")
            .join("export");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return ExportResultDto {
                ok: false,
                path: None,
                message: Some(format!("无法创建导出目录：{e}")),
            };
        }
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let path = dir.join(format!("数迹使用数据-{stamp}.csv"));
        let csv = self.export_csv("1970-01-01".to_string(), "2100-01-01".to_string());
        match std::fs::write(&path, csv) {
            Ok(_) => ExportResultDto {
                ok: true,
                path: Some(path.to_string_lossy().to_string()),
                message: None,
            },
            Err(e) => ExportResultDto {
                ok: false,
                path: None,
                message: Some(format!("写入导出文件失败：{e}")),
            },
        }
    }
}
