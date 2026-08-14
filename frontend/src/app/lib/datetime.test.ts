import { describe, expect, it } from 'vitest';
import { dateStrInTz, shiftDayStr, timezoneOffsetMs, todayStr, weekdayOf } from './datetime';

describe('datetime 日期时区工具', () => {
  it('utc+8 偏移为 8 小时', () => {
    expect(timezoneOffsetMs({ timezone: 'utc+8' })).toBe(8 * 3600 * 1000);
  });

  it('utc+8 时区下 UTC 23:30 属于第二天', () => {
    // 2026-08-14T23:30:00Z = 2026-08-15 07:30 (+8)
    const d = new Date('2026-08-14T23:30:00Z');
    expect(dateStrInTz(d, { timezone: 'utc+8' })).toBe('2026-08-15');
  });

  it('utc+8 时区下 UTC 00:30 属于当天', () => {
    const d = new Date('2026-08-14T00:30:00Z');
    expect(dateStrInTz(d, { timezone: 'utc+8' })).toBe('2026-08-14');
  });

  it('shiftDayStr 加减天数跨月正确', () => {
    expect(shiftDayStr('2026-08-31', 1)).toBe('2026-09-01');
    expect(shiftDayStr('2026-03-01', -1)).toBe('2026-02-28');
  });

  it('weekdayOf 周日起点为 0', () => {
    // 2026-08-16 是周日
    expect(weekdayOf('2026-08-16')).toBe(0);
    // 2026-08-17 是周一
    expect(weekdayOf('2026-08-17')).toBe(1);
  });

  it('todayStr 返回有效 YYYY-MM-DD', () => {
    expect(todayStr({ timezone: 'utc+8' })).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });
});
