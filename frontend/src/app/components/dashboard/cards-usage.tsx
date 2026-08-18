// 使用统计域：健康 hook、实时活跃、统计/应用/小时/日历/健康内容与独立卡片。
import { useCallback, useEffect, useMemo, useState } from 'react';
import { BellRing, ChartColumn, Clock3, HeartPulse, Hourglass, Timer, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Bar, BarChart, CartesianGrid, Line, LineChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from 'recharts';
import { apiService } from '../../services/api';
import { useAppStore } from '../../store/app-store';
import { useShallow } from 'zustand/react/shallow';
import type { AppPeriodUsageDto, AppUsageDto, DashboardDataDto, DayDetailDto, HealthSnapshotDto } from '../../types';
import AppIcon from './AppIcon';
import ChartTooltip, { formatAxisSeconds } from './ChartTooltip';
import { Card } from '../ui/index';
import type { CardSize } from './dashboard-layout';
import { CardShell, CHART_H, LIST_LIMIT, WEEK_LABELS, clsx, formatDuration, formatDurationCompact, isNarrow, isWide, truncateLabel } from './card-common';
function useHealthData(): HealthSnapshotDto | null {
  const { config } = useAppStore(useShallow((s) => ({ config: s.config })));
  const refreshSeconds = config?.live_refresh_interval_seconds ?? 1;
  const [snap, setSnap] = useState<HealthSnapshotDto | null>(null);

  useEffect(() => {
    let disposed = false;
    const tick = async () => {
      try {
        const next = await apiService.getHealthSnapshot();
        if (!disposed) setSnap(next);
      } catch {
        /* 静默降级 */
      }
    };
    void tick();
    const timer = window.setInterval(() => void tick(), refreshSeconds * 1000);
    return () => {
      disposed = true;
      window.clearInterval(timer);
    };
  }, []);

  return snap;
}

/* ────────────────────────── 内容片段（供聚合卡复用） ────────────────────────── */


/**
 * 使用时间秒级跳动：每 1s 拉取「进行中的活跃会话已持续秒数」（空闲时后端返回 0，
 * 自动停止），叠加到基础统计上。只重渲染所在卡片，不影响其它卡片。
 */
export function useLiveActive(tickable: boolean, base: number | undefined): number | undefined {
  const [elapsed, setElapsed] = useState(0);
  useEffect(() => {
    if (!tickable) {
      setElapsed(0);
      return;
    }
    let disposed = false;
    const tick = () => {
      void apiService
        .getActiveSessionElapsed()
        .then((v) => {
          if (!disposed) setElapsed(v);
        })
        .catch(() => {});
    };
    tick();
    const timer = window.setInterval(tick, 1000);
    return () => {
      disposed = true;
      window.clearInterval(timer);
    };
  }, [tickable]);
  return tickable && base !== undefined ? base + elapsed : undefined;
}

export function StatsContent({
  data,
  size,
  liveActive,
}: {
  data: DashboardDataDto | null;
  size: CardSize;
  /** 秒级跳动的活跃秒数（含进行中会话），为空时回落为 data 的统计值。 */
  liveActive?: number;
}) {
  const { t } = useTranslation();
  const compact = isNarrow(size);
  const activeSeconds = liveActive ?? data?.active_seconds ?? 0;
  const tiles = [
    {
      icon: <Timer className="h-4 w-4" />,
      label: t('dashboard.stats.active'),
      value: data ? (compact ? formatDurationCompact(activeSeconds) : formatDuration(activeSeconds)) : '--',
      accent: true,
    },
    {
      icon: <Clock3 className="h-4 w-4" />,
      label: t('dashboard.stats.idle'),
      value: data ? (compact ? formatDurationCompact(data.idle_seconds) : formatDuration(data.idle_seconds)) : '--',
      accent: false,
    },
    {
      icon: <ChartColumn className="h-4 w-4" />,
      label: t('dashboard.stats.apps'),
      value: data ? String(data.apps.filter((a) => a.active_seconds > 0).length) : '--',
      accent: true,
    },
  ];
  // 小卡：三行竖排（图标 + 名称 + 数字横排），避免三列把时长挤出去。
  if (compact) {
    return (
      <div className="flex h-full flex-col justify-center gap-1.5">
        {tiles.map((tile) => (
          <div
            key={tile.label}
            className="flex items-center gap-2 rounded-lg border border-border/60 px-2 py-1.5"
          >
            <span
              className={clsx(
                'flex h-6 w-6 shrink-0 items-center justify-center rounded-lg',
                tile.accent ? 'border border-primary/15 bg-primary/10 text-primary' : 'border border-border bg-muted text-muted-foreground',
              )}
            >
              {tile.icon}
            </span>
            <span className="min-w-0 flex-1 truncate text-xs text-muted-foreground">{tile.label}</span>
            <span className="shrink-0 text-lg font-semibold tabular-nums leading-tight">{tile.value}</span>
          </div>
        ))}
      </div>
    );
  }
  return (
    <div className="grid grid-cols-3 gap-1.5">
      {tiles.map((tile) => (
        <div
          key={tile.label}
          className="flex min-w-0 flex-col items-center gap-1 rounded-lg border border-border/60 px-1.5 py-2 text-center"
        >
          <span
            className={clsx(
              'flex h-8 w-8 shrink-0 items-center justify-center rounded-lg',
              tile.accent ? 'border border-primary/15 bg-primary/10 text-primary' : 'border border-border bg-muted text-muted-foreground',
            )}
          >
            {tile.icon}
          </span>
          <span className="w-full truncate text-[10px] leading-tight text-muted-foreground">{tile.label}</span>
          <span className="w-full truncate text-sm font-semibold tabular-nums leading-tight">{tile.value}</span>
        </div>
      ))}
    </div>
  );
}


function AppUsageContent({
  data,
  focus,
  size,
}: {
  data: DashboardDataDto | null;
  focus: string;
  size: CardSize;
}) {
  const { t } = useTranslation();
  const [selectedApp, setSelectedApp] = useState<AppUsageDto | null>(null);
  const [appWindows, setAppWindows] = useState<{ title: string; seconds: number }[]>([]);
  const [appHourly, setAppHourly] = useState<number[]>([]);
  const [periodUsage, setPeriodUsage] = useState<AppPeriodUsageDto | null>(null);

  const limit = LIST_LIMIT[size];
  const chartCount = isNarrow(size) ? 6 : 10;

  const topApps = useMemo(() => {
    if (!data) return [];
    return [...data.apps]
      .filter((a) => a.active_seconds > 0)
      .sort((a, b) => b.active_seconds - a.active_seconds)
      .slice(0, limit);
  }, [data, limit]);

  const maxAppSeconds = useMemo(
    () => topApps.reduce((m, a) => Math.max(m, a.active_seconds), 1),
    [topApps],
  );

  const handleSelectApp = useCallback(
    async (app: AppUsageDto) => {
      setSelectedApp(app);
      try {
        const [windows, hourlyData, periods] = await Promise.all([
          apiService.getWindowTitles(app.app_name, focus),
          apiService.getAppHourly(app.app_name, focus),
          apiService.getAppPeriodUsage(app.app_name, focus),
        ]);
        setAppWindows(windows);
        setAppHourly(hourlyData);
        setPeriodUsage(periods);
      } catch {
        setAppWindows([]);
        setAppHourly([]);
        setPeriodUsage(null);
      }
    },
    [focus],
  );

  if (!data || topApps.length === 0) {
    return <p className="py-8 text-center text-sm text-muted-foreground">{t('dashboard.empty')}</p>;
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className={isNarrow(size) ? 'min-h-0 flex-1' : CHART_H[size]}>
        <ResponsiveContainer width="100%" height="100%">
          <BarChart
            data={topApps.slice(0, chartCount).map((a) => ({ name: a.app_name, 时长: a.active_seconds }))}
            margin={{ left: 0, right: 8 }}
          >
            <CartesianGrid strokeDasharray="3 3" stroke="var(--chart-grid)" vertical={false} />
            <XAxis
              dataKey="name"
              tick={{ fontSize: 9 }}
              interval={0}
              angle={0}
              height={30}
              tickFormatter={(v) => truncateLabel(String(v), isNarrow(size) ? 5 : 8)}
            />
            <YAxis tick={{ fontSize: 9, fill: 'var(--chart-tick)' }} tickFormatter={formatAxisSeconds} width={42} />
            <Tooltip
              cursor={{ fill: 'rgba(47,109,246,0.06)' }}
              content={<ChartTooltip valueFormatter={(v) => formatDuration(Number(v))} />}
            />
            <Bar dataKey="时长" fill="var(--chart-primary)" radius={[4, 4, 0, 0]} />
          </BarChart>
        </ResponsiveContainer>
      </div>
      <div className="mt-2 flex min-h-0 flex-1 flex-col overflow-y-auto pr-1">
        {topApps.map((app) => (
          <button
            key={app.exe_path || app.app_name}
            type="button"
            onClick={() => void handleSelectApp(app)}
            className={clsx(
              'flex min-h-0 w-full items-center gap-2 rounded-md border px-2 text-left transition-colors',
              'flex-1 text-sm',
              isNarrow(size) ? 'py-0.5' : 'py-1',
              selectedApp?.app_name === app.app_name
                ? 'border-primary/30 bg-primary/10'
                : 'border-transparent hover:border-border hover:bg-accent/50',
            )}
          >
            <AppIcon exePath={app.exe_path} size={isNarrow(size) ? 18 : 20} />
            <span className="min-w-0 flex-1 truncate">{app.app_name}</span>
            <span className={clsx('shrink-0 tabular-nums text-muted-foreground', isNarrow(size) ? 'text-xs' : 'text-sm')}>
              {formatDuration(app.active_seconds)}
            </span>
            <span className={clsx('h-1 shrink-0 overflow-hidden rounded-full bg-muted', isNarrow(size) ? 'w-8' : 'w-12')}>
              <span
                className="block h-full rounded-full bg-primary"
                style={{ width: `${Math.round((app.active_seconds / maxAppSeconds) * 100)}%` }}
              />
            </span>
          </button>
        ))}
      {selectedApp && (
        <div className="mt-2 rounded-lg border border-border/60 bg-background/60 p-2.5 text-xs">
          <div className="mb-2 flex items-center justify-between gap-2">
            <span className="truncate font-semibold">{selectedApp.app_name}</span>
            <button
              type="button"
              onClick={() => {
                setSelectedApp(null);
                setPeriodUsage(null);
              }}
              className="shrink-0 text-muted-foreground hover:text-foreground"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </div>
          {periodUsage?.app_name === selectedApp.app_name && (
            <div className="mb-2 grid grid-cols-3 gap-1.5">
              {[
                ['usage.period.today', periodUsage.today_seconds],
                ['usage.period.week', periodUsage.week_seconds],
                ['usage.period.month', periodUsage.month_seconds],
              ].map(([label, seconds]) => (
                <div key={String(label)} className="min-w-0 rounded border border-border/60 px-1.5 py-1">
                  <div className="truncate text-[10px] text-muted-foreground">{t(String(label))}</div>
                  <div className="truncate text-[11px] font-semibold tabular-nums">{formatDuration(Number(seconds))}</div>
                </div>
              ))}
            </div>
          )}
          {!isNarrow(size) && appHourly.length > 0 && (
            <div className="h-20">
              <ResponsiveContainer width="100%" height="100%">
                <LineChart data={appHourly.map((v, i) => ({ h: i, v }))}>
                  <CartesianGrid strokeDasharray="3 3" stroke="var(--chart-grid)" vertical={false} />
                  <XAxis dataKey="h" tick={{ fontSize: 9 }} interval={3} />
                  <YAxis tick={{ fontSize: 9, fill: 'var(--chart-tick)' }} tickFormatter={formatAxisSeconds} width={40} />
                  <Tooltip
                    cursor={{ stroke: 'var(--chart-axis)', strokeDasharray: '3 3' }}
                    content={<ChartTooltip valueFormatter={(v) => formatDuration(Number(v))} />}
                  />
                  <Line
                    type="monotone"
                    name={t('dashboard.hourly.activeLabel')}
                    dataKey="v"
                    stroke="var(--chart-primary)"
                    dot={false}
                    strokeWidth={1.5}
                  />
                </LineChart>
              </ResponsiveContainer>
            </div>
          )}
          {appWindows.length > 0 && (
            <div className="mt-2 space-y-1">
              {appWindows.map((w) => (
                <div key={w.title} className="flex justify-between text-muted-foreground">
                  <span className="min-w-0 truncate">{w.title}</span>
                  <span className="ml-2 shrink-0 tabular-nums">{formatDuration(w.seconds)}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
    </div>
  );
}


export function HourlyContent({ hourly, size }: { hourly: number[]; size: CardSize }) {
  const { t } = useTranslation();
  return (
    // 填满卡片剩余高度（不再按宽度锁 16:9/21:9——九宫格行高有限，固定比例会超高裁掉横轴）。
    <div className="min-h-0 flex-1">
      <ResponsiveContainer width="100%" height="100%">
        <LineChart data={hourly.map((v, i) => ({ h: i, v }))} margin={{ left: 0, right: 8 }}>
          <CartesianGrid strokeDasharray="3 3" stroke="var(--chart-grid)" vertical={false} />
          <XAxis dataKey="h" tick={{ fontSize: 10 }} interval={3} />
          <YAxis tick={{ fontSize: 10, fill: 'var(--chart-tick)' }} tickFormatter={formatAxisSeconds} width={44} />
          <Tooltip
            cursor={{ stroke: 'var(--chart-axis)', strokeDasharray: '3 3' }}
            content={
              <ChartTooltip
                labelFormatter={(h) => `${h} 时`}
                valueFormatter={(v) => formatDuration(Number(v))}
              />
            }
          />
          <Line
            type="monotone"
            name={t('dashboard.hourly.activeLabel')}
            dataKey="v"
            stroke="var(--chart-primary)"
            dot={false}
            strokeWidth={2}
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}


function heatColor(seconds: number, max: number): string {
  if (seconds <= 0 || max <= 0) return 'bg-card hover:bg-accent';
  const ratio = seconds / max;
  if (ratio < 0.25) return 'bg-primary/15 hover:bg-primary/25';
  if (ratio < 0.5) return 'bg-primary/30 hover:bg-primary/40';
  if (ratio < 0.75) return 'bg-primary/55 hover:bg-primary/65';
  return 'bg-primary/85 hover:bg-primary';
}


function CalendarContent({
  usage,
  max,
  focus,
  size,
}: {
  usage: Map<string, number>;
  max: number;
  focus: string;
  size: CardSize;
}) {
  const { t } = useTranslation();
  const [dayDetail, setDayDetail] = useState<DayDetailDto | null>(null);

  const handleSelectDate = useCallback(async (date: string) => {
    try {
      const detail = await apiService.getDayDetail(date);
      setDayDetail(detail);
    } catch {
      setDayDetail(null);
    }
  }, []);

  const days = useMemo(() => {
    const [y, m] = focus.split('-').map(Number);
    const first = new Date(Date.UTC(y, m - 1, 1));
    const daysInMonth = new Date(Date.UTC(y, m, 0)).getUTCDate();
    const lead = (first.getUTCDay() + 6) % 7; // 周一开始
    const prefix = `${y}-${String(m).padStart(2, '0')}-`;
    const cells: (string | null)[] = Array.from({ length: lead }, () => null);
    for (let d = 1; d <= daysInMonth; d += 1) {
      cells.push(`${prefix}${String(d).padStart(2, '0')}`);
    }
    return cells;
  }, [focus]);

  return (
    <div className={clsx('flex h-full flex-col', isWide(size) && 'mx-auto max-w-[420px]')}>
      <div className={clsx('mb-1.5 text-center text-muted-foreground', isNarrow(size) ? 'text-xs' : 'text-sm')}>
        {Number(focus.split('-')[0])} 年 {Number(focus.split('-')[1])} 月
      </div>
      <div className={clsx('grid grid-cols-7 text-center text-muted-foreground', isNarrow(size) ? 'gap-0.5 text-xs' : 'gap-1 text-sm')}>
        {WEEK_LABELS.map((w) => (
          <span key={w}>{w}</span>
        ))}
      </div>
      <div className={clsx('mt-0.5 grid grid-cols-7', isNarrow(size) ? 'min-h-0 flex-1 grid-rows-6 gap-0.5' : 'gap-1')}>
        {days.map((date, i) =>
          date ? (
            <button
              key={date}
              type="button"
              onClick={() => void handleSelectDate(date)}
              className={clsx(
                'flex items-center justify-center rounded-md font-medium transition-colors',
                isNarrow(size) ? 'h-full text-xs' : 'h-8 text-sm',
                heatColor(usage.get(date) ?? 0, max),
              )}
            >
              {Number(date.split('-')[2])}
            </button>
          ) : (
            <span key={`empty-${i}`} />
          ),
        )}
      </div>
      {dayDetail && (
        <div className="mt-3 rounded-lg border border-border/60 bg-background/60 p-2.5 text-xs">
          <div className="mb-1 flex items-center justify-between font-medium">
            <span>{dayDetail.date}</span>
            <button
              type="button"
              onClick={() => setDayDetail(null)}
              className="text-muted-foreground hover:text-foreground"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </div>
          <div className="space-y-1 text-muted-foreground">
            <p>{t('dashboard.calendar.active')}: {formatDuration(dayDetail.active_seconds)}</p>
            <p>{t('dashboard.calendar.sessions')}: {dayDetail.session_count}</p>
          </div>
        </div>
      )}
    </div>
  );
}


function HealthContent() {
  const { t } = useTranslation();
  const snap = useHealthData();
  const rows = [
    {
      icon: <Hourglass className="h-4 w-4" />,
      label: t('health.streak'),
      value: snap ? formatDuration(snap.streak_seconds) : '--',
    },
    {
      icon: <BellRing className="h-4 w-4" />,
      label: t('health.nextReminder'),
      value: snap ? formatDuration(snap.next_reminder_seconds) : '--',
    },
    {
      icon: <HeartPulse className="h-4 w-4" />,
      label: t('health.remindersToday'),
      value: snap ? String(snap.reminders_today) : '--',
    },
  ];
  return (
    <div className="flex h-full flex-col justify-center gap-1.5">
      {rows.map((row) => (
        <div key={row.label} className="flex items-center gap-2 rounded-lg border border-border/60 px-2 py-1.5">
          <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md border border-primary/15 bg-primary/10 text-primary">
            {row.icon}
          </span>
          <div className="min-w-0">
            <div className="truncate text-[11px] leading-tight text-muted-foreground">{row.label}</div>
            <div className="truncate text-sm font-semibold leading-tight">{row.value}</div>
          </div>
        </div>
      ))}
    </div>
  );
}


export function StatsCard({
  data,
  size,
  tickable,
}: {
  data: DashboardDataDto | null;
  size: CardSize;
  /** 统计范围包含今天时启用秒级跳动（历史范围不跳）。 */
  tickable?: boolean;
}) {
  const { t } = useTranslation();
  const liveActive = useLiveActive(tickable ?? false, data?.active_seconds);
  return (
    <CardShell title={t('dashboard.cards.stats')}>
      <StatsContent data={data} size={size} liveActive={liveActive} />
    </CardShell>
  );
}


export function AppUsageCard({ data, focus, size }: { data: DashboardDataDto | null; focus: string; size: CardSize }) {
  const { t } = useTranslation();
  return (
    <CardShell title={t('dashboard.appUsage.title')} bodyClassName="!p-2.5">
      <AppUsageContent data={data} focus={focus} size={size} />
    </CardShell>
  );
}


export function HourlyCard({ hourly, size }: { hourly: number[]; size: CardSize }) {
  const { t } = useTranslation();
  return (
    <CardShell title={t('dashboard.hourly.title')}>
      <HourlyContent hourly={hourly} size={size} />
    </CardShell>
  );
}


export function CalendarCard({
  usage,
  max,
  focus,
  size,
}: {
  usage: Map<string, number>;
  max: number;
  focus: string;
  size: CardSize;
}) {
  const { t } = useTranslation();
  return (
    <CardShell title={t('dashboard.cards.calendar')}>
      <CalendarContent usage={usage} max={max} focus={focus} size={size} />
    </CardShell>
  );
}


export function HealthCard() {
  const { t } = useTranslation();
  return (
    <CardShell title={t('dashboard.cards.health')}>
      <HealthContent />
    </CardShell>
  );
}




