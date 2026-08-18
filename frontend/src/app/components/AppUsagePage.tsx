'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  Line,
  LineChart,
  Pie,
  PieChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { apiService } from '../services/api';
import { useAppStore } from '../store/app-store';
import type { AppPeriodUsageDto, AppUsageDto } from '../types';
import { shiftDayStr, todayStr, weekdayOf } from '../lib/datetime';
import { Card } from './ui/index';
import AppIcon from './dashboard/AppIcon';
import ChartTooltip, { formatAxisSeconds } from './dashboard/ChartTooltip';

type RangeKey = 'today' | 'yesterday' | 'week' | 'month';

const PIE_COLORS = [
  '#2f6df6',
  '#7c5cf0',
  '#0ea5e9',
  '#10b981',
  '#f59e0b',
  '#ef4444',
  '#ec4899',
  '#14b8a6',
  '#8b5cf6',
  '#64748b',
];

/** 截断长应用名（图表 X 轴标签用；悬停 Tooltip 仍显示全名）。 */
function truncateLabel(name: string, max: number): string {
  return name.length > max ? `${name.slice(0, max)}…` : name;
}

/** 日期范围边界按配置时区计算（与应用统计页一致）。 */
function rangeFor(
  key: RangeKey,
  config: { timezone?: string } | null,
): { start: string; end: string; focus: string } {
  const focus = todayStr(config);
  if (key === 'today') return { start: focus, end: focus, focus };
  if (key === 'yesterday') {
    const s = shiftDayStr(focus, -1);
    return { start: s, end: s, focus: s };
  }
  if (key === 'week') {
    const wd = weekdayOf(focus) === 0 ? 7 : weekdayOf(focus);
    return { start: shiftDayStr(focus, -(wd - 1)), end: focus, focus };
  }
  return { start: `${focus.slice(0, 7)}-01`, end: focus, focus };
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

export default function AppUsagePage() {
  const { t } = useTranslation();
  const { config } = useAppStore(useShallow((s) => ({ config: s.config })));
  const [range, setRange] = useState<RangeKey>('today');
  const [apps, setApps] = useState<AppUsageDto[]>([]);
  const [hourly, setHourly] = useState<number[]>([]);
  const [selected, setSelected] = useState<AppUsageDto | null>(null);
  const [windows, setWindows] = useState<{ title: string; seconds: number }[]>([]);
  const [appHourly, setAppHourly] = useState<number[]>([]);
  const [periodUsage, setPeriodUsage] = useState<AppPeriodUsageDto | null>(null);
  const [loading, setLoading] = useState(true);

  const { start, end, focus } = useMemo(() => rangeFor(range, config), [range, config]);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    Promise.all([apiService.getUsageSplit(start, end), apiService.getDayHourly(focus)])
      .then(([list, hourData]) => {
        if (cancelled) return;
        setApps(list.filter((a) => a.active_seconds > 0).sort((a, b) => b.active_seconds - a.active_seconds));
        setHourly(hourData);
        setSelected(null);
        setPeriodUsage(null);
        setLoading(false);
      })
      .catch(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [start, end, focus]);

  const handleSelect = useCallback(
    async (app: AppUsageDto) => {
      setSelected(app);
      try {
        const [winList, hourData, periods] = await Promise.all([
          apiService.getWindowTitles(app.app_name, focus),
          apiService.getAppHourly(app.app_name, focus),
          apiService.getAppPeriodUsage(app.app_name, focus),
        ]);
        setWindows(winList);
        setAppHourly(hourData);
        setPeriodUsage(periods);
      } catch {
        setWindows([]);
        setAppHourly([]);
        setPeriodUsage(null);
      }
    },
    [focus],
  );

  const top10 = useMemo(() => apps.slice(0, 10), [apps]);
  const total = apps.reduce((sum, a) => sum + a.active_seconds, 0);
  const maxApp = top10.reduce((m, a) => Math.max(m, a.active_seconds), 1);
  const pieData = useMemo(
    () =>
      top10.map((a, i) => ({
        name: a.app_name,
        value: a.active_seconds,
        color: PIE_COLORS[i % PIE_COLORS.length],
      })),
    [top10],
  );

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap gap-1.5">
        {(['today', 'yesterday', 'week', 'month'] as RangeKey[]).map((key) => (
          <button
            key={key}
            type="button"
            onClick={() => setRange(key)}
            className={
              'rounded-full border px-3 py-1 text-xs font-medium transition-colors ' +
              (range === key
                ? 'border-primary/30 bg-primary/10 text-primary'
                : 'border-border bg-card text-muted-foreground hover:text-foreground')
            }
          >
            {t(`dashboard.range.${key}`)}
          </button>
        ))}
      </div>

      {loading ? (
        <Card className="py-14 text-center text-sm text-muted-foreground">{t('usage.loading')}</Card>
      ) : apps.length === 0 ? (
        <Card className="py-14 text-center text-sm text-muted-foreground">{t('dashboard.empty')}</Card>
      ) : (
        <>
          {/* 图表区：柱状图 + 饼图并排 */}
          <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
            <Card padding="none" className="overflow-hidden">
              <div className="border-b border-border/60 px-4 py-3 text-sm font-semibold">{t('usage.chart.bar')}</div>
              <div className="h-64 p-4">
                <ResponsiveContainer width="100%" height="100%">
                  <BarChart data={top10.map((a) => ({ name: a.app_name, 时长: a.active_seconds }))} margin={{ left: -14, right: 4 }}>
                    <CartesianGrid strokeDasharray="3 3" stroke="var(--chart-grid)" vertical={false} />
                    <XAxis
                      dataKey="name"
                      tick={{ fontSize: 10, fill: 'var(--chart-tick)' }}
                      interval={0}
                      angle={0}
                      height={32}
                      tickFormatter={(v) => truncateLabel(String(v), 6)}
                    />
                    <YAxis tick={{ fontSize: 10, fill: 'var(--chart-tick)' }} tickFormatter={formatAxisSeconds} width={48} />
                    <Tooltip
                      cursor={{ fill: 'rgba(47,109,246,0.06)' }}
                      content={<ChartTooltip valueFormatter={(v) => formatDuration(Number(v))} />}
                    />
                    <Bar dataKey="时长" fill="var(--chart-primary)" radius={[4, 4, 0, 0]} />
                  </BarChart>
                </ResponsiveContainer>
              </div>
            </Card>

            <Card padding="none" className="overflow-hidden">
              <div className="border-b border-border/60 px-4 py-3 text-sm font-semibold">{t('usage.chart.pie')}</div>
              <div className="h-64 p-4">
                <ResponsiveContainer width="100%" height="100%">
                  <PieChart>
                    <Pie data={pieData} dataKey="value" nameKey="name" innerRadius={48} outerRadius={82} paddingAngle={2}>
                      {pieData.map((entry) => (
                        <Cell key={entry.name} fill={entry.color} />
                      ))}
                    </Pie>
                    <Tooltip
                      content={
                        <ChartTooltip
                          valueFormatter={(v) => {
                            const n = Number(v);
                            const pct = total > 0 ? Math.round((n / total) * 100) : 0;
                            return `${formatDuration(n)} (${pct}%)`;
                          }}
                        />
                      }
                    />
                  </PieChart>
                </ResponsiveContainer>
              </div>
            </Card>
          </div>

          {/* 小时分布 */}
          <Card padding="none" className="overflow-hidden">
            <div className="border-b border-border/60 px-4 py-3 text-sm font-semibold">{t('usage.chart.hourly')}</div>
            <div className="h-52 p-4">
              <ResponsiveContainer width="100%" height="100%">
                <LineChart data={hourly.map((v, i) => ({ h: i, v }))} margin={{ left: -14, right: 8 }}>
                  <CartesianGrid strokeDasharray="3 3" stroke="var(--chart-grid)" vertical={false} />
                  <XAxis dataKey="h" tick={{ fontSize: 10, fill: 'var(--chart-tick)' }} interval={3} />
                  <YAxis tick={{ fontSize: 10, fill: 'var(--chart-tick)' }} tickFormatter={formatAxisSeconds} width={48} />
                  <Tooltip
                    cursor={{ stroke: 'var(--chart-axis)', strokeDasharray: '3 3' }}
                    content={
                      <ChartTooltip
                        labelFormatter={(h) => `${h} 时`}
                        valueFormatter={(v) => formatDuration(Number(v))}
                      />
                    }
                  />
                  <Line type="monotone" name={t('usage.activeLabel')} dataKey="v" stroke="var(--chart-primary)" dot={false} strokeWidth={2} />
                </LineChart>
              </ResponsiveContainer>
            </div>
          </Card>

          {/* 应用列表 */}
          <Card padding="none" className="overflow-hidden">
            <div className="border-b border-border/60 px-4 py-3 text-sm font-semibold">
              {t('usage.title')}
              <span className="ml-2 text-xs font-normal text-muted-foreground">
                {t('usage.total')} {formatDuration(total)}
              </span>
            </div>
            <div className="p-3">
              <div className="space-y-1">
                {apps.map((app) => (
                  <div key={app.exe_path || app.app_name} className="rounded-lg border border-transparent transition-colors hover:border-border">
                    <button
                      type="button"
                      onClick={() => void handleSelect(app)}
                      className="flex w-full items-center gap-3 px-2.5 py-2 text-left"
                    >
                      <AppIcon exePath={app.exe_path} size={26} />
                      <span className="min-w-0 flex-1 truncate text-sm">{app.app_name}</span>
                      <span className="text-xs tabular-nums text-muted-foreground">{formatDuration(app.active_seconds)}</span>
                      <span className="h-1.5 w-24 overflow-hidden rounded-full bg-muted">
                        <span
                          className="block h-full rounded-full bg-primary"
                          style={{ width: `${Math.round((app.active_seconds / maxApp) * 100)}%` }}
                        />
                      </span>
                      <span className="w-10 text-right text-xs tabular-nums text-muted-foreground">
                        {total > 0 ? Math.round((app.active_seconds / total) * 100) : 0}%
                      </span>
                    </button>
                    {selected?.app_name === app.app_name && (
                      <div className="mx-3 mb-2 rounded-lg border border-border/60 bg-background/60 p-3">
                        {periodUsage?.app_name === app.app_name && (
                          <div className="mb-3 grid grid-cols-3 gap-2">
                            {[
                              ['usage.period.today', periodUsage.today_seconds],
                              ['usage.period.week', periodUsage.week_seconds],
                              ['usage.period.month', periodUsage.month_seconds],
                            ].map(([label, seconds]) => (
                              <div key={String(label)} className="min-w-0 rounded border border-border/60 px-2 py-1.5">
                                <div className="truncate text-[11px] text-muted-foreground">{t(String(label))}</div>
                                <div className="truncate text-xs font-semibold tabular-nums">{formatDuration(Number(seconds))}</div>
                              </div>
                            ))}
                          </div>
                        )}
                        {appHourly.length > 0 && (
                          <div className="h-28">
                            <ResponsiveContainer width="100%" height="100%">
                              <LineChart data={appHourly.map((v, i) => ({ h: i, v }))}>
                                <CartesianGrid strokeDasharray="3 3" stroke="var(--chart-grid)" vertical={false} />
                                <XAxis dataKey="h" tick={{ fontSize: 9, fill: 'var(--chart-tick)' }} interval={3} />
                                <YAxis tick={{ fontSize: 9, fill: 'var(--chart-tick)' }} tickFormatter={formatAxisSeconds} width={40} />
                                <Tooltip
                                  cursor={{ stroke: 'var(--chart-axis)', strokeDasharray: '3 3' }}
                                  content={<ChartTooltip valueFormatter={(v) => formatDuration(Number(v))} />}
                                />
                                <Line type="monotone" name={t('usage.activeLabel')} dataKey="v" stroke="var(--chart-primary)" dot={false} strokeWidth={1.5} />
                              </LineChart>
                            </ResponsiveContainer>
                          </div>
                        )}
                        {windows.length > 0 && (
                          <div className="mt-2 space-y-1">
                            {windows.map((w) => (
                              <div key={w.title} className="flex justify-between text-xs text-muted-foreground">
                                <span className="min-w-0 truncate">{w.title}</span>
                                <span className="ml-2 shrink-0 tabular-nums">{formatDuration(w.seconds)}</span>
                              </div>
                            ))}
                          </div>
                        )}
                        {appHourly.length === 0 && windows.length === 0 && (
                          <p className="text-xs text-muted-foreground">{t('usage.noDetail')}</p>
                        )}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            </div>
          </Card>
        </>
      )}
    </div>
  );
}
