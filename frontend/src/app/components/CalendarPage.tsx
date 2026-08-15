'use client';

import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { AnimatePresence, motion } from 'framer-motion';
import { ArrowLeft, CalendarRange, ChevronLeft, ChevronRight, LocateFixed } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts';
import { apiService } from '../services/api';
import { useAppStore } from '../store/app-store';
import type { DayDetailDto, DayMetricsDto } from '../types';
import { todayStr } from '../lib/datetime';
import { Card } from './ui/index';
import ChartTooltip from './dashboard/ChartTooltip';

const WEEK_LABELS = ['1', '2', '3', '4', '5', '6', '日'];
/** Top 应用堆叠柱的颜色（与 Pie 配色一致）。 */
const APP_COLORS = ['#2f6df6', '#7c5cf0', '#0ea5e9', '#10b981', '#f59e0b', '#ec4899'];

function formatDuration(seconds: number): string {
  if (!seconds || seconds <= 0) return '0 秒';
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  if (h > 0) return `${h} 小时 ${m} 分 ${s} 秒`;
  if (m > 0) return `${m} 分 ${s} 秒`;
  return `${s} 秒`;
}

function formatBytes(bytes: number, perSecond = false): string {
  if (!bytes || bytes <= 0) return perSecond ? '0.0 B/s' : '0.0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(1)} ${units[unit]}${perSecond ? '/s' : ''}`;
}

function heatColor(seconds: number, max: number): string {
  if (seconds <= 0 || max <= 0) return 'bg-card hover:bg-accent';
  const ratio = seconds / max;
  if (ratio < 0.25) return 'bg-primary/15 hover:bg-primary/25';
  if (ratio < 0.5) return 'bg-primary/30 hover:bg-primary/40';
  if (ratio < 0.75) return 'bg-primary/55 hover:bg-primary/65';
  return 'bg-primary/85 hover:bg-primary';
}

/** 分钟序号 → "HH:MM"。 */
function fmtMin(minute: number): string {
  const hh = Math.floor(minute / 60);
  const mm = minute % 60;
  return `${String(hh).padStart(2, '0')}:${String(mm).padStart(2, '0')}`;
}

function SectionTitle({ children }: { children: ReactNode }) {
  return <div className="border-b border-border/60 px-4 py-3 text-sm font-semibold">{children}</div>;
}

