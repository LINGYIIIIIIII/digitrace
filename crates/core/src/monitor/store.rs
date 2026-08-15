//! 时间序列存储（SQLite，分钟级聚合，独立于 time.db）。
//!
//! 低占用设计：每秒采样只在内存累加（sum/max/count），每分钟一次事务批量
//! upsert；秒级明细不落盘；按保留天数自动清理（默认 90 天）。

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Duration, FixedOffset, Local, Timelike};
use rusqlite::{Connection, params};

const DEFAULT_RETENTION_DAYS: u64 = 90;

/// 一个聚合样本（图表查询用）。
#[derive(Debug, Clone)]
pub struct Sample {
    pub day: String,
    pub minute: u32,
    pub avg: f64,
    pub max: f64,
    pub samples: u32,
}

#[derive(Debug, Default)]
struct MinuteAccum {
    sum: f64,
    max: f64,
    count: u32,
}

pub struct MetricStore {
    conn: Connection,
    retention_days: u64,
    day: String,
    minute: u32,
    buf: HashMap<String, MinuteAccum>,
}

impl MetricStore {
    /// 打开（或创建）历史数据库。
    pub fn open(path: PathBuf, retention_days: u64) -> rusqlite::Result<Self> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let conn = Connection::open(&path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS metric_samples (
                day     TEXT    NOT NULL,
                minute  INTEGER NOT NULL,
                metric  TEXT    NOT NULL,
                avg     REAL    NOT NULL,
                max     REAL    NOT NULL,
                samples INTEGER NOT NULL,
                PRIMARY KEY (day, minute, metric)
            );
            CREATE INDEX IF NOT EXISTS idx_metric_samples_day ON metric_samples(day);
            -- 一次性清理历史 DeepSeek 痕迹（旧库可能残留）。
            DROP TABLE IF EXISTS deepseek_usage;
            DELETE FROM metric_samples WHERE metric = 'deepseek_active';
            PRAGMA journal_mode=WAL;
            PRAGMA cache_size=-1024;
            -- 多进程（完整版 + 独立监控）并发写同一 monitor.db 时避免锁冲突。
            PRAGMA busy_timeout=5000;",
        )?;

