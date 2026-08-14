//! 统一日期时区工具：所有「日历日」统计（今天/本周/本月、历史窗口）按
//! `AppConfig.timezone` 计算（system=跟随系统，utc+8=东八区固定），
//! 采样写库与查询边界共用同一套逻辑，保证数据一致。

use chrono::{DateTime, Duration, FixedOffset, Local, Timelike, Utc};

/// 根据时区配置返回固定偏移（utc+8）或跟随系统的当前偏移。
pub fn configured_offset_for(timezone: &str) -> FixedOffset {
    if timezone == "utc+8" {
        FixedOffset::east_opt(8 * 3600).expect("invalid utc+8 offset")
    } else {
        *Local::now().offset()
    }
}

/// 配置时区下的当前时刻。
pub fn now_in_for(timezone: &str) -> DateTime<FixedOffset> {
    Utc::now().with_timezone(&configured_offset_for(timezone))
}

/// 把任意 UTC 时刻转成配置时区的 (day, minute)。
pub fn day_minute_at_for(timezone: &str, utc: DateTime<Utc>) -> (String, u32) {
    let t = utc.with_timezone(&configured_offset_for(timezone));
    let day = t.format("%Y-%m-%d").to_string();
    let minute = t.hour() * 60 + t.minute();
    (day, minute)
}

/// 当前配置时区下的 (day, minute)。
pub fn now_day_minute_for(timezone: &str) -> (String, u32) {
    day_minute_at_for(timezone, Utc::now())
}

/// 计算历史查询窗口边界：(起始日, 起始分钟, 结束日)。
/// mode：24h（最近 24 小时）/ today（当天 0 点起）/ session（本次启动起）/
/// 7d / 30d（最近 N 天）。
pub fn history_window_for(
    timezone: &str,
    mode: &str,
    started_at: DateTime<Utc>,
) -> (String, u32, String) {
    let now = now_day_minute_for(timezone);
    let now_utc = Utc::now();
    let start = match mode {
        "today" => (now.0.clone(), 0u32),
        "24h" => day_minute_at_for(timezone, now_utc - Duration::hours(24)),
        "session" => day_minute_at_for(timezone, started_at),
        "7d" => day_minute_at_for(timezone, now_utc - Duration::days(7)),
        "30d" => day_minute_at_for(timezone, now_utc - Duration::days(30)),
        _ => (now.0.clone(), 0u32),
    };
    (start.0, start.1, now.0)
}

/// AppConfig 便捷包装（避免调用方手动取 timezone 字段）。
pub fn configured_offset(config: &crate::AppConfig) -> FixedOffset {
    configured_offset_for(&config.timezone)
}

pub fn now_in(config: &crate::AppConfig) -> DateTime<FixedOffset> {
    now_in_for(&config.timezone)
}

pub fn now_day_minute(config: &crate::AppConfig) -> (String, u32) {
    now_day_minute_for(&config.timezone)
}

pub fn history_window(
    config: &crate::AppConfig,
    mode: &str,
    started_at: DateTime<Utc>,
) -> (String, u32, String) {
    history_window_for(&config.timezone, mode, started_at)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc8_boundary_is_stable() {
        // UTC 2026-08-10 17:30 → UTC+8 = 2026-08-11 01:30 → day 应为 08-11，minute 90。
        let utc = DateTime::parse_from_rfc3339("2026-08-10T17:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let (day, minute) = day_minute_at_for("utc+8", utc);
        assert_eq!(day, "2026-08-11");
        assert_eq!(minute, 90);
    }

    #[test]
    fn utc8_midnight_crosses_day() {
        // UTC 2026-08-10 15:59 → UTC+8 = 08-10 23:59；16:00 → 08-11 00:00。
        let before = DateTime::parse_from_rfc3339("2026-08-10T15:59:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let after = DateTime::parse_from_rfc3339("2026-08-10T16:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(day_minute_at_for("utc+8", before).0, "2026-08-10");
        assert_eq!(day_minute_at_for("utc+8", after).0, "2026-08-11");
        assert_eq!(day_minute_at_for("utc+8", after).1, 0);
    }

    #[test]
    fn history_window_modes() {
        let started = DateTime::parse_from_rfc3339("2026-08-10T02:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let (sday, smin, eday) = history_window_for("utc+8", "today", started);
        assert_eq!(sday, eday);
        assert_eq!(smin, 0);
        let (sday, smin, _) = history_window_for("utc+8", "session", started);
        assert_eq!(sday, "2026-08-10");
        assert_eq!(smin, 10 * 60); // 02:00Z = 10:00 +8
    }
}