/** 某日历日的仪表盘：24h 活跃 / 应用 / 硬件 / 网络 / 会话。 */
function DayPanel({ date, onBack }: { date: string; onBack: () => void }) {
  const { t } = useTranslation();
  const [detail, setDetail] = useState<DayDetailDto | null>(null);
  const [hourly, setHourly] = useState<number[]>([]);
  const [metrics, setMetrics] = useState<DayMetricsDto | null>(null);
  const [topApps, setTopApps] = useState<{ app: string; hours: number[] }[]>([]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [d, h, m, hourApps] = await Promise.all([
          apiService.getDayDetail(date),
          apiService.getDayHourly(date),
          apiService.getDayMetrics(date),
          Promise.all(Array.from({ length: 24 }, (_, hh) => apiService.getHourApps(date, hh))),
        ]);
        if (cancelled) return;
        setDetail(d);
        setHourly(h);
        setMetrics(m);
        // 聚合 24 小时每小时的应用使用 → Top 6
        const byApp = new Map<string, number[]>();
        hourApps.forEach((apps, hh) => {
          for (const a of apps) {
            const arr = byApp.get(a.app_name) ?? new Array<number>(24).fill(0);
            arr[hh] = (arr[hh] ?? 0) + a.active_seconds;
            byApp.set(a.app_name, arr);
          }
        });
        const top = [...byApp.entries()]
          .map(([app, hours]) => ({ app, hours, total: hours.reduce((a, b) => a + b, 0) }))
          .sort((a, b) => b.total - a.total)
          .slice(0, 6)
          .map(({ app, hours }) => ({ app, hours }));
        setTopApps(top);
      } catch {
        /* 静默 */
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [date]);

  const hourlyData = hourly.map((v, h) => ({ h, v }));
  const stackedData = Array.from({ length: 24 }, (_, h) => {
    const row: Record<string, number | string> = { h };
    for (const { app, hours } of topApps) row[app] = hours[h] ?? 0;
    return row;
  });

  // 分钟级序列降采样到 ≤96 点
  const cpuData = useMemo(() => {
    const memByMin = new Map((metrics?.mem_percent ?? []).map((p) => [p.minute, p.avg]));
    const raw = (metrics?.cpu_percent ?? []).map((p) => ({
      t: fmtMin(p.minute),
      cpu: p.avg,
      mem: memByMin.get(p.minute) ?? 0,
    }));
    const step = Math.max(1, Math.floor(raw.length / 96));
    return raw.filter((_, i) => i % step === 0);
  }, [metrics]);

  const tempData = useMemo(() => {
    const gpuByMin = new Map((metrics?.gpu_temp_c ?? []).map((p) => [p.minute, p.avg]));
    const raw = (metrics?.cpu_temp_c ?? []).map((p) => ({
      t: fmtMin(p.minute),
      cpu: p.avg,
      gpu: gpuByMin.get(p.minute) ?? null,
    }));
    const step = Math.max(1, Math.floor(raw.length / 96));
    return raw.filter((_, i) => i % step === 0);
  }, [metrics]);

  const netData = useMemo(() => {
    const byMin = new Map<number, { down: number; up: number }>();
    for (const p of metrics?.net_down_bps ?? []) {
      const cur = byMin.get(p.minute) ?? { down: 0, up: 0 };
      cur.down = p.avg;
      byMin.set(p.minute, cur);
    }
    for (const p of metrics?.net_up_bps ?? []) {
      const cur = byMin.get(p.minute) ?? { down: 0, up: 0 };
      cur.up = p.avg;
      byMin.set(p.minute, cur);
    }
    const minutes = [...byMin.keys()].sort((a, b) => a - b);
    const step = Math.max(1, Math.floor(minutes.length / 96));
    return minutes
      .filter((_, i) => i % step === 0)
      .map((minute) => {
        const v = byMin.get(minute)!;
        return { t: fmtMin(minute), down: v.down, up: v.up };
      });
  }, [metrics]);

  const hasHw = cpuData.length > 0 || tempData.length > 0;
  const hasNet = netData.length > 0;
  const hasApps = topApps.length > 0;

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <button
          type="button"
          onClick={onBack}
          className="flex items-center gap-1 rounded-lg border border-border px-2.5 py-1 text-xs hover:bg-accent"
        >
          <ArrowLeft className="h-3.5 w-3.5" />
          {t('calendar.back')}
        </button>
        <span className="text-sm font-semibold">{date} · {t('calendar.dayView')}</span>
      </div>

      {/* 当日统计 */}
      <div className="grid grid-cols-3 gap-3">
        <Card className="p-3">
          <div className="text-xs text-muted-foreground">{t('calendar.active')}</div>
          <div className="mt-0.5 text-lg font-semibold tabular-nums">
            {detail ? formatDuration(detail.active_seconds) : '--'}
          </div>
        </Card>
        <Card className="p-3">
          <div className="text-xs text-muted-foreground">{t('calendar.idle')}</div>
          <div className="mt-0.5 text-lg font-semibold tabular-nums">
            {detail ? formatDuration(detail.idle_seconds) : '--'}
          </div>
        </Card>
        <Card className="p-3">
          <div className="text-xs text-muted-foreground">{t('calendar.sessions')}</div>
          <div className="mt-0.5 text-lg font-semibold tabular-nums">{detail?.session_count ?? '--'}</div>
        </Card>
      </div>

      {/* 24h 活跃 */}
      <Card padding="none" className="overflow-hidden">
        <SectionTitle>{t('calendar.hourlyTitle')}</SectionTitle>
        <div className="h-44 p-4">
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={hourlyData} margin={{ left: -14, right: 8 }}>
              <CartesianGrid strokeDasharray="3 3" stroke="var(--chart-grid)" vertical={false} />
              <XAxis dataKey="h" tick={{ fontSize: 10, fill: 'var(--chart-tick)' }} interval={2} />
              <YAxis
                tick={{ fontSize: 10, fill: 'var(--chart-tick)' }}
                tickFormatter={(v) => formatDuration(Number(v)).split(' ')[0]}
                width={48}
              />
              <Tooltip
                cursor={{ fill: 'rgba(47,109,246,0.06)' }}
                content={
                  <ChartTooltip
                    labelFormatter={(h) => `${h} 时`}
                    valueFormatter={(v) => formatDuration(Number(v))}
                  />
                }
              />
              <Bar dataKey="v" fill="var(--chart-primary)" radius={[3, 3, 0, 0]} isAnimationActive={false} />
            </BarChart>
          </ResponsiveContainer>
        </div>
      </Card>

      {/* 应用使用 Top 6（24h 堆叠） */}
      <Card padding="none" className="overflow-hidden">
        <SectionTitle>{t('calendar.appsTitle')}</SectionTitle>
        <div className="h-56 p-4">
          {hasApps ? (
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={stackedData} margin={{ left: -14, right: 8 }}>
                <CartesianGrid strokeDasharray="3 3" stroke="var(--chart-grid)" vertical={false} />
                <XAxis dataKey="h" tick={{ fontSize: 10, fill: 'var(--chart-tick)' }} interval={2} />
                <YAxis
                  tick={{ fontSize: 10, fill: 'var(--chart-tick)' }}
                  tickFormatter={(v) => formatDuration(Number(v)).split(' ')[0]}
                  width={48}
                />
                <Tooltip
                  cursor={{ fill: 'rgba(47,109,246,0.06)' }}
                  content={
                    <ChartTooltip
                      labelFormatter={(h) => `${h} 时`}
                      valueFormatter={(v) => formatDuration(Number(v))}
                    />
                  }
                />
                {topApps.map(({ app }, i) => (
                  <Bar
                    key={app}
                    dataKey={app}
                    stackId="apps"
                    fill={APP_COLORS[i % APP_COLORS.length]}
                    radius={i === 0 ? [3, 3, 0, 0] : [0, 0, 0, 0]}
                    isAnimationActive={false}
                  />
                ))}
              </BarChart>
            </ResponsiveContainer>
          ) : (
            <p className="flex h-full items-center justify-center text-sm text-muted-foreground">{t('calendar.empty')}</p>
          )}
        </div>
        {topApps.length > 0 && (
          <div className="flex flex-wrap gap-x-3 gap-y-1 border-t border-border/60 px-4 py-2">
            {topApps.map(({ app }, i) => (
              <span key={app} className="flex items-center gap-1 text-[11px] text-muted-foreground">
                <span className="h-2 w-2 rounded-sm" style={{ backgroundColor: APP_COLORS[i % APP_COLORS.length] }} />
                <span className="max-w-40 truncate">{app}</span>
              </span>
            ))}
          </div>
        )}
      </Card>

      {/* 硬件曲线（CPU / 内存 / 温度） */}
      <Card padding="none" className="overflow-hidden">
        <SectionTitle>{t('calendar.hardwareTitle')}</SectionTitle>
        {hasHw ? (
          <>
            <div className="h-44 p-4">
              <ResponsiveContainer width="100%" height="100%">
                <AreaChart data={cpuData} margin={{ left: -14, right: 8 }}>
                  <CartesianGrid strokeDasharray="3 3" stroke="var(--chart-grid)" vertical={false} />
                  <XAxis dataKey="t" tick={{ fontSize: 9, fill: 'var(--chart-tick)' }} interval="preserveStartEnd" />
                  <YAxis
                    tick={{ fontSize: 10, fill: 'var(--chart-tick)' }}
                    width={42}
                    domain={[0, 100]}
                    tickFormatter={(v) => `${v}%`}
                  />
                  <Tooltip
                    cursor={{ stroke: 'var(--chart-axis)', strokeDasharray: '3 3' }}
                    content={<ChartTooltip valueFormatter={(v) => `${Number(v).toFixed(1)}%`} />}
                  />
                  <Area type="monotone" dataKey="cpu" name={t('calendar.cpu')} stroke="#2f6df6" fill="#2f6df6" fillOpacity={0.18} strokeWidth={2} dot={false} isAnimationActive={false} />
                  <Area type="monotone" dataKey="mem" name={t('calendar.mem')} stroke="#10b981" fill="#10b981" fillOpacity={0.15} strokeWidth={2} dot={false} isAnimationActive={false} />
                </AreaChart>
              </ResponsiveContainer>
            </div>
            {tempData.length > 0 && (
              <div className="h-40 border-t border-border/60 p-4">
                <ResponsiveContainer width="100%" height="100%">
                  <LineChart data={tempData} margin={{ left: -14, right: 8 }}>
                    <CartesianGrid strokeDasharray="3 3" stroke="var(--chart-grid)" vertical={false} />
                    <XAxis dataKey="t" tick={{ fontSize: 9, fill: 'var(--chart-tick)' }} interval="preserveStartEnd" />
                    <YAxis
                      tick={{ fontSize: 10, fill: 'var(--chart-tick)' }}
                      width={42}
                      tickFormatter={(v) => `${Number(v).toFixed(0)}°`}
                    />
                    <Tooltip
                      cursor={{ stroke: 'var(--chart-axis)', strokeDasharray: '3 3' }}
                      content={<ChartTooltip valueFormatter={(v) => `${Number(v).toFixed(1)}°C`} />}
                    />
                    <Line type="monotone" dataKey="cpu" name={t('calendar.cpuTemp')} stroke="#ef4444" strokeWidth={2} dot={false} isAnimationActive={false} />
                    {tempData.some((p) => p.gpu != null) && (
                      <Line type="monotone" dataKey="gpu" name={t('calendar.gpuTemp')} stroke="#ec4899" strokeWidth={2} dot={false} isAnimationActive={false} />
                    )}
                  </LineChart>
                </ResponsiveContainer>
              </div>
            )}
          </>
        ) : (
          <p className="py-8 text-center text-sm text-muted-foreground">{t('calendar.noMetrics')}</p>
        )}
      </Card>

      {/* 网络曲线（下载 / 上传） */}
      <Card padding="none" className="overflow-hidden">
        <SectionTitle>{t('calendar.networkTitle')}</SectionTitle>
        {hasNet ? (
          <div className="h-44 p-4">
            <ResponsiveContainer width="100%" height="100%">
              <AreaChart data={netData} margin={{ left: -14, right: 8 }}>
                <CartesianGrid strokeDasharray="3 3" stroke="var(--chart-grid)" vertical={false} />
                <XAxis dataKey="t" tick={{ fontSize: 9, fill: 'var(--chart-tick)' }} interval="preserveStartEnd" />
                <YAxis
                  tick={{ fontSize: 10, fill: 'var(--chart-tick)' }}
                  width={52}
                  tickFormatter={(v) => formatBytes(Number(v), true)}
                />
                <Tooltip
                  cursor={{ stroke: 'var(--chart-axis)', strokeDasharray: '3 3' }}
                  content={<ChartTooltip valueFormatter={(v) => formatBytes(Number(v), true)} />}
                />
                <Area type="monotone" dataKey="down" name={t('network.download')} stroke="#1E88E5" fill="#1E88E5" fillOpacity={0.18} strokeWidth={2} dot={false} isAnimationActive={false} />
                <Area type="monotone" dataKey="up" name={t('network.upload')} stroke="#FB8C00" fill="#FB8C00" fillOpacity={0.15} strokeWidth={2} dot={false} isAnimationActive={false} />
              </AreaChart>
            </ResponsiveContainer>
          </div>
        ) : (
          <p className="py-8 text-center text-sm text-muted-foreground">{t('calendar.noMetrics')}</p>
        )}
      </Card>

      {/* 会话记录 */}
      <Card padding="none" className="overflow-hidden">
        <SectionTitle>{t('calendar.sessionList')}</SectionTitle>
        {detail && detail.sessions.length > 0 ? (
          <div className="max-h-72 space-y-1 overflow-y-auto p-3">
            {detail.sessions.map((s, i) => (
              <div key={i} className="flex items-center justify-between rounded px-2 py-1 text-xs hover:bg-accent/40">
                <span className="min-w-0 truncate">{s.app_name}</span>
                <span className="ml-2 shrink-0 tabular-nums text-muted-foreground">
                  {s.started_at} · {formatDuration(s.duration_secs)}
                </span>
              </div>
            ))}
          </div>
        ) : (
          <p className="py-6 text-center text-sm text-muted-foreground">{t('calendar.empty')}</p>
        )}
      </Card>
    </div>
  );
}

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
                  <button
                    key={m}
                    type="button"
                    onClick={() => goToMonth(m)}
                    className="flex min-h-0 flex-col rounded-xl border border-border/60 p-1.5 text-left transition-colors hover:border-primary/30 hover:bg-accent/40"
                  >
                    <div className="mb-1.5 flex shrink-0 items-baseline justify-between gap-1 px-0.5">
                      <span className="text-xs font-semibold">{m + 1} 月</span>
                      <span className="truncate text-[10px] tabular-nums text-muted-foreground">
                        {formatDuration(total)}
                      </span>
                    </div>
                    <div className="grid min-h-0 flex-1 grid-cols-7 grid-rows-6 gap-px">
                      {cells.map((date, i) =>
                        date ? (
                          <span
                            key={date}
                            className={'block min-h-0 min-w-0 rounded-[2px] ' + heatColor(usage.get(date) ?? 0, max)}
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
