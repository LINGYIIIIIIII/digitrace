'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';
import { AnimatePresence, motion } from 'framer-motion';
import { CalendarRange, ChevronLeft, ChevronRight, LocateFixed, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { apiService } from '../services/api';
import { useAppStore } from '../store/app-store';
import type { DayDetailDto } from '../types';
import { todayStr } from '../lib/datetime';
import { Card } from './ui/index';

const WEEK_LABELS = ['1', '2', '3', '4', '5', '6', '日'];

function formatDuration(seconds: number): string {
  if (!seconds || seconds <= 0) return '0 秒';
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  if (h > 0) return `${h} 小时 ${m} 分 ${s} 秒`;
  if (m > 0) return `${m} 分 ${s} 秒`;
  return `${s} 秒`;
}

function heatColor(seconds: number, max: number): string {
  if (seconds <= 0 || max <= 0) return 'bg-card hover:bg-accent';
  const ratio = seconds / max;
  if (ratio < 0.25) return 'bg-primary/15 hover:bg-primary/25';
  if (ratio < 0.5) return 'bg-primary/30 hover:bg-primary/40';
  if (ratio < 0.75) return 'bg-primary/55 hover:bg-primary/65';
  return 'bg-primary/85 hover:bg-primary';
}

export default function CalendarPage() {
  const { t } = useTranslation();
  const { config } = useAppStore(useShallow((s) => ({ config: s.config })));
  const today = todayStr(config);
  const [year, setYear] = useState(() => Number(today.slice(0, 4)));
  const [month, setMonth] = useState(() => Number(today.slice(5, 7)) - 1); // 0-11
  const [view, setView] = useState<'month' | 'year'>('month');
  const [usage, setUsage] = useState<Map<string, number>>(new Map());
  const [max, setMax] = useState(0);
  const [detail, setDetail] = useState<DayDetailDto | null>(null);

  useEffect(() => {
    let cancelled = false;
    apiService
      .exportCsv(`${year}-01-01`, `${year}-12-31`)
      .then((csv) => {
        if (cancelled) return;
        const map = new Map<string, number>();
        const re = /,(\d{4}-\d{2}-\d{2}),(\d+),(\d+)/g;
        let match: RegExpExecArray | null;
        let m = 0;
        while ((match = re.exec(csv)) !== null) {
          const active = Number(match[2]) || 0;
          const total = (map.get(match[1]) ?? 0) + active;
          map.set(match[1], total);
          if (total > m) m = total;
        }
        setUsage(map);
        setMax(m);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [year]);

  const days = useMemo(() => {
    const first = new Date(Date.UTC(year, month, 1));
    const daysInMonth = new Date(Date.UTC(year, month + 1, 0)).getUTCDate();
    const lead = (first.getUTCDay() + 6) % 7;
    const prefix = `${year}-${String(month + 1).padStart(2, '0')}-`;
    const cells: (string | null)[] = Array.from({ length: lead }, () => null);
    for (let d = 1; d <= daysInMonth; d += 1) {
      cells.push(`${prefix}${String(d).padStart(2, '0')}`);
    }
    return cells;
  }, [year, month]);

  // 年视图：12 个月，每月迷你日历 + 合计时长。
  const yearMonths = useMemo(
    () =>
      Array.from({ length: 12 }, (_, m) => {
        const first = new Date(Date.UTC(year, m, 1));
        const daysInMonth = new Date(Date.UTC(year, m + 1, 0)).getUTCDate();
        const lead = (first.getUTCDay() + 6) % 7;
        const prefix = `${year}-${String(m + 1).padStart(2, '0')}-`;
        const cells: (string | null)[] = Array.from({ length: lead }, () => null);
        let total = 0;
        for (let d = 1; d <= daysInMonth; d += 1) {
          const dateStr = `${prefix}${String(d).padStart(2, '0')}`;
          cells.push(dateStr);
          total += usage.get(dateStr) ?? 0;
        }
        return { month: m, cells, total };
      }),
    [year, usage],
  );

  const handleSelectDate = useCallback(async (date: string) => {
    try {
      setDetail(await apiService.getDayDetail(date));
    } catch {
      setDetail(null);
    }
  }, []);

  const shiftMonth = useCallback(
    (delta: number) => {
      const next = new Date(Date.UTC(year, month + delta, 1));
      setYear(next.getUTCFullYear());
      setMonth(next.getUTCMonth());
      setDetail(null);
    },
    [year, month],
  );

  const shiftYear = useCallback(
    (delta: number) => {
      setYear((y) => y + delta);
      setDetail(null);
    },
    [],
  );

  const goToMonth = useCallback((m: number) => {
    setMonth(m);
    setView('month');
    setDetail(null);
  }, []);

  const goToToday = useCallback(() => {
    const d = todayStr(config);
    setYear(Number(d.slice(0, 4)));
    setMonth(Number(d.slice(5, 7)) - 1);
    setView('month');
    setDetail(null);
  }, [config]);

  return (
    <div className="mx-auto max-w-4xl space-y-4">
      <Card padding="none" className="overflow-hidden">
        {/* 工具栏：年份跳转 + 月份切换 + 视图切换 */}
        <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border/60 px-4 py-3">
          <div className="flex items-center gap-1">
            <button
              type="button"
              onClick={() => shiftYear(-1)}
              className="rounded-lg border border-border px-2.5 py-1 text-xs hover:bg-accent"
              title={t('calendar.prevYear')}
            >
              «
            </button>
            <span className="min-w-16 text-center text-sm font-semibold">{year} 年</span>
            <button
              type="button"
              onClick={() => shiftYear(1)}
              className="rounded-lg border border-border px-2.5 py-1 text-xs hover:bg-accent"
              title={t('calendar.nextYear')}
            >
              »
            </button>
            {view === 'month' && (
              <>
                <button
                  type="button"
                  onClick={() => shiftMonth(-1)}
                  className="rounded-lg border border-border p-1.5 hover:bg-accent"
                  title={t('calendar.prevMonth')}
                >
                  <ChevronLeft className="h-4 w-4" />
                </button>
                <span className="min-w-16 text-center text-sm font-semibold">{month + 1} 月</span>
                <button
                  type="button"
                  onClick={() => shiftMonth(1)}
                  className="rounded-lg border border-border p-1.5 hover:bg-accent"
                  title={t('calendar.nextMonth')}
                >
                  <ChevronRight className="h-4 w-4" />
                </button>
              </>
            )}
          </div>

          <div className="flex items-center gap-1.5">
            <button
              type="button"
              onClick={goToToday}
              className="rounded-lg border border-border px-2.5 py-1 text-xs hover:bg-accent"
              title={t('calendar.today')}
            >
              <LocateFixed className="mr-1 inline h-3 w-3" />
              {t('calendar.today')}
            </button>
            <button
              type="button"
              onClick={() => setView(view === 'month' ? 'year' : 'month')}
              className={
                'rounded-lg border px-2.5 py-1 text-xs transition-colors ' +
                (view === 'year'
                  ? 'border-primary/30 bg-primary/10 text-primary'
                  : 'border-border hover:bg-accent')
              }
            >
              <CalendarRange className="mr-1 inline h-3 w-3" />
              {view === 'month' ? t('calendar.yearView') : t('calendar.monthView')}
            </button>
          </div>
        </div>

        <AnimatePresence mode="wait" initial={false}>
          {view === 'year' ? (
            /* 年视图：4×3 十二个月 */
            <motion.div
              key="year"
              initial={{ opacity: 0, scale: 0.98 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.98 }}
              transition={{ duration: 0.18, ease: 'easeOut' }}
            >
              <div className="grid grid-cols-2 gap-3 p-4 sm:grid-cols-3 lg:grid-cols-4">
                {yearMonths.map(({ month: m, cells, total }) => (
                  <button
                    key={m}
                    type="button"
                    onClick={() => goToMonth(m)}
                    className="rounded-xl border border-border/60 p-2 text-left transition-colors hover:border-primary/30 hover:bg-accent/40"
                  >
                    <div className="mb-1.5 flex items-baseline justify-between">
                      <span className="text-xs font-semibold">{m + 1} 月</span>
                      <span className="text-[10px] tabular-nums text-muted-foreground">
                        {formatDuration(total)}
                      </span>
                    </div>
                    <div className="grid grid-cols-7 gap-px">
                      {cells.map((date, i) =>
                        date ? (
                          <span
                            key={date}
                            className={'block aspect-square rounded-[2px] ' + heatColor(usage.get(date) ?? 0, max)}
                          />
                        ) : (
                          <span key={`empty-${i}`} />
                        ),
                      )}
                    </div>
                  </button>
                ))}
              </div>
            </motion.div>
          ) : (
            /* 月视图 */
            <motion.div
              key="month"
              initial={{ opacity: 0, scale: 0.98 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.98 }}
              transition={{ duration: 0.18, ease: 'easeOut' }}
            >
              <div className="grid grid-cols-7 gap-1 px-3 pt-3 text-center text-xs font-medium text-muted-foreground">
                {WEEK_LABELS.map((w) => (
                  <span key={w}>{w}</span>
                ))}
              </div>
              <div className="grid grid-cols-7 gap-1 p-3">
                {days.map((date, i) =>
                  date ? (
                    <button
                      key={date}
                      type="button"
                      onClick={() => void handleSelectDate(date)}
                      className={
                        'aspect-square rounded-lg text-xs font-medium transition-colors ' +
                        heatColor(usage.get(date) ?? 0, max)
                      }
                    >
                      {Number(date.split('-')[2])}
                    </button>
                  ) : (
                    <span key={`empty-${i}`} />
                  ),
                )}
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </Card>

      {detail && view === 'month' && (
        <Card padding="none" className="overflow-hidden">
          <div className="flex items-center justify-between border-b border-border/60 px-4 py-3">
            <span className="text-sm font-semibold">{detail.date}</span>
            <button type="button" onClick={() => setDetail(null)} className="text-muted-foreground hover:text-foreground">
              <X className="h-4 w-4" />
            </button>
          </div>
          <div className="grid grid-cols-2 gap-3 p-4 text-sm sm:grid-cols-3">
            <div>
              <div className="text-xs text-muted-foreground">{t('calendar.active')}</div>
              <div className="font-semibold">{formatDuration(detail.active_seconds)}</div>
            </div>
            <div>
              <div className="text-xs text-muted-foreground">{t('calendar.idle')}</div>
              <div className="font-semibold">{formatDuration(detail.idle_seconds)}</div>
            </div>
            <div>
              <div className="text-xs text-muted-foreground">{t('calendar.sessions')}</div>
              <div className="font-semibold">{detail.session_count}</div>
            </div>
          </div>
          {detail.sessions.length > 0 && (
            <div className="border-t border-border/60 px-4 py-3">
              <div className="mb-2 text-xs font-medium text-muted-foreground">{t('calendar.sessionList')}</div>
              <div className="max-h-52 space-y-1 overflow-y-auto pr-1">
                {detail.sessions.map((s, i) => (
                  <div key={i} className="flex justify-between text-xs">
                    <span className="min-w-0 truncate">{s.app_name}</span>
                    <span className="ml-2 shrink-0 tabular-nums text-muted-foreground">
                      {s.started_at} · {formatDuration(s.duration_secs)}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}
        </Card>
      )}
    </div>
  );
}
