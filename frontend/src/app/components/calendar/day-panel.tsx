'use client';

import { useEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import { ArrowLeft } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Area, AreaChart, Bar, BarChart, CartesianGrid, Line, LineChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from 'recharts';
import { apiService } from '../../services/api';
import type { DayDetailDto, DayMetricsDto } from '../../types';
import { Card } from '../ui/index';
import ChartTooltip from '../dashboard/ChartTooltip';
import { formatBytes, formatDuration, fmtMin } from './calendar-common';
const APP_COLORS = ['#2f6df6', '#7c5cf0', '#0ea5e9', '#10b981', '#f59e0b', '#ec4899'];
function SectionTitle({ children }: { children: ReactNode }) {
  return <div className="border-b border-border/60 px-4 py-3 text-sm font-semibold">{children}</div>;
}

/** 某日历日的仪表盘：24h 活跃 / 应用 / 硬件 / 网络 / 会话。 */
export function DayPanel({ date, onBack }: { date: string; onBack: () => void }) {
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
          apiService.getDayHourApps(date),
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

  const gpuPowerData = useMemo(() => {
    const raw = (metrics?.gpu_power_watts ?? []).map((p) => ({
      t: fmtMin(p.minute),
      watts: p.avg,
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

  const hasHw = cpuData.length > 0 || tempData.length > 0 || gpuPowerData.length > 0;
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
            {gpuPowerData.length > 0 && (
              <div className="h-40 border-t border-border/60 p-4">
                <ResponsiveContainer width="100%" height="100%">
                  <LineChart data={gpuPowerData} margin={{ left: -14, right: 8 }}>
                    <CartesianGrid strokeDasharray="3 3" stroke="var(--chart-grid)" vertical={false} />
                    <XAxis dataKey="t" tick={{ fontSize: 9, fill: 'var(--chart-tick)' }} interval="preserveStartEnd" />
                    <YAxis
                      tick={{ fontSize: 10, fill: 'var(--chart-tick)' }}
                      width={42}
                      tickFormatter={(v) => `${Number(v).toFixed(0)}W`}
                    />
                    <Tooltip
                      cursor={{ stroke: 'var(--chart-axis)', strokeDasharray: '3 3' }}
                      content={<ChartTooltip valueFormatter={(v) => `${Number(v).toFixed(1)} W`} />}
                    />
                    <Line
                      type="monotone"
                      dataKey="watts"
                      name={t('calendar.gpuPower')}
                      stroke="#8e24aa"
                      strokeWidth={2}
                      dot={false}
                      isAnimationActive={false}
                    />
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



