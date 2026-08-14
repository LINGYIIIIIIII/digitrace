// 统一日期时区工具：所有「日历日」边界（今天/昨天/本周/本月、历史窗口）按
// AppConfig.timezone 计算（system=跟随系统，utc+8=东八区固定），与 Rust 侧一致。

/** 配置时区相对 UTC 的偏移毫秒。 */
export function timezoneOffsetMs(
  config: { timezone?: string } | null | undefined,
): number {
  if (config?.timezone === 'utc+8') return 8 * 3600 * 1000;
  return -new Date().getTimezoneOffset() * 60000;
}

/** 把任意时刻转成配置时区下的 YYYY-MM-DD。 */
export function dateStrInTz(
  date: Date,
  config: { timezone?: string } | null | undefined,
): string {
  return new Date(date.getTime() + timezoneOffsetMs(config)).toISOString().slice(0, 10);
}

/** 配置时区下的今天。 */
export function todayStr(config: { timezone?: string } | null | undefined): string {
  return dateStrInTz(new Date(), config);
}

/** 日期字符串加减天数（UTC 层面运算，与时区无关）。 */
export function shiftDayStr(day: string, delta: number): string {
  const d = new Date(`${day}T00:00:00Z`);
  d.setUTCDate(d.getUTCDate() + delta);
  return d.toISOString().slice(0, 10);
}

/** 日期字符串在配置时区下的星期几（0=周日）。 */
export function weekdayOf(day: string): number {
  return new Date(`${day}T00:00:00Z`).getUTCDay();
}
