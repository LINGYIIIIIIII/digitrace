'use client';

import type { ReactNode } from 'react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  ArrowDown,
  ArrowUp,
  BellRing,
  ChartColumn,
  Clock3,
  Cpu,
  Gauge,
  HardDriveDownload,
  HardDriveUpload,
  HeartPulse,
  Hourglass,
  MemoryStick,
  RotateCcw,
  Thermometer,
  Timer,
  X,
  ZoomIn,
  ZoomOut,
} from 'lucide-react';
import {
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
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { apiService } from '../../services/api';
import { useAppStore } from '../../store/app-store';
import type {
  AppUsageDto,
  DashboardDataDto,
  DayDetailDto,
  HardwareSnapshotDto,
  HealthSnapshotDto,
  NetworkSnapshotDto,
  TemperatureSnapshotDto,
} from '../../types';
import { Card } from '../ui/index';
import AppIcon from './AppIcon';
import ChartTooltip, { formatAxisSeconds } from './ChartTooltip';
import { DualArcGauge, SemiGauge, formatBytes, levelColor, tempColor } from './gauges';
import type { CardSize } from './dashboard-layout';

function clsx(...parts: (string | false | undefined | null)[]): string {
  return parts.filter(Boolean).join(' ');
}

function formatDuration(seconds: number): string {
  if (!seconds || seconds <= 0) return '0 秒';
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  if (h > 0) return `${h} 小时 ${m} 分 ${s} 秒`;
  if (m > 0) return `${m} 分 ${s} 秒`;
  return `${s} 秒`;
}

/** 紧凑时长（小卡片用）：1:02:03 / 4:05 / 0:00。 */
function formatDurationCompact(seconds: number): string {
  if (!seconds || seconds <= 0) return '0:00';
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  if (h > 0) return `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
  return `${m}:${String(s).padStart(2, '0')}`;
}

function formatBytesPerSecond(bytes: number): string {
  if (!bytes || bytes <= 0) return '0.0 B/s';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(1)} ${units[unit]}/s`;
}

/** 速率坐标轴取整：向上取到 1/2/5×10ⁿ 档位（避免 3.7MB/s 这种碎刻度）。 */
function niceCeil(v: number): number {
  if (v <= 0) return 0;
  const exp = Math.floor(Math.log10(v));
  const base = 10 ** exp;
  const m = v / base;
  const nice = m <= 1 ? 1 : m <= 2 ? 2 : m <= 5 ? 5 : 10;
  return nice * base;
}

/** 截断长应用名（图表 X 轴标签用；悬停 Tooltip 与下方列表仍显示全名）。 */
function truncateLabel(name: string, max: number): string {
  return name.length > max ? `${name.slice(0, max)}…` : name;
}

const CHART_H: Record<CardSize, string> = {
  '1x1': 'h-20',
  '1x2': 'h-28',
  '2x1': 'h-24',
  '2x2': 'h-32',
  '3x1': 'h-36',
  '3x2': 'h-44',
  '3x3': 'h-56',
};
const LIST_LIMIT: Record<CardSize, number> = {
  '1x1': 4,
  '1x2': 6,
  '2x1': 6,
  '2x2': 8,
  '3x1': 10,
  '3x2': 12,
  '3x3': 16,
};

/** 窄格（宽 1 列）：内容用紧凑密度。 */
function isNarrow(size: CardSize): boolean {
  return size === '1x1' || size === '1x2';
}
/** 宽格（宽 3 列）：内容用舒展密度。 */
function isWide(size: CardSize): boolean {
  return size === '3x1' || size === '3x2' || size === '3x3';
}
const WEEK_LABELS = ['1', '2', '3', '4', '5', '6', '日'];

function CardShell({
  title,
  children,
  bodyClassName,
}: {
  title: string;
  children: ReactNode;
  bodyClassName?: string;
}) {
  return (
    <Card padding="none" className="flex h-full flex-col overflow-hidden">
      <div className="border-b border-border/60 px-4 py-3 text-[var(--ui-title-size)] font-semibold">{title}</div>
      <div className={clsx('flex min-h-0 flex-1 flex-col p-[var(--ui-card-pad)]', bodyClassName)}>{children}</div>
    </Card>
  );
}

/* ────────────────────────── 数据轮询 Hooks ────────────────────────── */

function useNetworkLive(): { snapshot: NetworkSnapshotDto | null; points: { t: string; down: number; up: number }[] } {
  const { config } = useAppStore(useShallow((s) => ({ config: s.config })));
  const refreshSeconds = config?.live_refresh_interval_seconds ?? 1;
  const windowSeconds = config?.network_live_window_seconds ?? 300;
  const [snapshot, setSnapshot] = useState<NetworkSnapshotDto | null>(null);
  const [points, setPoints] = useState<{ t: string; down: number; up: number }[]>([]);

  // 实时曲线数据源 = 后端秒级环形缓冲（最近 network_live_window_seconds 秒）：
  // 任何时刻进入页面都能看到完整的最近窗口，切页不丢；采样由后端监控线程统一完成。
  useEffect(() => {
    let disposed = false;
    const pad = (n: number) => String(n).padStart(2, '0');
    const tick = async () => {
      try {
        const [snap, samples] = await Promise.all([
          apiService.getNetworkSnapshot(),
          apiService.getNetworkLiveWindow(windowSeconds),
        ]);
        if (disposed) return;
        setSnapshot(snap);
        setPoints(
          samples.map((s) => {
            const d = new Date(s.ts);
            return {
              t: `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`,
              down: s.down,
              up: s.up,
            };
          }),
        );
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
  }, [refreshSeconds, windowSeconds]);

  return { snapshot, points };
}

function useHardwareData(): { snapshot: HardwareSnapshotDto | null; temp: TemperatureSnapshotDto | null } {
  const { config } = useAppStore(useShallow((s) => ({ config: s.config })));
  const refreshSeconds = config?.live_refresh_interval_seconds ?? 1;
  const [snapshot, setSnapshot] = useState<HardwareSnapshotDto | null>(null);
  const [temp, setTemp] = useState<TemperatureSnapshotDto | null>(null);

  useEffect(() => {
    let disposed = false;
    const tick = async () => {
      try {
        const [hw, tp] = await Promise.all([
          apiService.getHardwareSnapshot(),
          apiService.getTemperatureSnapshot(),
        ]);
        if (disposed) return;
        setSnapshot(hw);
        setTemp(tp);
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
  }, [refreshSeconds]);

  return { snapshot, temp };
}

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
function useLiveActive(tickable: boolean, base: number | undefined): number | undefined {
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

function StatsContent({
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
        const [windows, hourlyData] = await Promise.all([
          apiService.getWindowTitles(app.app_name, focus),
          apiService.getAppHourly(app.app_name, focus),
        ]);
        setAppWindows(windows);
        setAppHourly(hourlyData);
      } catch {
        setAppWindows([]);
        setAppHourly([]);
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
              onClick={() => setSelectedApp(null)}
              className="shrink-0 text-muted-foreground hover:text-foreground"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </div>
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

function HourlyContent({ hourly, size }: { hourly: number[]; size: CardSize }) {
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

function NetworkStatsContent({ snapshot, size }: { snapshot: NetworkSnapshotDto | null; size: CardSize }) {
  const { t } = useTranslation();
  const compact = isNarrow(size);
  const tiles = [
    { icon: <ArrowDown className="h-4 w-4" />, label: t('network.download'), value: formatBytesPerSecond(snapshot?.download_bytes_per_sec ?? 0), color: '#1E88E5' },
    { icon: <ArrowUp className="h-4 w-4" />, label: t('network.upload'), value: formatBytesPerSecond(snapshot?.upload_bytes_per_sec ?? 0), color: '#FB8C00' },
    { icon: <HardDriveDownload className="h-4 w-4" />, label: t('network.sessionDownload'), value: formatBytes(snapshot?.session_download_bytes ?? 0), color: '#1E88E5' },
    { icon: <HardDriveUpload className="h-4 w-4" />, label: t('network.sessionUpload'), value: formatBytes(snapshot?.session_upload_bytes ?? 0), color: '#FB8C00' },
  ];
  return (
    <div className={compact ? 'grid h-full grid-cols-2 grid-rows-2 gap-1.5' : 'grid grid-cols-2 gap-2 lg:grid-cols-4'}>
      {tiles.map((tile) => (
        <div
          key={tile.label}
          className={clsx(
            'flex min-w-0 items-center rounded-lg border border-border/60',
            compact ? 'flex-col gap-0.5 px-1 py-1.5 text-center' : 'gap-2 rounded-xl px-2.5 py-2',
          )}
        >
          <span
            className={clsx(
              'flex shrink-0 items-center justify-center rounded-lg text-white',
              compact ? 'h-6 w-6' : 'h-8 w-8',
            )}
            style={{ backgroundColor: tile.color }}
          >
            {tile.icon}
          </span>
          <div className="min-w-0">
            <div className="truncate text-[10px] leading-tight text-muted-foreground">
              {tile.label}
            </div>
            <div className={clsx('truncate font-semibold tabular-nums leading-tight', compact ? 'text-base' : 'text-sm')}>
              {tile.value}
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}

function NetworkLiveContent({ points, size }: { points: { t: string; down: number; up: number }[]; size: CardSize }) {
  const { t } = useTranslation();
  const compact = isNarrow(size);
  const axisWidth = compact ? 46 : 74;
  // 纵轴缩放系数：1 = 自适应；>1 = 放大（纵轴范围变小）；<1 = 缩小。滚轮/按钮调整，双击复位。
  const [zoom, setZoom] = useState(1);
  const chartRef = useRef<HTMLDivElement | null>(null);
  // 滚轮作用于纵轴：上滚放大、下滚缩小。显式 passive:false 以阻止页面滚动。
  useEffect(() => {
    const el = chartRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      setZoom((z) => Math.min(8, Math.max(0.25, z * (e.deltaY < 0 ? 1.18 : 1 / 1.18))));
    };
    el.addEventListener('wheel', onWheel, { passive: false });
    return () => el.removeEventListener('wheel', onWheel);
  }, []);
  // 纵轴自动缩放：按数据峰值取整 + 10% 余量（1/2/5×10ⁿ 档位），最小 512KB/s 兜底
  // （空闲时曲线也有可看的基线）；滚轮系数叠加在自动档之上。
  const baseMax = useMemo(() => {
    let max = 0;
    for (const p of points) {
      if (p.down > max) max = p.down;
      if (p.up > max) max = p.up;
    }
    return Math.max(niceCeil(max * 1.1), 512 * 1024);
  }, [points]);
  const yMax = Math.max(baseMax / zoom, 64 * 1024);
  const zoomBy = useCallback((f: number) => setZoom((z) => Math.min(8, Math.max(0.25, z * f))), []);
  return (
    <div
      ref={chartRef}
      onDoubleClick={() => setZoom(1)}
      title={t('network.liveZoomHint')}
      className="relative min-h-0 flex-1"
    >
      {points.length === 0 ? (
        <p className="flex flex-1 items-center justify-center text-sm text-muted-foreground">{t('network.noHistory')}</p>
      ) : (
        <>
          <ResponsiveContainer width="100%" height="100%">
            <LineChart data={points} margin={{ left: 0, right: 8 }}>
              <CartesianGrid strokeDasharray="3 3" stroke="var(--chart-grid)" vertical={false} />
              <XAxis
                dataKey="t"
                tick={{ fontSize: 9 }}
                tickFormatter={(v) => String(v).slice(0, 5)}
                interval={Math.max(4, Math.floor(points.length / 10) - 1)}
              />
              <YAxis
                domain={[0, yMax]}
                tick={{ fontSize: compact ? 9 : 10 }}
                tickFormatter={(v) => formatBytesPerSecond(Number(v))}
                width={axisWidth}
              />
              <Tooltip
                cursor={{ stroke: 'var(--chart-axis)', strokeDasharray: '3 3' }}
                content={<ChartTooltip valueFormatter={(v) => formatBytesPerSecond(Number(v))} />}
              />
              <Line type="monotone" name={t('network.download')} dataKey="down" stroke="#1E88E5" dot={false} strokeWidth={2} isAnimationActive={false} />
              <Line type="monotone" name={t('network.upload')} dataKey="up" stroke="#FB8C00" dot={false} strokeWidth={2} isAnimationActive={false} />
            </LineChart>
          </ResponsiveContainer>
          {/* 纵轴缩放选择栏：＋放大 / −缩小 / 复位，滚轮同样生效 */}
          <div className="absolute right-2 top-0 z-10 flex items-center gap-0.5 rounded-md border border-border/60 bg-background/90 px-1 py-0.5 text-muted-foreground shadow-sm">
            <button
              type="button"
              onClick={() => zoomBy(1.25)}
              title={t('network.liveZoomIn')}
              className="flex h-4 w-4 items-center justify-center rounded hover:bg-accent hover:text-foreground"
            >
              <ZoomIn className="h-3 w-3" />
            </button>
            <span className="w-9 text-center text-[10px] tabular-nums">
              {zoom === 1 ? t('network.liveZoomAuto') : `×${zoom.toFixed(2)}`}
            </span>
            <button
              type="button"
              onClick={() => zoomBy(1 / 1.25)}
              title={t('network.liveZoomOut')}
              className="flex h-4 w-4 items-center justify-center rounded hover:bg-accent hover:text-foreground"
            >
              <ZoomOut className="h-3 w-3" />
            </button>
            {zoom !== 1 && (
              <button
                type="button"
                onClick={() => setZoom(1)}
                title={t('network.liveZoomReset')}
                className="flex h-4 w-4 items-center justify-center rounded hover:bg-accent hover:text-foreground"
              >
                <RotateCcw className="h-3 w-3" />
              </button>
            )}
          </div>
        </>
      )}
    </div>
  );
}

function HardwareGaugesContent({
  snapshot,
  temp,
  size,
}: {
  snapshot: HardwareSnapshotDto | null;
  temp: TemperatureSnapshotDto | null;
  size: CardSize;
}) {
  const { t } = useTranslation();
  const cpu = temp?.cpu;
  const gpus = temp?.gpus ?? [];
  const memoryPercent = snapshot
    ? (snapshot.memory_used_bytes / Math.max(1, snapshot.memory_total_bytes)) * 100
    : 0;
  const compact = isNarrow(size);
  // 所有档位都是三列横排：小卡为迷你表盘，中/大卡为标准表盘。
  const labelCls = compact ? 'text-[11px]' : 'text-xs';
  const iconCls = compact ? 'h-3 w-3' : 'h-3.5 w-3.5';
  const numCls = compact ? 'text-xl' : 'text-2xl';
  const unitCls = compact ? 'text-[11px]' : 'text-[11px]';
  const subCls = compact ? 'text-[10px]' : 'text-[11px]';
  const boxCls = compact
    ? 'flex flex-col items-center gap-0.5 rounded-lg border border-border/60 px-1.5 py-2'
    : 'flex flex-col items-center gap-1 rounded-xl border border-border/60 px-2 py-3';
  return (
    <div className="grid grid-cols-3 gap-1.5">
      <div className={boxCls}>
        <div className={clsx('flex items-center gap-1.5 font-medium text-muted-foreground', labelCls)}>
          <Cpu className={clsx('text-primary', iconCls)} />
          CPU
        </div>
        <DualArcGauge temp={cpu?.available ? cpu.temp_celsius : null} usage={snapshot?.cpu_percent}>
          <div className="flex items-baseline gap-0.5">
            <span
              className={clsx('font-bold leading-none tabular-nums', numCls)}
              style={{ color: tempColor(cpu?.available ? cpu.temp_celsius : null) }}
            >
              {cpu?.available ? (cpu.temp_celsius ?? 0).toFixed(0) : '--'}
            </span>
            <span className={clsx('font-medium text-muted-foreground', unitCls)}>°C</span>
          </div>
          <span className={clsx('mt-0.5 leading-none text-muted-foreground', subCls)}>
            {t('hardware.usageLabel', { pct: snapshot ? Math.round(snapshot.cpu_percent) : '--' })}
          </span>
        </DualArcGauge>
      </div>
      <div className={boxCls}>
        <div className={clsx('flex max-w-full items-center gap-1.5 font-medium text-muted-foreground', labelCls)}>
          <Gauge className={clsx('shrink-0 text-primary', iconCls)} />
          <span className="truncate">{gpus[0]?.name || 'GPU'}</span>
        </div>
        <DualArcGauge temp={gpus[0]?.temp_celsius} usage={gpus[0]?.usage_percent}>
          <div className="flex items-baseline gap-0.5">
            <span
              className={clsx('font-bold leading-none tabular-nums', numCls)}
              style={{ color: tempColor(gpus[0]?.temp_celsius) }}
            >
              {gpus[0]?.temp_celsius != null ? gpus[0].temp_celsius.toFixed(0) : '--'}
            </span>
            <span className={clsx('font-medium text-muted-foreground', unitCls)}>°C</span>
          </div>
          <span className={clsx('mt-0.5 leading-none text-muted-foreground', subCls)}>
            {t('hardware.usageLabel', {
              pct: gpus[0]?.usage_percent != null ? Math.round(gpus[0].usage_percent) : '--',
            })}
          </span>
        </DualArcGauge>
      </div>
      <div className={boxCls}>
        <div className={clsx('flex items-center gap-1.5 font-medium text-muted-foreground', labelCls)}>
          <MemoryStick className={clsx('text-primary', iconCls)} />
          {t('hardware.memory')}
        </div>
        <SemiGauge ratio={memoryPercent / 100} color="#1E88E5">
          <div className="flex items-baseline gap-0.5">
            <span
              className={clsx('font-bold leading-none tabular-nums', numCls)}
              style={{ color: levelColor(memoryPercent) }}
            >
              {snapshot ? memoryPercent.toFixed(0) : '--'}
            </span>
            <span className={clsx('font-medium text-muted-foreground', unitCls)}>%</span>
          </div>
          <span className={clsx('mt-0.5 leading-none text-muted-foreground', subCls)}>
            {snapshot
              ? `${formatBytes(snapshot.memory_used_bytes)} / ${formatBytes(snapshot.memory_total_bytes)}`
              : '--'}
          </span>
        </SemiGauge>
      </div>
    </div>
  );
}

function DiskTempContent({ temp }: { temp: TemperatureSnapshotDto | null }) {
  const { t } = useTranslation();
  const disks = temp?.disks ?? [];
  if (disks.length === 0) {
    return <p className="py-4 text-center text-sm text-muted-foreground">{t('hardware.loading')}</p>;
  }
  return (
    <div className="flex min-h-0 flex-col overflow-y-auto">
      {disks.map((d) => (
        <div key={d.drive} className="flex min-h-0 flex-1 items-center justify-between gap-3 rounded-lg px-2 text-sm">
          <span className="min-w-0 truncate">{d.model || d.drive}</span>
          <span className="shrink-0 tabular-nums text-muted-foreground">
            {d.temp_celsius != null ? `${d.temp_celsius.toFixed(1)}°C` : '--'}
          </span>
        </div>
      ))}
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

/* ────────────────────────── 独立卡片 ────────────────────────── */

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

export function NetworkStatsCard({ size }: { size: CardSize }) {
  const { t } = useTranslation();
  const { snapshot } = useNetworkLive();
  return (
    <CardShell title={t('network.statsTitle')}>
      <NetworkStatsContent snapshot={snapshot} size={size} />
    </CardShell>
  );
}

export function NetworkLiveCard({ size }: { size: CardSize }) {
  const { t } = useTranslation();
  const { points } = useNetworkLive();
  return (
    <CardShell title={t('network.liveTitle')}>
      <NetworkLiveContent points={points} size={size} />
    </CardShell>
  );
}

export function HardwareGaugesCard({ size }: { size: CardSize }) {
  const { t } = useTranslation();
  const { snapshot, temp } = useHardwareData();
  return (
    <CardShell title={t('hardware.title')}>
      <HardwareGaugesContent snapshot={snapshot} temp={temp} size={size} />
    </CardShell>
  );
}

export function DiskTempCard({ size }: { size: CardSize }) {
  const { t } = useTranslation();
  const { temp } = useHardwareData();
  return (
    <CardShell title={t('hardware.diskTemp')}>
      <DiskTempContent temp={temp} />
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

/* ────────────────────────── 聚合卡 ────────────────────────── */

export function DurationAggCard({
  data,
  hourly,
  size,
  tickable,
}: {
  data: DashboardDataDto | null;
  hourly: number[];
  size: CardSize;
  tickable?: boolean;
}) {
  const { t } = useTranslation();
  const liveActive = useLiveActive(tickable ?? false, data?.active_seconds);
  return (
    <CardShell title={t('dashboard.cards.durationAgg')}>
      {isNarrow(size) ? (
        <div className="flex items-center justify-between rounded-lg border border-border/60 px-2.5 py-2">
          <span className="text-[11px] text-muted-foreground">{t('dashboard.stats.active')}</span>
          <span className="text-sm font-semibold tabular-nums">
            {data ? formatDurationCompact(liveActive ?? data.active_seconds) : '--'}
          </span>
        </div>
      ) : (
        <StatsContent data={data} size={size} liveActive={liveActive} />
      )}
      <div className="my-2.5 border-t border-border/60" />
      <HourlyContent hourly={hourly} size={size} />
    </CardShell>
  );
}

export function NetAggCard({ size }: { size: CardSize }) {
  const { t } = useTranslation();
  const { snapshot, points } = useNetworkLive();
  return (
    <CardShell title={t('dashboard.cards.netAgg')}>
      {isNarrow(size) ? (
        <div className="flex items-center justify-between gap-2 rounded-lg border border-border/60 px-2.5 py-2 text-[11px]">
          <span className="inline-flex items-center gap-1 text-[#1E88E5]">
            <ArrowDown className="h-3 w-3" />
            {formatBytesPerSecond(snapshot?.download_bytes_per_sec ?? 0)}
          </span>
          <span className="inline-flex items-center gap-1 text-[#FB8C00]">
            <ArrowUp className="h-3 w-3" />
            {formatBytesPerSecond(snapshot?.upload_bytes_per_sec ?? 0)}
          </span>
        </div>
      ) : (
        <NetworkStatsContent snapshot={snapshot} size={size} />
      )}
      <div className="my-2.5 border-t border-border/60" />
      <NetworkLiveContent points={points} size={size} />
    </CardShell>
  );
}

export function HwAggCard({ size }: { size: CardSize }) {
  const { t } = useTranslation();
  const { snapshot, temp } = useHardwareData();
  const diskMaxTemp = (temp?.disks ?? []).reduce<number | null>(
    (acc, d) => (d.temp_celsius != null ? Math.max(acc ?? -Infinity, d.temp_celsius) : acc),
    null,
  );
  return (
    <CardShell title={t('dashboard.cards.hwAgg')}>
      <HardwareGaugesContent snapshot={snapshot} temp={temp} size={size} />
      <div className="my-2.5 border-t border-border/60" />
      {isNarrow(size) ? (
        <div className="flex items-center justify-between rounded-lg border border-border/60 px-2.5 py-2">
          <span className="flex items-center gap-1.5 text-[11px] text-muted-foreground">
            <Thermometer className="h-3 w-3 text-primary" />
            {t('hardware.diskTemp')}
          </span>
          <span className="text-sm font-semibold tabular-nums">
            {diskMaxTemp != null ? `${diskMaxTemp.toFixed(1)}°C` : '--'}
          </span>
        </div>
      ) : (
        <DiskTempContent temp={temp} />
      )}
    </CardShell>
  );
}

export function TempAggCard({ size }: { size: CardSize }) {
  const { t } = useTranslation();
  const { snapshot, temp } = useHardwareData();
  const cpu = temp?.cpu;
  const gpus = temp?.gpus ?? [];
  const disks = temp?.disks ?? [];
  const diskMaxTemp = disks.reduce<number | null>(
    (acc, d) => (d.temp_celsius != null ? Math.max(acc ?? -Infinity, d.temp_celsius) : acc),
    null,
  );
  const compact = isNarrow(size);
  const labelCls = compact ? 'text-[11px]' : 'text-xs';
  const iconCls = compact ? 'h-3 w-3' : 'h-3.5 w-3.5';
  const numCls = compact ? 'text-xl' : 'text-2xl';
  const unitCls = compact ? 'text-[11px]' : 'text-[11px]';
  const subCls = compact ? 'text-[10px]' : 'text-[11px]';
  const boxCls = compact
    ? 'flex flex-col items-center gap-0.5 rounded-lg border border-border/60 px-1.5 py-2'
    : 'flex flex-col items-center gap-1 rounded-xl border border-border/60 px-2 py-3';
  return (
    <CardShell title={t('dashboard.cards.tempAgg')}>
      <div className="grid grid-cols-3 gap-1.5">
        {/* CPU 温度：双弧（橙=温度，蓝=占用） */}
        <div className={boxCls}>
          <div className={clsx('flex items-center gap-1.5 font-medium text-muted-foreground', labelCls)}>
            <Cpu className={clsx('text-primary', iconCls)} />
            {t('hardware.cpuTemp')}
          </div>
          <DualArcGauge temp={cpu?.available ? cpu.temp_celsius : null} usage={snapshot?.cpu_percent}>
            <div className="flex items-baseline gap-0.5">
              <span
                className={clsx('font-bold leading-none tabular-nums', numCls)}
                style={{ color: tempColor(cpu?.available ? cpu.temp_celsius : null) }}
              >
                {cpu?.available ? (cpu.temp_celsius ?? 0).toFixed(0) : '--'}
              </span>
              <span className={clsx('font-medium text-muted-foreground', unitCls)}>°C</span>
            </div>
            <span className={clsx('mt-0.5 leading-none text-muted-foreground', subCls)}>
              {t('hardware.usageLabel', { pct: snapshot ? Math.round(snapshot.cpu_percent) : '--' })}
            </span>
          </DualArcGauge>
        </div>
        {/* GPU 温度：双弧 */}
        <div className={boxCls}>
          <div className={clsx('flex max-w-full items-center gap-1.5 font-medium text-muted-foreground', labelCls)}>
            <Gauge className={clsx('shrink-0 text-primary', iconCls)} />
            <span className="truncate">{t('hardware.gpuTemp')}</span>
          </div>
          <DualArcGauge temp={gpus[0]?.temp_celsius} usage={gpus[0]?.usage_percent}>
            <div className="flex items-baseline gap-0.5">
              <span
                className={clsx('font-bold leading-none tabular-nums', numCls)}
                style={{ color: tempColor(gpus[0]?.temp_celsius) }}
              >
                {gpus[0]?.temp_celsius != null ? gpus[0].temp_celsius.toFixed(0) : '--'}
              </span>
              <span className={clsx('font-medium text-muted-foreground', unitCls)}>°C</span>
            </div>
            <span className={clsx('mt-0.5 leading-none text-muted-foreground', subCls)}>
              {t('hardware.usageLabel', {
                pct: gpus[0]?.usage_percent != null ? Math.round(gpus[0].usage_percent) : '--',
              })}
            </span>
          </DualArcGauge>
        </div>
        {/* 磁盘温度：单弧（橙色温度） */}
        <div className={boxCls}>
          <div className={clsx('flex items-center gap-1.5 font-medium text-muted-foreground', labelCls)}>
            <Thermometer className={clsx('text-primary', iconCls)} />
            {t('hardware.diskTemp')}
          </div>
          <SemiGauge ratio={diskMaxTemp != null ? diskMaxTemp / 100 : 0} color="#FB8C00">
            <div className="flex items-baseline gap-0.5">
              <span
                className={clsx('font-bold leading-none tabular-nums', numCls)}
                style={{ color: tempColor(diskMaxTemp) }}
              >
                {diskMaxTemp != null ? diskMaxTemp.toFixed(0) : '--'}
              </span>
              <span className={clsx('font-medium text-muted-foreground', unitCls)}>°C</span>
            </div>
            <span className={clsx('mt-0.5 leading-none text-muted-foreground', subCls)}>
              {disks.length > 0 ? `${disks.length} 块磁盘` : '--'}
            </span>
          </SemiGauge>
        </div>
      </div>
    </CardShell>
  );
}
