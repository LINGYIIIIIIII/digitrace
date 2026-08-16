// 网络域卡片：实时数据 hook、统计内容、实时曲线内容与独立卡片。
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ArrowDown, ArrowUp, HardDriveDownload, HardDriveUpload, RotateCcw, ZoomIn, ZoomOut } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { CartesianGrid, Line, LineChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from 'recharts';
import type { NetworkSnapshotDto } from '../../types';
import { useNetworkLiveShared } from '../../lib/network-live-store';
import ChartTooltip from './ChartTooltip';
import { formatBytes } from './gauges';
import type { CardSize } from './dashboard-layout';
import { CardShell, clsx, formatBytesPerSecond, isNarrow, niceCeil } from './card-common';
export function useNetworkLive(): { snapshot: NetworkSnapshotDto | null; points: { t: string; down: number; up: number }[] } {
  // 共享单例轮询：仪表盘各卡共用同一轮询器与状态（lib/network-live-store.ts），
  // 避免每张卡各自每秒拉快照 + 300 点窗口造成重复请求与 GC 压力。
  return useNetworkLiveShared();
}


export function NetworkStatsContent({ snapshot, size }: { snapshot: NetworkSnapshotDto | null; size: CardSize }) {
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


export function NetworkLiveContent({ points, size }: { points: { t: string; down: number; up: number }[]; size: CardSize }) {
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



