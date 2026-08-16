'use client';

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { AnimatePresence, motion } from 'framer-motion';
import { CalendarRange, ChevronLeft, ChevronRight, LocateFixed } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { apiService } from '../services/api';
import { useAppStore } from '../store/app-store';
import { todayStr } from '../lib/datetime';
import { Card } from './ui/index';
import { DayPanel } from './calendar/day-panel';
import { formatDuration, heatColor } from './calendar/calendar-common';

const WEEK_LABELS = ['1', '2', '3', '4', '5', '6', '日'];

type View = 'month' | 'year' | 'day';
export default function CalendarPage() {
  const { t } = useTranslation();
  const { config } = useAppStore(useShallow((s) => ({ config: s.config })));
  const today = todayStr(config);
  const [year, setYear] = useState(() => Number(today.slice(0, 4)));
  const [month, setMonth] = useState(() => Number(today.slice(5, 7)) - 1); // 0-11
  const [view, setView] = useState<View>('month');
  const [selectedDate, setSelectedDate] = useState<string | null>(null);
  const [usage, setUsage] = useState<Map<string, number>>(new Map());
  const [max, setMax] = useState(0);

  const gridRef = useRef<HTMLDivElement | null>(null);
  const pageRef = useRef<HTMLDivElement | null>(null);

  // 页面高度 = 视口剩余高度（渲染后坐标实测，对 UI 缩放/侧边栏切换均自适应），
  // 让月/年视图用 CSS 1fr 铺满整个内容区、无需滚动。
  useEffect(() => {
    const el = pageRef.current;
    if (!el) return;
    const update = () => {
      const rect = el.getBoundingClientRect();
      const h = Math.max(window.innerHeight - rect.top - 16, 300);
      if (Math.abs(el.clientHeight - h) > 2) el.style.height = `${h}px`;
    };
    update();
    const ro = new ResizeObserver(update);
    ro.observe(el);
    window.addEventListener('resize', update);
    return () => {
      ro.disconnect();
      window.removeEventListener('resize', update);
    };
  }, []);

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

  const yearCols = useMemo(() => {
    if (typeof window === 'undefined') return 4;
    return window.innerWidth >= 1100 ? 4 : window.innerWidth >= 760 ? 3 : 2;
  }, []);
  const yearRows = Math.ceil(12 / yearCols);

  const openDay = useCallback((date: string) => {
    setSelectedDate(date);
    setView('day');
  }, []);

  const goToMonth = useCallback((m: number) => {
    setMonth(m);
    setView('month');
  }, []);

  const goToToday = useCallback(() => {
    const d = todayStr(config);
    setYear(Number(d.slice(0, 4)));
    setMonth(Number(d.slice(5, 7)) - 1);
    setView('month');
    setSelectedDate(null);
  }, [config]);

  const shiftMonth = useCallback(
    (delta: number) => {
      const next = new Date(Date.UTC(year, month + delta, 1));
      setYear(next.getUTCFullYear());
      setMonth(next.getUTCMonth());
    },
    [year, month],
  );

  const shiftYear = useCallback((delta: number) => setYear((y) => y + delta), []);

  // 日视图：整页仪表盘（允许滚动）。
  if (view === 'day' && selectedDate) {
    return (
      <div className="mx-auto max-w-4xl space-y-4">
        <DayPanel date={selectedDate} onBack={() => setView('month')} />
      </div>
    );
  }

  return (
    <div ref={pageRef} className="flex flex-col space-y-4">
      <Card padding="none" className="flex min-h-0 flex-1 flex-col overflow-hidden">
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
            /* 年视图：初始简洁样式，12 个月按 1fr 铺满页面 + 间距 */
            <motion.div
              key="year"
              className="flex min-h-0 flex-1 flex-col p-3"
              initial={{ opacity: 0, scale: 0.98 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.98 }}
              transition={{ duration: 0.18, ease: 'easeOut' }}
            >
              <div
                className="grid h-full w-full"
                style={{
                  gridTemplateColumns: `repeat(${yearCols}, 1fr)`,
                  gridAutoRows: '1fr',
                  gap: 12,
                }}
              >
                {yearMonths.map(({ month: m, cells, total }) => (
                  <div
                    key={m}
                    className="flex min-h-0 flex-col rounded-xl border border-border/60 p-1.5"
                  >
                    <button
                      type="button"
                      onClick={() => goToMonth(m)}
                      title={t('calendar.monthView')}
                      className="mb-1.5 flex shrink-0 items-baseline justify-between gap-1 px-0.5 text-left transition-colors hover:text-primary"
                    >
                      <span className="text-xs font-semibold">{m + 1} 月</span>
                      <span className="truncate text-[10px] tabular-nums text-muted-foreground">
                        {formatDuration(total)}
                      </span>
                    </button>
                    <div className="grid min-h-0 flex-1 grid-cols-7 grid-rows-6 gap-px">
                      {cells.map((date, i) => {
                        if (!date) return <span key={`empty-${i}`} />;
                        const secs = usage.get(date) ?? 0;
                        const ratio = max > 0 ? secs / max : 0;
                        return (
                          <button
                            key={date}
                            type="button"
                            onClick={() => openDay(date)}
                            title={date}
                            className={
                              'flex min-h-0 min-w-0 items-center justify-center rounded-[2px] text-[9px] font-medium leading-none transition-colors ' +
                              heatColor(secs, max) +
                              (ratio >= 0.5 ? ' text-white' : ' text-foreground/65')
                            }
                          >
                            {Number(date.split('-')[2])}
                          </button>
                        );
                      })}
                    </div>
                  </div>
                ))}
              </div>
            </motion.div>
          ) : (
            /* 月视图：日期格按 1fr 铺满页面（不锁 1:1），排出间距，无滚动 */
            <motion.div
              key="month"
              className="flex min-h-0 flex-1 flex-col p-3"
              initial={{ opacity: 0, scale: 0.98 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.98 }}
              transition={{ duration: 0.18, ease: 'easeOut' }}
            >
              <div className="grid shrink-0 grid-cols-7 gap-2 pb-2 text-center text-xs font-medium text-muted-foreground">
                {WEEK_LABELS.map((w) => (
                  <span key={w}>{w}</span>
                ))}
              </div>
              <div ref={gridRef} className="min-h-0 flex-1">
                <div className="grid h-full w-full grid-cols-7 grid-rows-6 gap-2">
                  {days.map((date, i) =>
                    date ? (
                      <button
                        key={date}
                        type="button"
                        onClick={() => openDay(date)}
                        className={
                          'flex min-h-0 min-w-0 flex-col items-center justify-center rounded-lg border border-border/50 shadow-sm transition-colors hover:border-primary/40 ' +
                          heatColor(usage.get(date) ?? 0, max)
                        }
                      >
                        <span className="text-[13px] font-medium leading-none">
                          {Number(date.split('-')[2])}
                        </span>
                      </button>
                    ) : (
                      <span key={`empty-${i}`} />
                    ),
                  )}
                </div>
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </Card>
    </div>
  );
}