        let retention_days = if retention_days == 0 {
            DEFAULT_RETENTION_DAYS
        } else {
            retention_days
        };
        let store = Self {
            conn,
            retention_days,
            day: String::new(),
            minute: 0,
            buf: HashMap::new(),
        };
        store.cleanup_old()?;
        Ok(store)
    }

    /// 记录一个采样值。按配置时区的"天 + 分钟"分桶，跨分钟自动 flush。
    pub fn record(&mut self, now: &DateTime<FixedOffset>, metric: &str, value: f64) {
        let day = now.format("%Y-%m-%d").to_string();
        let minute = now.hour() * 60 + now.minute();
        if day != self.day || minute != self.minute {
            let _ = self.flush();
            self.day = day;
            self.minute = minute;
        }
        let acc = self.buf.entry(metric.to_string()).or_default();
        acc.sum += value;
        acc.max = acc.max.max(value);
        acc.count += 1;
    }

    /// 把内存桶写入数据库（一个事务），并清空。
    pub fn flush(&mut self) -> rusqlite::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO metric_samples(day, minute, metric, avg, max, samples)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(day, minute, metric) DO UPDATE SET
                    avg = (metric_samples.avg * metric_samples.samples
                           + excluded.avg * excluded.samples)
                          / (metric_samples.samples + excluded.samples),
                    max = MAX(metric_samples.max, excluded.max),
                    samples = metric_samples.samples + excluded.samples",
            )?;
            for (metric, acc) in &self.buf {
                stmt.execute(params![
                    self.day,
                    self.minute,
                    metric,
                    acc.sum / acc.count as f64,
                    acc.max,
                    acc.count,
                ])?;
            }
        }
        tx.commit()?;
        self.buf.clear();
        Ok(())
    }

    /// 清空全部监控历史（网络分钟级）。
    pub fn clear(&mut self) -> rusqlite::Result<()> {
        self.flush()?;
        self.conn.execute_batch("DELETE FROM metric_samples;")?;
        Ok(())
    }

    /// 查询某个指标最近 `days` 天的分钟级样本（升序）。
    pub fn query_range(&self, metric: &str, days: u64) -> rusqlite::Result<Vec<Sample>> {
        let start_day = (Local::now() - Duration::days(days as i64))
            .format("%Y-%m-%d")
            .to_string();
        let mut stmt = self.conn.prepare(
            "SELECT day, minute, avg, max, samples
             FROM metric_samples
             WHERE metric = ?1 AND day >= ?2
             ORDER BY day, minute",
        )?;
        let rows = stmt.query_map(params![metric, start_day], |row| {
            Ok(Sample {
                day: row.get(0)?,
                minute: row.get(1)?,
                avg: row.get(2)?,
                max: row.get(3)?,
                samples: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    /// 查询某指标从 (start_day, start_minute) 起到 end_day 的分钟级样本
    /// （升序，跨天正确；用于 24h / 今日 / 本次启动 / 近 N 天窗口）。
    pub fn query_window(
        &self,
        metric: &str,
        start_day: &str,
        start_minute: u32,
        end_day: &str,
    ) -> rusqlite::Result<Vec<Sample>> {
        let mut stmt = self.conn.prepare(
            "SELECT day, minute, avg, max, samples
             FROM metric_samples
             WHERE metric = ?1 AND day <= ?4
               AND (day > ?2 OR (day = ?2 AND minute >= ?3))
             ORDER BY day, minute",
        )?;
        let rows = stmt.query_map(params![metric, start_day, start_minute, end_day], |row| {
            Ok(Sample {
                day: row.get(0)?,
                minute: row.get(1)?,
                avg: row.get(2)?,
                max: row.get(3)?,
                samples: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    /// 清理超过保留天数的旧数据。
    fn cleanup_old(&self) -> rusqlite::Result<()> {
        let cutoff = (Local::now() - Duration::days(self.retention_days as i64))
            .format("%Y-%m-%d")
            .to_string();
        self.conn
            .execute("DELETE FROM metric_samples WHERE day < ?1", params![cutoff])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp_path() -> PathBuf {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("tt_monitor_test_{}_{}.db", std::process::id(), n))
    }

    #[test]
    fn minute_aggregation_roundtrip() {
        let path = tmp_path();
        let mut store = MetricStore::open(path.clone(), 30).unwrap();
        let now = Local::now();
        let now_fixed = now.fixed_offset();
        store.record(&now_fixed, "net_down_bps", 10.0);
        store.record(&now_fixed, "net_down_bps", 30.0);
        store.flush().unwrap();
        let rows = store.query_range("net_down_bps", 1).unwrap();
        assert_eq!(rows.len(), 1);
        assert!((rows[0].avg - 20.0).abs() < 1e-9);
        assert_eq!(rows[0].max as u32, 30);
        assert_eq!(rows[0].samples, 2);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn query_window_crosses_days_and_minutes() {
        let path = tmp_path();
        let mut store = MetricStore::open(path.clone(), 30).unwrap();
        let tz = FixedOffset::east_opt(8 * 3600).unwrap();
        let t = |s: &str| {
            chrono::DateTime::parse_from_rfc3339(s)
                .unwrap()
                .with_timezone(&tz)
        };
        // 08-10 20:00、08-11 00:30、08-11 01:00（UTC+8）。
        store.record(&t("2026-08-10T20:00:00+08:00"), "net_down_bps", 1.0);
        store.record(&t("2026-08-11T00:30:00+08:00"), "net_down_bps", 2.0);
        store.record(&t("2026-08-11T01:00:00+08:00"), "net_down_bps", 3.0);
        store.flush().unwrap();

        // 从 08-11 00:00 起 → 只含 08-11 的两条。
        let rows = store
            .query_window("net_down_bps", "2026-08-11", 0, "2026-08-11")
            .unwrap();
        assert_eq!(rows.len(), 2);

        // 从 08-10 20:30 起跨天 → 08-10 20:00 被排除，剩 08-11 两条。
        let rows = store
            .query_window("net_down_bps", "2026-08-10", 20 * 60 + 30, "2026-08-11")
            .unwrap();
        assert_eq!(rows.len(), 2);

        // 从 08-11 00:31 起 → 只剩 01:00 一条。
        let rows = store
            .query_window("net_down_bps", "2026-08-11", 31, "2026-08-11")
            .unwrap();
        assert_eq!(rows.len(), 1);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn clear_removes_all_metrics() {
        let path = tmp_path();
        let mut store = MetricStore::open(path.clone(), 30).unwrap();
        let now = Local::now();
        store.record(&now.fixed_offset(), "net_down_bps", 10.0);
        store.flush().unwrap();
        store.clear().unwrap();
        assert!(store.query_range("net_down_bps", 1).unwrap().is_empty());
        let _ = std::fs::remove_file(&path);
    }
}
