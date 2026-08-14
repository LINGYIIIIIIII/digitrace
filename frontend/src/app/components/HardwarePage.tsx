'use client';

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Cpu, Gauge, HardDrive, MemoryStick, ShieldAlert, Thermometer } from 'lucide-react';
import {
  CartesianGrid,
  Legend,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../store/app-store';
import { apiService } from '../services/api';
import type { DiskHealthDto, HardwareSnapshotDto, TemperatureSnapshotDto } from '../types';
import { Button, Card } from './ui/index';
import ChartTooltip from './dashboard/ChartTooltip';
import {
  DualArcGauge,
  SemiGauge,
  StatCard,
  formatBytes,
  levelColor,
  tempColor,
  timeStr,
} from './dashboard/gauges';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';

type WindowKey = 5 | 10 | 30 | 60;

type LivePoint = {
  t: string;
  cpuPct: number | null;
  memPct: number | null;
  cpuTemp: number | null;
  gpuTemp: number | null;
};

// 曲线配色：占用率蓝/橙，温度红/绿，一眼可分。
const LINE_COLORS = {
  cpuPct: '#1E88E5',
  memPct: '#FB8C00',
  cpuTemp: '#E53935',
  gpuTemp: '#43A047',
};

// 本地最多保留 3600 个点（1 秒轮询可覆盖 60 分钟窗口）。
const MAX_POINTS = 3600;

export default function HardwarePage() {
  const { t } = useTranslation();
  const { config } = useAppStore(useShallow((s) => ({ config: s.config })));
  const refreshSeconds = config?.live_refresh_interval_seconds ?? 1;

  const [snapshot, setSnapshot] = useState<HardwareSnapshotDto | null>(null);
  const [temp, setTemp] = useState<TemperatureSnapshotDto | null>(null);
  const [diskHealth, setDiskHealth] = useState<DiskHealthDto[]>([]);
  const [diskHealthBusy, setDiskHealthBusy] = useState(false);
  const [error, setError] = useState(false);
  const [windowMin, setWindowMin] = useState<WindowKey>(10);
  const [installOpen, setInstallOpen] = useState(false);
  const [driverBusy, setDriverBusy] = useState(false);
  const liveRef = useRef<LivePoint[]>([]);
  const [live, setLive] = useState<LivePoint[]>([]);

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
        setError(false);
        const now = new Date();
        const point: LivePoint = {
          t: timeStr(now),
          cpuPct: hw.cpu_percent,
          memPct: hw.memory_total_bytes > 0 ? (hw.memory_used_bytes / hw.memory_total_bytes) * 100 : null,
          cpuTemp: tp.cpu.available ? (tp.cpu.temp_celsius ?? null) : null,
          gpuTemp: tp.gpus[0]?.temp_celsius ?? null,
        };
        const next = [...liveRef.current, point];
        if (next.length > MAX_POINTS) next.shift();
        liveRef.current = next;
        setLive(next);
      } catch {
        if (!disposed) setError(true);
      }
    };
    void tick();
    const timer = window.setInterval(() => void tick(), refreshSeconds * 1000);
    return () => {
      disposed = true;
      window.clearInterval(timer);
    };
  }, [refreshSeconds]);

  // 磁盘健康自动查询一天一次（后端 24 小时缓存）：进页面加载一次，
  // 需要最新数据时点卡片上的「刷新」强制查询。
  const loadDiskHealth = useCallback(async (force: boolean) => {
    setDiskHealthBusy(force);
    try {
      const dh = await apiService.getDiskHealth(force);
      setDiskHealth(dh);
    } catch {
      /* 静默降级：读不到健康数据不影响页面 */
    } finally {
      setDiskHealthBusy(false);
    }
  }, []);

  useEffect(() => {
    void loadDiskHealth(false);
  }, [loadDiskHealth]);

  const windowed = useMemo(() => live.slice(-windowMin * 60), [live, windowMin]);

  const cpu = temp?.cpu;
  const gpus = temp?.gpus ?? [];
  const diskTemps = temp?.disks ?? [];
  const memoryPercent = snapshot
    ? (snapshot.memory_used_bytes / Math.max(1, snapshot.memory_total_bytes)) * 100
    : 0;
  const diskMaxTemp = diskTemps.reduce<number | null>(
    (acc, d) => (d.temp_celsius != null ? Math.max(acc ?? -Infinity, d.temp_celsius) : acc),
    null,
  );

  const handleInstallDriver = async () => {
    setDriverBusy(true);
    try {
      const res = await apiService.installPawnioDriver();
      if (res.ok) toast.success(res.message);
      else toast.error(res.message);
      setInstallOpen(false);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    } finally {
      setDriverBusy(false);
    }
  };

  const handleRestartElevated = async () => {
    setDriverBusy(true);
    try {
      const res = await apiService.restartElevated();
      if (res.ok) toast.success(res.message);
      else toast.error(res.message);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    } finally {
      setDriverBusy(false);
    }
  };

  if (error && !snapshot) {
    return (
      <div className="mx-auto max-w-2xl rounded-2xl border border-destructive/30 bg-destructive/5 p-6 text-center text-sm text-destructive">
        {t('hardware.loadFailed')}
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* 顶部表盘：CPU / GPU 双弧（橙=温度，蓝=占用），内存单弧 */}
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
        {/* CPU：双弧 + 各核心温度详情 */}
        <Card className="flex flex-col items-center gap-3 py-4">
          <div className="flex items-center gap-2 text-sm font-medium text-muted-foreground">
            <Cpu className="h-4 w-4 text-primary" />
            CPU
          </div>
          <DualArcGauge temp={cpu?.available ? cpu.temp_celsius : null} usage={snapshot?.cpu_percent}>
            <div className="flex items-baseline gap-0.5">
              <span
                className="text-[28px] font-bold leading-none tabular-nums tracking-tight"
                style={{ color: tempColor(cpu?.available ? cpu.temp_celsius : null) }}
              >
                {cpu?.available ? (cpu.temp_celsius ?? 0).toFixed(0) : '--'}
              </span>
              <span className="text-xs font-medium text-muted-foreground">°C</span>
            </div>
            <span className="mt-1 text-[11px] leading-none text-muted-foreground">
              {t('hardware.usageLabel', { pct: snapshot ? Math.round(snapshot.cpu_percent) : '--' })}
            </span>
          </DualArcGauge>
          {cpu?.available && (
            <p className="text-[11px] leading-none text-muted-foreground">
              {t('hardware.avgTempHint', { cores: cpu.per_core.length })}
            </p>
          )}
        </Card>

        {/* GPU：双弧 + 名称 */}
        <Card className="flex flex-col items-center gap-3 py-4">
          <div className="flex max-w-full items-center gap-2 text-sm font-medium text-muted-foreground">
            <Gauge className="h-4 w-4 shrink-0 text-primary" />
            <span className="truncate">{gpus[0]?.name || 'GPU'}</span>
          </div>
          <DualArcGauge temp={gpus[0]?.temp_celsius} usage={gpus[0]?.usage_percent}>
            <div className="flex items-baseline gap-0.5">
              <span
                className="text-[28px] font-bold leading-none tabular-nums tracking-tight"
                style={{ color: tempColor(gpus[0]?.temp_celsius) }}
              >
                {gpus[0]?.temp_celsius != null ? gpus[0].temp_celsius.toFixed(0) : '--'}
              </span>
              <span className="text-xs font-medium text-muted-foreground">°C</span>
            </div>
            <span className="mt-1 text-[11px] leading-none text-muted-foreground">
              {t('hardware.usageLabel', {
                pct: gpus[0]?.usage_percent != null ? Math.round(gpus[0].usage_percent) : '--',
              })}
            </span>
          </DualArcGauge>
        </Card>

        {/* 内存：单弧（蓝色占用率） */}
        <Card className="flex flex-col items-center gap-3 py-4">
          <div className="flex items-center gap-2 text-sm font-medium text-muted-foreground">
            <MemoryStick className="h-4 w-4 text-primary" />
            {t('hardware.memory')}
          </div>
          <SemiGauge ratio={memoryPercent / 100} color="#1E88E5">
            <div className="flex items-baseline gap-0.5">
              <span
                className="text-[28px] font-bold leading-none tabular-nums tracking-tight"
                style={{ color: levelColor(memoryPercent) }}
              >
                {snapshot ? memoryPercent.toFixed(0) : '--'}
              </span>
              <span className="text-xs font-medium text-muted-foreground">%</span>
            </div>
            <span className="mt-1 text-[11px] leading-none text-muted-foreground">
              {snapshot
                ? `${formatBytes(snapshot.memory_used_bytes)} / ${formatBytes(snapshot.memory_total_bytes)}`
                : '--'}
            </span>
          </SemiGauge>
        </Card>
      </div>

      {/* 磁盘温度小卡 */}
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
        <StatCard
          icon={<Thermometer className="h-4 w-4" />}
          label={t('hardware.diskTemp')}
          value={diskMaxTemp != null ? `${diskMaxTemp.toFixed(0)}` : '--'}
          unit="°C"
          sub={diskTemps.length > 0 ? `${diskTemps.length} 块磁盘` : t('hardware.loading')}
          color="#FB8C00"
        />
      </div>

      {/* 实时曲线：占用率 + 温度合并一张图，四种颜色 */}
      <Card padding="none" className="overflow-hidden">
        <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border/60 px-4 py-3">
          <span className="text-sm font-semibold">{t('hardware.liveTitle')}</span>
          <div className="flex gap-1">
            {([5, 10, 30, 60] as WindowKey[]).map((w) => (
              <button
                key={w}
                type="button"
                onClick={() => setWindowMin(w)}
                className={
                  'rounded-full border px-2.5 py-0.5 text-xs transition-colors ' +
                  (windowMin === w
                    ? 'border-primary/30 bg-primary/10 text-primary'
                    : 'border-border text-muted-foreground hover:text-foreground')
                }
              >
                {w} 分
              </button>
            ))}
          </div>
        </div>
        <div className="h-64 p-4">
          {windowed.length === 0 ? (
            <p className="flex h-full items-center justify-center text-sm text-muted-foreground">
              {t('hardware.loading')}
            </p>
          ) : (
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={windowed} margin={{ left: -14, right: 8 }}>
                <CartesianGrid strokeDasharray="3 3" stroke="var(--chart-grid)" vertical={false} />
                <XAxis dataKey="t" tick={{ fontSize: 9 }} interval={Math.floor(windowed.length / 8)} />
                <YAxis
                  yAxisId="pct"
                  domain={[0, 100]}
                  tick={{ fontSize: 10, fill: 'var(--chart-tick)' }}
                  width={38}
                />
                <YAxis
                  yAxisId="temp"
                  orientation="right"
                  domain={[0, 100]}
                  tick={{ fontSize: 10, fill: 'var(--chart-tick)' }}
                  width={42}
                />
                <Tooltip
                  cursor={{ stroke: 'var(--chart-axis)', strokeDasharray: '3 3' }}
                  content={<ChartTooltip valueFormatter={(v) => Number(v).toFixed(1)} />}
                />
                <Legend />
                <Line
                  yAxisId="pct"
                  type="monotone"
                  name={t('hardware.cpuPct')}
                  dataKey="cpuPct"
                  stroke={LINE_COLORS.cpuPct}
                  dot={false}
                  strokeWidth={2}
                  isAnimationActive={false}
                />
                <Line
                  yAxisId="pct"
                  type="monotone"
                  name={t('hardware.memPct')}
                  dataKey="memPct"
                  stroke={LINE_COLORS.memPct}
                  dot={false}
                  strokeWidth={2}
                  isAnimationActive={false}
                />
                <Line
                  yAxisId="temp"
                  type="monotone"
                  name={t('hardware.cpuTempLine')}
                  dataKey="cpuTemp"
                  stroke={LINE_COLORS.cpuTemp}
                  dot={false}
                  strokeWidth={2}
                  isAnimationActive={false}
                />
                <Line
                  yAxisId="temp"
                  type="monotone"
                  name={t('hardware.gpuTempLine')}
                  dataKey="gpuTemp"
                  stroke={LINE_COLORS.gpuTemp}
                  dot={false}
                  strokeWidth={2}
                  isAnimationActive={false}
                />
              </LineChart>
            </ResponsiveContainer>
          )}
        </div>
        <p className="border-t border-border/60 px-4 py-2 text-xs text-muted-foreground">
          {t('hardware.liveHint', { seconds: refreshSeconds })}
        </p>
      </Card>

      {/* CPU 温度 + 驱动状态 */}
      <Card className="gap-3">
        <div className="flex items-center gap-3">
          <span className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl border border-primary/15 bg-primary/10 text-primary">
            <Thermometer className="h-5 w-5" />
          </span>
          <div className="min-w-0 flex-1">
            <div className="flex items-baseline justify-between gap-2">
              <span className="text-sm font-medium">{t('hardware.cpuTemp')}</span>
              <span className="text-xl font-semibold tabular-nums">
                {cpu?.available ? `${(cpu.temp_celsius ?? 0).toFixed(1)}°C` : '--'}
              </span>
            </div>
          </div>
        </div>
        {cpu?.available ? (
          <>
            {cpu.per_core.length > 0 && (
              <div className="flex flex-wrap gap-1.5 pt-2">
                {cpu.per_core.map((v, i) => (
                  <span
                    key={i}
                    className="rounded-md bg-muted px-2 py-0.5 text-xs tabular-nums text-muted-foreground"
                    title={`${t('hardware.core')} ${i}`}
                  >
                    {v.toFixed(0)}°C
                  </span>
                ))}
              </div>
            )}
            {cpu.package_celsius != null && (
              <p className="pt-2 text-xs text-muted-foreground">
                {t('hardware.packageTemp')} {(cpu.package_celsius ?? 0).toFixed(1)}°C
              </p>
            )}
          </>
        ) : (
          <div className="pt-2">
            <p className="flex items-start gap-2 text-xs text-muted-foreground">
              <ShieldAlert className="mt-0.5 h-3.5 w-3.5 shrink-0 text-amber-500" />
              <span>{cpu?.message ?? t('hardware.tempUnavailable')}</span>
            </p>
            <div className="mt-2 flex flex-wrap gap-2">
              {cpu?.needs_admin && (
                <Button size="sm" loading={driverBusy} onClick={() => void handleRestartElevated()}>
                  {t('hardware.restartElevated')}
                </Button>
              )}
              {!cpu?.driver_installed && (
                <Button size="sm" variant="outline" onClick={() => setInstallOpen(true)}>
                  {t('hardware.installDriver')}
                </Button>
              )}
            </div>
          </div>
        )}
      </Card>

      {/* GPU 列表 */}
      <Card className="gap-3">
        <div className="flex items-center gap-3">
          <span className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl border border-primary/15 bg-primary/10 text-primary">
            <Gauge className="h-5 w-5" />
          </span>
          <div className="min-w-0 flex-1">
            <div className="text-sm font-medium">{t('hardware.gpuTemp')}</div>
          </div>
        </div>
        {gpus.length === 0 ? (
          <p className="text-xs text-muted-foreground">{t('hardware.gpuNotFound')}</p>
        ) : (
          <div className="divide-y divide-border/60">
            {gpus.map((g) => (
              <div key={g.name} className="flex items-center justify-between gap-3 py-2 text-sm">
                <span className="min-w-0 truncate">{g.name}</span>
                <span className="shrink-0 tabular-nums text-muted-foreground">
                  {g.temp_celsius != null ? `${g.temp_celsius.toFixed(1)}°C` : '--'}
                </span>
              </div>
            ))}
          </div>
        )}
      </Card>

      {/* 磁盘温度 */}
      <Card padding="none" className="overflow-hidden">
        <div className="flex items-center gap-2 border-b border-border/60 px-4 py-3 text-sm font-semibold">
          <HardDrive className="h-4 w-4 text-primary" />
          {t('hardware.diskTemp')}
        </div>
        {diskTemps.length === 0 ? (
          <p className="px-4 py-6 text-center text-sm text-muted-foreground">{t('hardware.loading')}</p>
        ) : (
          <div className="divide-y divide-border/60">
            {diskTemps.map((d) => (
              <div key={d.drive} className="flex items-center justify-between gap-3 px-4 py-3">
                <div className="min-w-0 flex-1">
                  <div className="text-sm font-medium">{d.model || d.drive}</div>
                  <div className="text-xs text-muted-foreground">{d.drive}</div>
                </div>
                <span className="shrink-0 tabular-nums text-muted-foreground">
                  {d.temp_celsius != null ? `${d.temp_celsius.toFixed(1)}°C` : '--'}
                </span>
              </div>
            ))}
          </div>
        )}
        <p className="border-t border-border/60 px-4 py-2.5 text-xs text-muted-foreground">
          {t('hardware.diskTempHint')}
        </p>
      </Card>

      {/* 磁盘健康 */}
      <Card padding="none" className="overflow-hidden">
        <div className="flex items-center justify-between gap-2 border-b border-border/60 px-4 py-3">
          <div className="flex items-center gap-2 text-sm font-semibold">
            <ShieldAlert className="h-4 w-4 text-primary" />
            {t('hardware.diskHealth')}
          </div>
          <Button
            size="sm"
            variant="outline"
            loading={diskHealthBusy}
            onClick={() => void loadDiskHealth(true)}
          >
            {t('hardware.diskHealthRefresh')}
          </Button>
        </div>
        {diskHealth.length === 0 ? (
          <p className="px-4 py-6 text-center text-sm text-muted-foreground">{t('hardware.loading')}</p>
        ) : (
          <div className="divide-y divide-border/60">
            {diskHealth.map((d) => {
              const statusClass =
                d.status === 'Healthy'
                  ? 'bg-emerald-500/15 text-emerald-500'
                  : d.status === 'Warning'
                    ? 'bg-amber-500/15 text-amber-500'
                    : d.status === 'Unhealthy'
                      ? 'bg-red-500/15 text-red-500'
                      : 'bg-muted text-muted-foreground';
              return (
                <div key={d.name} className="space-y-1 px-4 py-3">
                  <div className="flex items-center justify-between gap-3">
                    <div className="min-w-0 flex-1 truncate text-sm font-medium">{d.name}</div>
                    <span
                      className={`shrink-0 rounded-full px-2 py-0.5 text-[11px] font-medium ${statusClass}`}
                    >
                      {t(`hardware.health.${d.status}`)}
                    </span>
                  </div>
                  <div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
                    <span>
                      {t('hardware.diskTemp')}:{' '}
                      <span className="tabular-nums">
                        {d.temp_celsius != null ? `${d.temp_celsius.toFixed(1)}°C` : '--'}
                      </span>
                    </span>
                    {d.media_type === 'SSD' && (
                      <span>
                        {t('hardware.wear')}:{' '}
                        <span className="tabular-nums">
                          {d.wear_percent != null ? `${d.wear_percent.toFixed(1)}%` : '--'}
                        </span>
                      </span>
                    )}
                    <span>
                      {t('hardware.powerOnHours')}:{' '}
                      <span className="tabular-nums">{d.power_on_hours ?? '--'}</span>
                    </span>
                    <span>
                      {t('hardware.readErrors')}:{' '}
                      <span className="tabular-nums">{d.read_errors ?? '--'}</span>
                    </span>
                    <span>
                      {t('hardware.writeErrors')}:{' '}
                      <span className="tabular-nums">{d.write_errors ?? '--'}</span>
                    </span>
                  </div>
                </div>
              );
            })}
          </div>
        )}
        <p className="border-t border-border/60 px-4 py-2.5 text-xs text-muted-foreground">
          {t('hardware.diskHealthHint')}
        </p>
      </Card>

      {/* 磁盘空间 */}
      <Card padding="none" className="overflow-hidden">
        <div className="flex items-center gap-2 border-b border-border/60 px-4 py-3 text-sm font-semibold">
          <HardDrive className="h-4 w-4 text-primary" />
          {t('hardware.disks')}
        </div>
        {!snapshot || snapshot.disks.length === 0 ? (
          <p className="px-4 py-8 text-center text-sm text-muted-foreground">{t('hardware.loading')}</p>
        ) : (
          <div className="divide-y divide-border/60">
            {snapshot.disks.map((disk) => {
              const used = disk.total_bytes - disk.available_bytes;
              const percent = (used / Math.max(1, disk.total_bytes)) * 100;
              return (
                <div key={disk.drive} className="flex items-center gap-4 px-4 py-3">
                  <div className="min-w-0 flex-1">
                    <div className="flex items-baseline justify-between">
                      <span className="text-sm font-medium">{disk.drive}</span>
                      <span className="text-xs tabular-nums text-muted-foreground">
                        {formatBytes(used)} / {formatBytes(disk.total_bytes)}
                      </span>
                    </div>
                    <div className="mt-2">
                      <span className="h-2 w-full overflow-hidden rounded-full bg-muted">
                        <span
                          className="block h-full rounded-full transition-all duration-500"
                          style={{
                            width: `${Math.min(100, Math.max(0, percent))}%`,
                            backgroundColor: percent > 90 ? '#ef4444' : 'var(--chart-primary)',
                          }}
                        />
                      </span>
                    </div>
                    <div className="mt-1 text-xs tabular-nums text-muted-foreground">
                      {t('hardware.available')} {formatBytes(disk.available_bytes)}
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </Card>

      {/* 安装内核驱动确认 */}
      <Dialog open={installOpen} onOpenChange={setInstallOpen}>
        <DialogContent hideClose>
          <DialogHeader>
            <DialogTitle>{t('hardware.driverConfirmTitle')}</DialogTitle>
            <DialogDescription>{t('hardware.driverConfirmDescription')}</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setInstallOpen(false)}>
              {t('hardware.cancel')}
            </Button>
            <Button loading={driverBusy} onClick={() => void handleInstallDriver()}>
              {t('hardware.installDriver')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
