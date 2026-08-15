'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import {
  Bar,
  BarChart,
  CartesianGrid,
  Legend,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts';
import { ArrowDown, ArrowUp, HardDriveDownload, HardDriveUpload } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../store/app-store';
import { apiService } from '../services/api';
import { useNetworkLiveShared } from '../lib/network-live-store';
import type { HistoryPointDto } from '../types';
import AttributedUsageCard from './AttributedUsageCard';
import NetAppsCard from './NetAppsCard';
import { Card } from './ui/index';
import ChartTooltip from './dashboard/ChartTooltip';

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

type WindowKey = 5 | 10 | 30 | 60;
type RangeMode = '24h' | 'today' | 'session' | '7d' | '30d';
const RANGE_MODES: RangeMode[] = ['24h', 'today', 'session', '7d', '30d'];

export default function NetworkPage() {
  const { t } = useTranslation();
  const { config } = useAppStore(useShallow((s) => ({ config: s.config })));
  // 实时快照 + 秒级曲线窗口：与仪表盘共用同一单例轮询（lib/network-live-store.ts），
  // 避免本页再开一路每秒轮询（重复请求与 GC 压力）。
  const { snapshot, points: livePoints } = useNetworkLiveShared();
  const [rangeMode, setRangeMode] = useState<RangeMode>('7d');
  const [historyDown, setHistoryDown] = useState<HistoryPointDto[]>([]);
  const [historyUp, setHistoryUp] = useState<HistoryPointDto[]>([]);
  const [selectedDay, setSelectedDay] = useState<string | null>(null);
  const [historyWindow, setHistoryWindow] = useState<WindowKey>(10);

  // 历史（下载 + 上传），范围切换时加载。
  useEffect(() => {
    let cancelled = false;
    Promise.all([apiService.getNetworkHistory(rangeMode), apiService.getNetworkHistoryUp(rangeMode)])
      .then(([down, up]) => {
        if (cancelled) return;
        const sortFn = (a: HistoryPointDto, b: HistoryPointDto) =>
          a.day === b.day ? a.minute - b.minute : a.day.localeCompare(b.day);
        setHistoryDown([...down].sort(sortFn));
        setHistoryUp([...up].sort(sortFn));
        setSelectedDay(null);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [rangeMode]);

  // 按天聚合：分钟 avg(bytes/s) × 60 = 该分钟流量；同时统计峰值与活跃天数。
  const stats = useMemo(() => {
    const byDay = new Map<string, { down: number; up: number; maxDown: number; maxUp: number }>();
    const add = (list: HistoryPointDto[], isUp: boolean) => {
      for (const p of list) {
        const cur = byDay.get(p.day) ?? { down: 0, up: 0, maxDown: 0, maxUp: 0 };
        if (isUp) {
          cur.up += p.avg * 60;
          cur.maxUp = Math.max(cur.maxUp, p.max);
        } else {
          cur.down += p.avg * 60;
          cur.maxDown = Math.max(cur.maxDown, p.max);
        }
        byDay.set(p.day, cur);
      }
    };
    add(historyDown, false);
    add(historyUp, true);

    const days = [...byDay.entries()].sort((a, b) => a[0].localeCompare(b[0]));
    let totalDown = 0;
    let totalUp = 0;
    let peakDown = 0;
    let peakUp = 0;
    for (const [, v] of days) {
      totalDown += v.down;
      totalUp += v.up;
      peakDown = Math.max(peakDown, v.maxDown);
      peakUp = Math.max(peakUp, v.maxUp);
    }
    const activeDays = days.filter(([, v]) => v.down + v.up > 0).length;
    return {
      daily: days.map(([day, v]) => ({ day, label: day.slice(5), down: v.down, up: v.up })),
      totalDown,
      totalUp,
      peakDown,
      peakUp,
      activeDays,
    };
  }, [historyDown, historyUp]);

  // 选中天的小时聚合。
  const selectedHourly = useMemo(() => {
    if (!selectedDay) return [];
    const hours = new Array(24).fill(0) as number[];
    const add = (list: HistoryPointDto[], isUp: boolean) => {
      for (const p of list) {
        if (p.day !== selectedDay) continue;
        const h = Math.min(23, Math.floor(p.minute / 60));
        hours[h] += p.avg * 60;
      }
    };
    add(historyDown, false);
    add(historyUp, true);
    return hours.map((v, h) => ({ h, v }));
  }, [selectedDay, historyDown, historyUp]);

  const historyWindowed = useMemo(() => {
    const minutes = historyDown.slice(-historyWindow);
    return minutes.map((p) => {
      const h = Math.floor(p.minute / 60);
      const m = p.minute % 60;
      return {
        label: `${p.day.slice(5)} ${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}`,
        avgDown: p.avg,
        maxDown: p.max,
      };
    });
  }, [historyDown, historyWindow]);

  const renderRateCard = (icon: ReactNode, label: string, value: number, color: string, perSecond = true) => (
    <Card className="flex items-center gap-3">
      <span className="flex h-10 w-10 items-center justify-center rounded-xl text-white" style={{ backgroundColor: color }}>
        {icon}
      </span>
      <div>
        <div className="text-xs text-muted-foreground">{label}</div>
        <div className="text-lg font-semibold tabular-nums">{formatBytes(value, perSecond)}</div>
      </div>
    </Card>
  );

  return (
    <div className="space-y-4">
      {/* 流量统计面板（顶部） */}
      <div className="flex flex-wrap items-center justify-between gap-2">
        <span className="text-sm font-semibold">{t('network.statsTitle')}</span>
        <div className="flex gap-1">
          {RANGE_MODES.map((m) => (
            <button
              key={m}
              type="button"
              onClick={() => setRangeMode(m)}
              className={
                'rounded-full border px-3 py-1 text-xs font-medium transition-colors ' +
                (rangeMode === m
                  ? 'border-primary/30 bg-primary/10 text-primary'
                  : 'border-border text-muted-foreground hover:text-foreground')
              }
            >
              {t(`network.range.${m}`)}
            </button>
          ))}
        </div>
      </div>

      <div className="grid grid-cols-2 gap-3 xl:grid-cols-4">
        {renderRateCard(<HardDriveDownload className="h-5 w-5" />, t('network.totalDownload'), stats.totalDown, '#1E88E5', false)}
        {renderRateCard(<HardDriveUpload className="h-5 w-5" />, t('network.totalUpload'), stats.totalUp, '#FB8C00', false)}
        {renderRateCard(<ArrowDown className="h-5 w-5" />, t('network.peakDownload'), stats.peakDown, '#1E88E5', true)}
        {renderRateCard(<ArrowUp className="h-5 w-5" />, t('network.peakUpload'), stats.peakUp, '#FB8C00', true)}
      </div>
      <p className="-mt-2 text-xs text-muted-foreground">
        {t('network.activeDays', { count: stats.activeDays })} · {t('network.dailyAvg', { value: formatBytes(stats.activeDays > 0 ? (stats.totalDown + stats.totalUp) / stats.activeDays : 0) })}
      </p>

      <Card padding="none" className="overflow-hidden">
        <div className="border-b border-border/60 px-4 py-3 text-sm font-semibold">{t('network.dailyTitle')}</div>
        <div className="h-56 p-4">
          {stats.daily.length === 0 ? (
            <p className="flex h-full items-center justify-center text-sm text-muted-foreground">{t('network.noHistory')}</p>
          ) : (
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={stats.daily} margin={{ left: -14, right: 8 }}>
                <CartesianGrid strokeDasharray="3 3" stroke="var(--chart-grid)" vertical={false} />
                <XAxis dataKey="label" tick={{ fontSize: 10, fill: 'var(--chart-tick)' }} />
                <YAxis tick={{ fontSize: 10, fill: 'var(--chart-tick)' }} tickFormatter={(v) => formatBytes(Number(v))} width={60} />
                <Tooltip
                  cursor={{ fill: 'rgba(47,109,246,0.06)' }}
                  content={<ChartTooltip valueFormatter={(v) => formatBytes(Number(v))} />}
                />
                <Legend formatter={(value) => (value === 'down' ? t('network.download') : t('network.upload'))} />
                <Bar
                  name={t('network.download')}
                  dataKey="down"
                  fill="#1E88E5"
                  radius={[3, 3, 0, 0]}
                  isAnimationActive={false}
                  onClick={((data: { payload?: { day?: string } }) => setSelectedDay(data.payload?.day ?? null)) as never}
                />
                <Bar name={t('network.upload')} dataKey="up" fill="#FB8C00" radius={[3, 3, 0, 0]} isAnimationActive={false} />
              </BarChart>
            </ResponsiveContainer>
          )}
        </div>
      </Card>

      {selectedDay && (
        <Card padding="none" className="overflow-hidden">
          <div className="border-b border-border/60 px-4 py-3 text-sm font-semibold">
            {t('network.hourlyTitle')} {selectedDay}
          </div>
          <div className="h-48 p-4">
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={selectedHourly} margin={{ left: -14, right: 8 }}>
                <CartesianGrid strokeDasharray="3 3" stroke="var(--chart-grid)" vertical={false} />
                <XAxis dataKey="h" tick={{ fontSize: 10, fill: 'var(--chart-tick)' }} interval={3} />
                <YAxis tick={{ fontSize: 10, fill: 'var(--chart-tick)' }} tickFormatter={(v) => formatBytes(Number(v))} width={60} />
                <Tooltip
                  cursor={{ stroke: 'var(--chart-axis)', strokeDasharray: '3 3' }}
                  content={
                    <ChartTooltip
                      labelFormatter={(h) => `${h} 时`}
                      valueFormatter={(v) => formatBytes(Number(v))}
                    />
                  }
                />
                <Line type="monotone" name={t('network.download')} dataKey="v" stroke="#1E88E5" dot={false} strokeWidth={2} isAnimationActive={false} />
              </LineChart>
            </ResponsiveContainer>
          </div>
        </Card>
      )}

      {/* 实时卡片 */}
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">
        {renderRateCard(<ArrowDown className="h-5 w-5" />, t('network.download'), snapshot?.download_bytes_per_sec ?? 0, '#1E88E5', true)}
        {renderRateCard(<ArrowUp className="h-5 w-5" />, t('network.upload'), snapshot?.upload_bytes_per_sec ?? 0, '#FB8C00', true)}
        {renderRateCard(<HardDriveDownload className="h-5 w-5" />, t('network.sessionDownload'), snapshot?.session_download_bytes ?? 0, '#1E88E5', false)}
        {renderRateCard(<HardDriveUpload className="h-5 w-5" />, t('network.sessionUpload'), snapshot?.session_upload_bytes ?? 0, '#FB8C00', false)}
      </div>

      {/* 实时曲线 */}
      <Card padding="none" className="overflow-hidden">
        <div className="border-b border-border/60 px-4 py-3 text-sm font-semibold">{t('network.liveTitle')}</div>
        <div className="h-56 p-4">
          <ResponsiveContainer width="100%" height="100%">
            <LineChart data={livePoints} margin={{ left: -14, right: 8 }}>
              <CartesianGrid strokeDasharray="3 3" stroke="var(--chart-grid)" vertical={false} />
              <XAxis
                dataKey="t"
                tick={{ fontSize: 9 }}
                tickFormatter={(v) => String(v).slice(0, 5)}
                interval={Math.max(4, Math.floor(livePoints.length / 10) - 1)}
              />
              <YAxis tick={{ fontSize: 10 }} tickFormatter={(v) => formatBytes(Number(v), true)} width={74} />
              <Tooltip
                cursor={{ stroke: 'var(--chart-axis)', strokeDasharray: '3 3' }}
                content={<ChartTooltip valueFormatter={(v) => formatBytes(Number(v), true)} />}
              />
              <Legend formatter={(value) => (value === 'down' ? t('network.download') : t('network.upload'))} />
              <Line type="monotone" name={t('network.download')} dataKey="down" stroke="#1E88E5" dot={false} strokeWidth={2} isAnimationActive={false} />
              <Line type="monotone" name={t('network.upload')} dataKey="up" stroke="#FB8C00" dot={false} strokeWidth={2} isAnimationActive={false} />
            </LineChart>
          </ResponsiveContainer>
        </div>
      </Card>

      {/* 历史曲线（下载 avg/max） */}
      <Card padding="none" className="overflow-hidden">
        <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border/60 px-4 py-3">
          <span className="text-sm font-semibold">{t('network.historyTitle')}</span>
          <div className="flex gap-1">
            {([5, 10, 30, 60] as WindowKey[]).map((w) => (
              <button
                key={w}
                type="button"
                onClick={() => setHistoryWindow(w)}
                className={
                  'rounded-full border px-2.5 py-0.5 text-xs transition-colors ' +
                  (historyWindow === w
                    ? 'border-primary/30 bg-primary/10 text-primary'
                    : 'border-border text-muted-foreground hover:text-foreground')
                }
              >
                {w} 分
              </button>
            ))}
          </div>
        </div>
        <div className="h-56 p-4">
          {historyWindowed.length === 0 ? (
            <p className="flex h-full items-center justify-center text-sm text-muted-foreground">{t('network.noHistory')}</p>
          ) : (
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={historyWindowed} margin={{ left: -14, right: 8 }}>
                <CartesianGrid strokeDasharray="3 3" stroke="var(--chart-grid)" vertical={false} />
                <XAxis dataKey="label" tick={{ fontSize: 9 }} interval="preserveStartEnd" />
                <YAxis tick={{ fontSize: 10 }} tickFormatter={(v) => formatBytes(Number(v), true)} width={74} />
                <Tooltip
                  cursor={{ stroke: 'var(--chart-axis)', strokeDasharray: '3 3' }}
                  content={<ChartTooltip valueFormatter={(v) => formatBytes(Number(v), true)} />}
                />
                <Legend formatter={(value) => (value === 'avgDown' ? t('network.avg') : t('network.max'))} />
                <Line type="monotone" name={t('network.avg')} dataKey="avgDown" stroke="#1E88E5" dot={false} strokeWidth={2} isAnimationActive={false} />
                <Line type="monotone" name={t('network.max')} dataKey="maxDown" stroke="#90CAF9" dot={false} strokeWidth={1.5} strokeDasharray="4 4" isAnimationActive={false} />
              </LineChart>
            </ResponsiveContainer>
          )}
        </div>
      </Card>

      {/* 按应用流量 */}
      <NetAppsCard title={t('network.appTraffic')} limit={15} />

      {/* 按应用流量（Windows 官方数据，免管理员、非实时累计） */}
      <AttributedUsageCard title={t('network.attrTitle')} limit={15} />
    </div>
  );
}
