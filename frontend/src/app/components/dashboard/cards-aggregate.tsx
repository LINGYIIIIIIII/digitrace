// 聚合卡：把多张单独卡合并为一张（时长/网络/硬件/温度）。
import { ArrowDown, ArrowUp, Cpu, Gauge, Thermometer } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { DashboardDataDto } from '../../types';
import type { CardSize } from './dashboard-layout';
import { CardShell, clsx, formatBytesPerSecond, formatDurationCompact, isNarrow } from './card-common';
import { DualArcGauge, SemiGauge, tempColor } from './gauges';
import { NetworkLiveContent, NetworkStatsContent, useNetworkLive } from './cards-network';
import { DiskTempContent, HardwareGaugesContent, useHardwareData } from './cards-hardware';
import { HourlyContent, StatsContent, useLiveActive } from './cards-usage';
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

