// 硬件域卡片：硬件/温度数据 hook、表盘内容、磁盘温度与独立卡片。
import { useEffect, useState } from 'react';
import { Cpu, Gauge, MemoryStick } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { apiService } from '../../services/api';
import { useAppStore } from '../../store/app-store';
import type { HardwareSnapshotDto, TemperatureSnapshotDto } from '../../types';
import { DualArcGauge, SemiGauge, formatBytes, levelColor, tempColor } from './gauges';
import type { CardSize } from './dashboard-layout';
import { CardShell, clsx, isNarrow } from './card-common';
export function useHardwareData(): { snapshot: HardwareSnapshotDto | null; temp: TemperatureSnapshotDto | null } {
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


export function HardwareGaugesContent({
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


export function DiskTempContent({ temp }: { temp: TemperatureSnapshotDto | null }) {
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



