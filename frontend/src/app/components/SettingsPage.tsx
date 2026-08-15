'use client';

import { useCallback, useEffect, useState } from 'react';
import type { ComponentType, ReactNode } from 'react';
import {
  Database,
  Download,
  FileDown,
  FileText,
  FolderOpen,
  Globe,
  CodeXml,
  Clock,
  GitBranch,
  Languages,
  Monitor,
  Palette,
  Play,
  RefreshCw,
  Rocket,
  ShieldCheck,
  ShieldAlert,
  SlidersHorizontal,
  Sparkles,
  Thermometer,
  Timer,
  Trash2,
  ZoomIn,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { UpdateCheckCard } from './UpdateCheckCard';
import { useShallow } from 'zustand/react/shallow';
import { useLocale } from '../lib/i18n';
import { apiService } from '../services/api';
import { useAppStore } from '../store/app-store';
import type { CpuTemperatureDto, ExportResultDto } from '../types';
import { applyFontFamily, applyThemeMode, applyUiZoom, loadUiZoom } from '../lib/appearance';
import { Slider } from '@/components/ui/slider';
import { Input } from '@/components/ui/input';
import { Button, Card, Select, ToggleSwitch } from './ui/index';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';

type SettingsCategory = 'general' | 'appearance' | 'startup' | 'hardware' | 'update' | 'logs';

const CATEGORIES: {
  id: SettingsCategory;
  icon: ComponentType<{ className?: string }>;
  labelKey: string;
}[] = [
  { id: 'general', icon: SlidersHorizontal, labelKey: 'settings.tabs.general' },
  { id: 'appearance', icon: Palette, labelKey: 'settings.tabs.appearance' },
  { id: 'startup', icon: Rocket, labelKey: 'settings.tabs.startup' },
  { id: 'hardware', icon: Thermometer, labelKey: 'settings.tabs.hardware' },
  { id: 'update', icon: RefreshCw, labelKey: 'settings.tabs.update' },
  { id: 'logs', icon: Database, labelKey: 'settings.tabs.logs' },
];

const THEME_MODE_OPTIONS = [
  { value: 'system', labelKey: 'settings.theme.system' },
  { value: 'light', labelKey: 'settings.theme.light' },
  { value: 'dark', labelKey: 'settings.theme.dark' },
];

const WINDOW_BLUR_OPTIONS = [
  { value: 'auto', labelKey: 'settings.blur.auto' },
  { value: 'mica', labelKey: 'settings.blur.mica' },
  { value: 'acrylic', labelKey: 'settings.blur.acrylic' },
  { value: 'tabbed', labelKey: 'settings.blur.tabbed' },
  { value: 'off', labelKey: 'settings.blur.off' },
];

const FONT_OPTIONS = [
  { value: 'system', labelKey: 'settings.font.system' },
  { value: 'harmonyos', labelKey: 'settings.font.harmonyos' },
  { value: 'noto', labelKey: 'settings.font.noto' },
  { value: 'misans', labelKey: 'settings.font.misans' },
];

const TITLEBAR_ITEMS = [
  { value: 'monitoring', labelKey: 'settings.titlebar.monitoring' },
  { value: 'tray', labelKey: 'settings.titlebar.tray' },
  { value: 'active', labelKey: 'settings.titlebar.active' },
  { value: 'idle', labelKey: 'settings.titlebar.idle' },
];

const TRAY_ITEMS = [
  { value: 'cpu', labelKey: 'settings.tray.cpu' },
  { value: 'memory', labelKey: 'settings.tray.memory' },
  { value: 'network', labelKey: 'settings.tray.network' },
  { value: 'active', labelKey: 'settings.tray.active' },
  { value: 'temp', labelKey: 'settings.tray.temp' },
];

// 语言选项固定显示各自母语名称，不随界面语言翻译（避免 THRM 的坑）。
const LANGUAGE_OPTIONS = [
  { value: 'zh-CN', label: '中文' },
  { value: 'en-US', label: 'English' },
  { value: 'ja-JP', label: '日本語' },
];

const POLL_INTERVAL_OPTIONS = [
  { value: '1000', label: '1 秒' },
  { value: '2000', label: '2 秒' },
  { value: '3000', label: '3 秒' },
  { value: '5000', label: '5 秒' },
];

const IDLE_THRESHOLD_OPTIONS = [
  { value: '1', label: '1 分钟' },
  { value: '3', label: '3 分钟' },
  { value: '5', label: '5 分钟' },
  { value: '10', label: '10 分钟' },
];

const REFRESH_INTERVAL_OPTIONS = [
  { value: '5', label: '5 秒' },
  { value: '10', label: '10 秒' },
  { value: '30', label: '30 秒' },
  { value: '60', label: '1 分钟' },
];

const LIVE_REFRESH_INTERVAL_OPTIONS = [
  { value: '1', label: '1 秒' },
  { value: '2', label: '2 秒' },
  { value: '5', label: '5 秒' },
  { value: '10', label: '10 秒' },
];

const NETWORK_LIVE_WINDOW_OPTIONS = [
  { value: '60', label: '1 分钟' },
  { value: '120', label: '2 分钟' },
  { value: '300', label: '5 分钟' },
  { value: '600', label: '10 分钟' },
];

function SettingRow({
  icon,
  title,
  description,
  children,
}: {
  icon: ReactNode;
  title: string;
  description: string;
  children: ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-4 px-5 py-4">
      <div className="flex min-w-0 items-start gap-3">
        <span className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-primary/15 bg-primary/10 text-primary">
          {icon}
        </span>
        <div className="min-w-0">
          <div className="text-sm font-medium text-foreground">{title}</div>
          <p className="mt-0.5 text-xs leading-relaxed text-muted-foreground">{description}</p>
        </div>
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

export default function SettingsPage() {
  const { t } = useTranslation();
  const { locale, setLocale } = useLocale();
  const { config, updateConfig } = useAppStore(
    useShallow((state) => ({ config: state.config, updateConfig: state.updateConfig })),
  );
  const isWindows = /Windows/i.test(navigator.userAgent);

  const [autoStart, setAutoStart] = useState(false);
  const [elevatedAutoStart, setElevatedAutoStart] = useState(false);
  const [logPath, setLogPath] = useState('');
  const [uiZoom, setUiZoom] = useState(() => (typeof window !== 'undefined' ? loadUiZoom() : 100));
const [clearOpen, setClearOpen] = useState(false);
const [clearing, setClearing] = useState(false);
const [exporting, setExporting] = useState(false);
const [exportResult, setExportResult] = useState<ExportResultDto | null>(null);
const [restartOpen, setRestartOpen] = useState(false);
const [driverOpen, setDriverOpen] = useState(false);
const [driverUninstallOpen, setDriverUninstallOpen] = useState(false);
const [driverBusy, setDriverBusy] = useState(false);
const [driverStatus, setDriverStatus] = useState<CpuTemperatureDto | null>(null);
  const [category, setCategory] = useState<SettingsCategory>('general');

  const themeModeOptions = THEME_MODE_OPTIONS.map((item) => ({
    value: item.value,
    label: t(item.labelKey),
  }));
  const windowBlurOptions = WINDOW_BLUR_OPTIONS.map((item) => ({
    value: item.value,
    label: t(item.labelKey),
  }));
  const fontOptions = FONT_OPTIONS.map((item) => ({
    value: item.value,
    label: t(item.labelKey),
  }));

  const titlebarItems = config?.titlebar_items?.length
    ? config.titlebar_items
    : ['monitoring', 'tray'];

  useEffect(() => {
    void apiService.isAutoStart().then(setAutoStart).catch(() => setAutoStart(false));
    void apiService
      .isElevatedAutoStart()
      .then(setElevatedAutoStart)
      .catch(() => setElevatedAutoStart(false));
    void apiService
      .getLogPath()
      .then(setLogPath)
      .catch(() => setLogPath(''));
    const refreshDriver = () => {
      void apiService
        .getTemperatureSnapshot()
        .then((s) => setDriverStatus(s.cpu))
        .catch(() => undefined);
    };
    refreshDriver();
    const timer = window.setInterval(refreshDriver, 5000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    if (!config) return;
    applyThemeMode(config.theme_mode || 'system');
    applyFontFamily(config.font_family || 'system');
  }, [config]);

  useEffect(() => {
    const applySystemTheme = () => applyThemeMode(config?.theme_mode || 'system');
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    mq.addEventListener('change', applySystemTheme);
    return () => mq.removeEventListener('change', applySystemTheme);
  }, [config]);

  const patchConfig = useCallback(
    async (patch: Partial<NonNullable<typeof config>>) => {
      if (!config) return;
      await updateConfig({ ...config, ...patch });
    },
    [config, updateConfig],
  );

  const handleThemeModeChange = useCallback(
    async (mode: string) => {
      applyThemeMode(mode);
      await patchConfig({ theme_mode: mode });
    },
    [patchConfig],
  );

  const handleFontChange = useCallback(
    async (family: string) => {
      applyFontFamily(family);
      await patchConfig({ font_family: family });
    },
    [patchConfig],
  );

  const handleWindowBlurChange = useCallback(
    async (mode: string) => {
      await patchConfig({ window_blur: mode });
      setRestartOpen(true);
    },
    [patchConfig],
  );

  const handleRestartNow = useCallback(() => {
    apiService
      .restartApp()
      .catch((error) => {
        toast.error(`重启失败: ${String(error)}`, { duration: 30000 });
      });
  }, []);

  const handleTitlebarToggle = useCallback(
    async (value: string, enabled: boolean) => {
      // ?? 只在「没有值」时兜底；空数组表示全部关闭，必须保留。
      const current = config?.titlebar_items ?? ['monitoring', 'tray'];
      const next = enabled
        ? Array.from(new Set([...current, value]))
        : current.filter((v) => v !== value);
      await patchConfig({ titlebar_items: next });
    },
    [config, patchConfig],
  );

  const handleTrayToggle = useCallback(
    async (value: string, enabled: boolean) => {
      const current = config?.tray_items ?? ['cpu', 'memory', 'network', 'active'];
      const next = enabled
        ? Array.from(new Set([...current, value]))
        : current.filter((v) => v !== value);
      await patchConfig({ tray_items: next });
    },
    [config, patchConfig],
  );

  const handleInstallDriver = async () => {
    setDriverBusy(true);
    try {
      const res = await apiService.installPawnioDriver();
      if (res.ok) toast.success(res.message);
      else toast.error(res.message);
      setDriverOpen(false);
      window.setTimeout(() => {
        void apiService
          .getTemperatureSnapshot()
          .then((s) => setDriverStatus(s.cpu))
          .catch(() => undefined);
      }, 3000);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    } finally {
      setDriverBusy(false);
    }
  };

  const handleUninstallDriver = async () => {
    setDriverBusy(true);
    try {
      const res = await apiService.uninstallPawnioDriver();
      if (res.ok) toast.success(res.message);
      else toast.error(res.message);
      setDriverUninstallOpen(false);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    } finally {
      setDriverBusy(false);
    }
  };

  const handleAutoStartToggle = useCallback(
    async (enabled: boolean) => {
      setAutoStart(enabled);
      try {
        await apiService.setAutoStart(enabled);
        toast.success(enabled ? t('settings.startup.enabled') : t('settings.startup.disabled'));
      } catch {
        setAutoStart(!enabled);
        toast.error(t('settings.startup.failed'));
      }
    },
    [t],
  );

  const handleElevatedAutoStartToggle = useCallback(
    async (enabled: boolean) => {
      setElevatedAutoStart(enabled);
      try {
        await apiService.setElevatedAutoStart(enabled);
        toast.success(
          enabled ? t('settings.startup.elevatedEnabled') : t('settings.startup.elevatedDisabled'),
        );
        // 管理员自启开启后，普通自启开关同步为开；关闭后刷新真实状态。
        void apiService.isAutoStart().then(setAutoStart).catch(() => setAutoStart(false));
      } catch {
        setElevatedAutoStart(!enabled);
        toast.error(t('settings.startup.elevatedFailed'));
      }
    },
    [t],
  );

  const handleClearData = useCallback(async () => {
    setClearing(true);
    try {
      await apiService.clearData();
      toast.success(t('settings.data.cleared'));
      setClearOpen(false);
    } catch {
      toast.error(t('settings.data.clearFailed'));
    } finally {
      setClearing(false);
    }
  }, [t]);

  const handleExportPlaintext = useCallback(async () => {
    setExporting(true);
    setExportResult(null);
    try {
      const res = await apiService.exportPlaintext();
      setExportResult(res);
      if (res.ok) {
        toast.success(t('settings.data.exported'));
      } else {
        toast.error(res.message ?? t('settings.data.exportFailed'));
      }
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    } finally {
      setExporting(false);
    }
  }, [t]);

  return (
    <div className="mx-auto max-w-3xl space-y-4">
      <div className="flex flex-col gap-4 md:flex-row">
        {/* 左侧分类导航（与主界面左侧导航风格一致） */}
        <nav className="flex shrink-0 flex-row gap-1 overflow-x-auto md:w-44 md:flex-col" aria-label={t('settings.tabs.general')}>
          {CATEGORIES.map((cat) => {
            const Icon = cat.icon;
            const active = category === cat.id;
            return (
              <button
                key={cat.id}
                type="button"
                onClick={() => setCategory(cat.id)}
                className={
                  'flex items-center gap-2.5 rounded-lg px-3 py-2 text-sm font-medium transition-colors ' +
                  (active
                    ? 'border border-primary/15 bg-primary/10 text-primary'
                    : 'border border-transparent text-muted-foreground hover:bg-accent hover:text-foreground')
                }
              >
                <Icon className="h-4 w-4 shrink-0" />
                <span className="truncate">{t(cat.labelKey)}</span>
              </button>
            );
          })}
        </nav>

        <div className="min-w-0 flex-1 space-y-3">
          {category === 'general' && (
            <>
              <Card padding="none" className="overflow-hidden">
                <SettingRow
                  icon={<Play className="h-4 w-4" />}
                  title={t('settings.monitor.autoTrack')}
                  description={t('settings.monitor.autoTrackDescription')}
                >
                  <ToggleSwitch
                    enabled={config?.auto_start_tracking ?? true}
                    onChange={(v) => patchConfig({ auto_start_tracking: v })}
                  />
                </SettingRow>
              </Card>
              <Card padding="none" className="overflow-hidden">
                <SettingRow
                  icon={<RefreshCw className="h-4 w-4" />}
                  title={t('settings.monitor.pollInterval')}
                  description={t('settings.monitor.pollIntervalDescription')}
                >
                  <div className="w-32">
                    <Select
                      value={String(config?.poll_interval_ms ?? 3000)}
                      onChange={(v) => patchConfig({ poll_interval_ms: Number(v) })}
                      options={POLL_INTERVAL_OPTIONS}
                      size="sm"
                    />
                  </div>
                </SettingRow>
                <SettingRow
                  icon={<Monitor className="h-4 w-4" />}
                  title={t('settings.monitor.idleThreshold')}
                  description={t('settings.monitor.idleThresholdDescription')}
                >
                  <div className="w-32">
                    <Select
                      value={String(config?.idle_threshold_minutes ?? 5)}
                      onChange={(v) => patchConfig({ idle_threshold_minutes: Number(v) })}
                      options={IDLE_THRESHOLD_OPTIONS}
                      size="sm"
                    />
                  </div>
                </SettingRow>
                <SettingRow
                  icon={<RefreshCw className="h-4 w-4" />}
                  title={t('settings.refresh.interval')}
                  description={t('settings.refresh.intervalDescription')}
                >
                  <div className="w-32">
                    <Select
                      value={String(config?.refresh_interval_seconds ?? 10)}
                      onChange={(v) => patchConfig({ refresh_interval_seconds: Number(v) })}
                      options={REFRESH_INTERVAL_OPTIONS}
                      size="sm"
                    />
                  </div>
                </SettingRow>
                <SettingRow
                  icon={<RefreshCw className="h-4 w-4" />}
                  title={t('settings.refresh.liveInterval')}
                  description={t('settings.refresh.liveIntervalDescription')}
                >
                  <div className="w-32">
                    <Select
                      value={String(config?.live_refresh_interval_seconds ?? 1)}
                      onChange={(v) => patchConfig({ live_refresh_interval_seconds: Number(v) })}
                      options={LIVE_REFRESH_INTERVAL_OPTIONS}
                      size="sm"
                    />
                  </div>
                </SettingRow>
                <SettingRow
                  icon={<RefreshCw className="h-4 w-4" />}
                  title={t('settings.refresh.liveWindow')}
                  description={t('settings.refresh.liveWindowDescription')}
                >
                  <div className="w-32">
                    <Select
                      value={String(config?.network_live_window_seconds ?? 300)}
                      onChange={(v) => patchConfig({ network_live_window_seconds: Number(v) })}
                      options={NETWORK_LIVE_WINDOW_OPTIONS}
                      size="sm"
                    />
                  </div>
                </SettingRow>
                <SettingRow
                  icon={<Globe className="h-4 w-4" />}
                  title={t('settings.timezone.title')}
                  description={t('settings.timezone.description')}
                >
                  <div className="w-40">
                    <Select
                      value={config?.timezone ?? 'system'}
                      onChange={(v) => patchConfig({ timezone: String(v) })}
                      options={[
                        { value: 'system', label: t('settings.timezone.system') },
                        { value: 'utc+8', label: t('settings.timezone.utc8') },
                      ]}
                      size="sm"
                    />
                  </div>
                </SettingRow>
              </Card>
            </>
          )}

          {category === 'appearance' && (
            <>
          <Card padding="none" className="overflow-hidden">
            <SettingRow
              icon={<Monitor className="h-4 w-4" />}
              title={t('settings.theme.title')}
              description={t('settings.theme.description')}
            >
              <div className="w-36">
                <Select
                  value={(config?.theme_mode || 'system') as string}
                  onChange={(v) => handleThemeModeChange(String(v))}
                  options={themeModeOptions}
                  size="sm"
                />
              </div>
            </SettingRow>
            {isWindows && (
              <SettingRow
                icon={<Sparkles className="h-4 w-4" />}
                title={t('settings.blur.title')}
                description={t('settings.blur.description')}
              >
                <div className="w-36">
                  <Select
                    value={(config?.window_blur || 'auto') as string}
                    onChange={(v) => handleWindowBlurChange(String(v))}
                    options={windowBlurOptions}
                    size="sm"
                  />
                </div>
              </SettingRow>
            )}
            <SettingRow
              icon={<Monitor className="h-4 w-4" />}
              title={t('settings.font.title')}
              description={t('settings.font.description')}
            >
              <div className="w-36">
                <Select
                  value={(config?.font_family || 'system') as string}
                  onChange={(v) => handleFontChange(String(v))}
                  options={fontOptions}
                  size="sm"
                />
              </div>
            </SettingRow>
            <SettingRow
              icon={<ZoomIn className="h-4 w-4" />}
              title={t('settings.zoom.title')}
              description={t('settings.zoom.description')}
            >
              <div className="flex items-center gap-2">
                <Slider
                  value={[uiZoom]}
                  min={75}
                  max={175}
                  step={1}
                  onValueChange={(v) => {
                    setUiZoom(v[0]);
                    applyUiZoom(v[0]);
                  }}
                  className="w-28"
                />
                <Input
                  type="number"
                  min={75}
                  max={175}
                  value={String(uiZoom)}
                  onChange={(e) => {
                    const n = Number(e.target.value);
                    if (n >= 75 && n <= 175) {
                      setUiZoom(n);
                      applyUiZoom(n);
                    }
                  }}
                  className="h-8 w-14 text-center"
                />
                <span className="text-xs text-muted-foreground">%</span>
                <Select
                  value={String(uiZoom)}
                  onChange={(v) => {
                    const n = Number(v);
                    setUiZoom(n);
                    applyUiZoom(n);
                  }}
                  options={[75, 100, 125, 150].map((n) => ({ value: String(n), label: `${n}%` }))}
                  size="sm"
                  className="w-16"
                  triggerClassName="h-8"
                />
              </div>
            </SettingRow>
            <SettingRow
              icon={<Languages className="h-4 w-4" />}
              title={t('settings.language.title')}
              description={t('settings.language.description')}
            >
              <div className="w-36">
                <Select
                  value={locale}
                  onChange={(v) => setLocale(String(v) as 'zh-CN' | 'en-US' | 'ja-JP')}
                  options={LANGUAGE_OPTIONS}
                  size="sm"
                />
              </div>
            </SettingRow>
          </Card>

          {/* 顶栏显示 */}
          <Card padding="none" className="overflow-hidden">
            <div className="border-b border-border/60 px-5 py-3 text-sm font-semibold">
              {t('settings.titlebar.title')}
            </div>
            {TITLEBAR_ITEMS.map((item) => (
              <SettingRow
                key={item.value}
                icon={<Timer className="h-4 w-4" />}
                title={t(item.labelKey)}
                description=""
              >
                <ToggleSwitch
                  size="sm"
                  enabled={titlebarItems.includes(item.value)}
                  onChange={(v) => void handleTitlebarToggle(item.value, v)}
                />
              </SettingRow>
            ))}
          </Card>
            </>
          )}

          {category === 'startup' && (
            <>
          <Card padding="none" className="overflow-hidden">
            <SettingRow
              icon={<Rocket className="h-4 w-4" />}
              title={t('settings.startup.autoStart')}
              description={t('settings.startup.autoStartDescription')}
            >
              <ToggleSwitch enabled={autoStart} onChange={handleAutoStartToggle} />
            </SettingRow>
            <SettingRow
              icon={<ShieldCheck className="h-4 w-4" />}
              title={t('settings.startup.elevated')}
              description={t('settings.startup.elevatedDescription')}
            >
              <ToggleSwitch
                enabled={elevatedAutoStart}
                onChange={handleElevatedAutoStartToggle}
              />
            </SettingRow>
            <SettingRow
              icon={<Monitor className="h-4 w-4" />}
              title={t('settings.startup.minimizeToTray')}
              description={t('settings.startup.minimizeToTrayDescription')}
            >
              <ToggleSwitch
                enabled={config?.minimize_to_tray ?? true}
                onChange={(v) => patchConfig({ minimize_to_tray: v })}
              />
            </SettingRow>
            <SettingRow
              icon={<Monitor className="h-4 w-4" />}
              title={t('settings.startup.launchShowWindow')}
              description={t('settings.startup.launchShowWindowDescription')}
            >
              <ToggleSwitch
                enabled={config?.launch_show_window ?? true}
                onChange={(v) => void patchConfig({ launch_show_window: v })}
              />
            </SettingRow>
          </Card>

          {/* 托盘显示内容 */}
          <Card padding="none" className="overflow-hidden">
            <div className="border-b border-border/60 px-5 py-3 text-sm font-semibold">
              {t('settings.tray.title')}
            </div>
            {TRAY_ITEMS.map((item) => (
              <SettingRow
                key={item.value}
                icon={<Monitor className="h-4 w-4" />}
                title={t(item.labelKey)}
                description=""
              >
                <ToggleSwitch
                  size="sm"
                  enabled={(config?.tray_items ?? ['cpu', 'memory', 'network', 'active']).includes(
                    item.value,
                  )}
                  onChange={(v) => void handleTrayToggle(item.value, v)}
                />
              </SettingRow>
            ))}
            <p className="border-t border-border/60 px-5 py-3 text-xs text-muted-foreground">
              {t('settings.tray.hint')}
            </p>
          </Card>
            </>
          )}

          {category === 'hardware' && (
            <>
              <Card padding="none" className="overflow-hidden">
                <div className="border-b border-border/60 px-5 py-3 text-sm font-semibold">
                  {t('settings.hardware.driverTitle')}
                </div>
                <SettingRow
                  icon={<ShieldAlert className="h-4 w-4" />}
                  title={t('settings.hardware.driverStatus')}
                  description={
                    driverStatus
                      ? driverStatus.driver_installed
                        ? driverStatus.driver_running
                          ? t('settings.hardware.statusRunning', {
                              version: driverStatus.driver_version ?? '--',
                            })
                          : t('settings.hardware.statusStopped', {
                              version: driverStatus.driver_version ?? '--',
                            })
                        : t('settings.hardware.statusNotInstalled')
                      : t('hardware.loading')
                  }
                >
                  {driverStatus?.driver_installed ? (
                    <Button
                      variant="danger"
                      size="sm"
                      onClick={() => setDriverUninstallOpen(true)}
                    >
                      {t('settings.hardware.uninstall')}
                    </Button>
                  ) : (
                    <Button size="sm" onClick={() => setDriverOpen(true)}>
                      {t('settings.hardware.install')}
                    </Button>
                  )}
                </SettingRow>
                <p className="border-t border-border/60 px-5 py-3 text-xs leading-relaxed text-muted-foreground">
                  {t('settings.hardware.driverRisk')}
                </p>
              </Card>

              <Card padding="none" className="overflow-hidden">
                <div className="border-b border-border/60 px-5 py-3 text-sm font-semibold">
                  {t('settings.hardware.tempTitle')}
                </div>
                <p className="px-5 py-4 text-xs leading-relaxed text-muted-foreground">
                  {t('settings.hardware.tempDescription')}
                </p>
              </Card>
            </>
          )}

          {category === 'update' && (
            <>
              <Card padding="none" className="overflow-hidden">
                <SettingRow
                  icon={<RefreshCw className="h-4 w-4" />}
                  title={t('settings.update.autoCheck')}
                  description={t('settings.update.autoCheckDescription')}
                >
                  <ToggleSwitch
                    enabled={config?.update_check_enabled ?? true}
                    onChange={(v) => void patchConfig({ update_check_enabled: v })}
                  />
                </SettingRow>
                <div className="border-t border-border/60">
                  <SettingRow
                    icon={<ShieldCheck className="h-4 w-4" />}
                    title={t('settings.update.silentUpdate')}
                    description={t('settings.update.silentUpdateDescription')}
                  >
                    <ToggleSwitch
                      enabled={config?.update_silent ?? false}
                      onChange={(v) => void patchConfig({ update_silent: v })}
                    />
                  </SettingRow>
                </div>
                <div className="border-t border-border/60">
                  <SettingRow
                    icon={<Clock className="h-4 w-4" />}
                    title={t('settings.update.checkTime')}
                    description={t('settings.update.checkTimeDescription')}
                  >
                    <div className="w-36">
                      <Select
                        value={config?.update_check_hour != null ? String(config.update_check_hour) : ''}
                        onChange={(v) =>
                          void patchConfig({ update_check_hour: v === '' ? null : Number(v) })
                        }
                        options={[
                          { value: '', label: t('settings.update.checkTimeNone') },
                          ...Array.from({ length: 24 }, (_, h) => ({
                            value: String(h),
                            label: `${String(h).padStart(2, '0')}:00`,
                          })),
                        ]}
                        size="sm"
                      />
                    </div>
                  </SettingRow>
                </div>
                <div className="border-t border-border/60 px-5 py-4">
                  <div className="flex items-start gap-3">
                    <span className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-primary/15 bg-primary/10 text-primary">
                      <Download className="h-4 w-4" />
                    </span>
                    <div className="min-w-0 flex-1">
                      <div className="text-sm font-medium text-foreground">
                        {t('settings.update.manifestUrl')}
                      </div>
                      <p className="mt-0.5 text-xs leading-relaxed text-muted-foreground">
                        {t('settings.update.manifestUrlDescription')}
                      </p>
                      <Input
                        className="mt-2 h-8 font-mono text-xs"
                        placeholder="https://example.com/数迹/latest.json"
                        value={config?.update_manifest_url ?? ''}
                        onChange={(e) => void patchConfig({ update_manifest_url: e.target.value })}
                      />
                    </div>
                  </div>
                </div>
                <div className="border-t border-border/60 px-5 py-4">
                  <div className="flex items-start gap-3">
                    <span className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-primary/15 bg-primary/10 text-primary">
                      <GitBranch className="h-4 w-4" />
                    </span>
                    <div className="min-w-0 flex-1">
                      <div className="text-sm font-medium text-foreground">
                        {t('settings.update.githubRepo')}
                      </div>
                      <p className="mt-0.5 text-xs leading-relaxed text-muted-foreground">
                        {t('settings.update.githubRepoDescription')}
                      </p>
                      <Input
                        className="mt-2 h-8 font-mono text-xs"
                        placeholder="LINGYIIIIIIII/digitrace"
                        value={config?.update_github_repo ?? ''}
                        onChange={(e) => void patchConfig({ update_github_repo: e.target.value })}
                      />
                    </div>
                  </div>
                </div>
              </Card>
              <div className="rounded-2xl border border-border bg-card px-5 py-4">
                <div className="mb-2 flex items-center gap-2 text-primary">
                  <RefreshCw className="h-4 w-4" />
                  <h4 className="text-sm font-semibold">{t('about.update.title')}</h4>
                </div>
                <UpdateCheckCard />
              </div>
              <p className="px-1 text-xs leading-relaxed text-muted-foreground">
                {t('settings.update.hint')}
              </p>
              <p className="px-1 text-xs font-medium">
                {!config?.update_manifest_url?.trim() && !config?.update_github_repo?.trim()
                  ? t('settings.update.statusNoUrl')
                  : config?.update_check_enabled
                    ? t('settings.update.statusAutoOn')
                    : t('settings.update.statusAutoOff')}
              </p>
            </>
          )}

          {category === 'logs' && (
            <>
          <Card padding="none" className="overflow-hidden">
            <div className="px-5 py-4">
              <div className="flex items-start gap-3">
                <span className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-primary/15 bg-primary/10 text-primary">
                  <FileText className="h-4 w-4" />
                </span>
                <div className="min-w-0">
                  <div className="text-sm font-medium text-foreground">{t('settings.logs.path')}</div>
                  <p className="mt-1 break-all rounded-lg border border-border/60 bg-background/60 px-2.5 py-1.5 font-mono text-xs text-muted-foreground">
                    {logPath || '—'}
                  </p>
                </div>
              </div>
            </div>
            <div className="border-t border-border/60 px-5 py-4">
              <div className="flex items-start gap-3">
                <span className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-primary/15 bg-primary/10 text-primary">
                  <Monitor className="h-4 w-4" />
                </span>
                <div className="min-w-0">
                  <div className="text-sm font-medium text-foreground">{t('settings.logs.dataLocation')}</div>
                  <p className="mt-0.5 text-xs leading-relaxed text-muted-foreground">
                    {t('settings.logs.dataLocationDescription')}
                  </p>
                  <p className="mt-1 break-all rounded-lg border border-border/60 bg-background/60 px-2.5 py-1.5 font-mono text-xs text-muted-foreground">
                    {config?.db_path ? config.db_path.replace(/\\[^\\/]+$/, '') : '—'}
                  </p>
                </div>
              </div>
            </div>
          </Card>
          <Card padding="none" className="overflow-hidden">
            <div className="px-5 py-4">
              <div className="flex items-start gap-3">
                <span className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-primary/15 bg-primary/10 text-primary">
                  <FileDown className="h-4 w-4" />
                </span>
                <div className="min-w-0 flex-1">
                  <div className="text-sm font-medium text-foreground">
                    {t('settings.data.exportTitle')}
                  </div>
                  <p className="mt-0.5 text-xs leading-relaxed text-muted-foreground">
                    {t('settings.data.exportDescription')}
                  </p>
                  {exportResult?.ok && exportResult.path && (
                    <div className="mt-2 flex flex-wrap items-center gap-2">
                      <span className="break-all rounded-lg border border-border/60 bg-background/60 px-2.5 py-1.5 font-mono text-[11px] text-muted-foreground">
                        {exportResult.path}
                      </span>
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => void apiService.revealInExplorer(exportResult.path!)}
                      >
                        <FolderOpen className="mr-1 h-3.5 w-3.5" />
                        {t('settings.data.openFolder')}
                      </Button>
                    </div>
                  )}
                  {exportResult && !exportResult.ok && (
                    <p className="mt-2 text-xs text-destructive">
                      {exportResult.message ?? t('settings.data.exportFailed')}
                    </p>
                  )}
                </div>
              </div>
            </div>
            <div className="flex justify-end border-t border-border/60 px-5 py-3">
              <Button variant="outline" size="sm" loading={exporting} onClick={() => void handleExportPlaintext()}>
                {t('settings.data.export')}
              </Button>
            </div>
          </Card>
          <Card padding="none" className="overflow-hidden">
            <SettingRow
              icon={<Trash2 className="h-4 w-4" />}
              title={t('settings.data.clearTitle')}
              description={t('settings.data.clearDescription')}
            >
              <Button variant="danger" size="sm" onClick={() => setClearOpen(true)}>
                {t('settings.data.clear')}
              </Button>
            </SettingRow>
          </Card>
            </>
          )}
        </div>
      </div>

      <Dialog open={clearOpen} onOpenChange={setClearOpen}>
        <DialogContent hideClose>
          <DialogHeader>
            <DialogTitle>{t('settings.data.confirmTitle')}</DialogTitle>
            <DialogDescription>{t('settings.data.confirmDescription')}</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setClearOpen(false)}>
              {t('settings.data.cancel')}
            </Button>
            <Button variant="danger" loading={clearing} onClick={handleClearData}>
              {t('settings.data.confirmClear')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={restartOpen} onOpenChange={setRestartOpen}>
        <DialogContent hideClose>
          <DialogHeader>
            <DialogTitle>{t('settings.blur.restartTitle')}</DialogTitle>
            <DialogDescription>{t('settings.blur.restartDescription')}</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setRestartOpen(false)}>
              {t('settings.blur.restartLater')}
            </Button>
            <Button onClick={handleRestartNow}>{t('settings.blur.restartNow')}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={driverOpen} onOpenChange={setDriverOpen}>
        <DialogContent hideClose>
          <DialogHeader>
            <DialogTitle>{t('settings.hardware.installTitle')}</DialogTitle>
            <DialogDescription>{t('settings.hardware.installDescription')}</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDriverOpen(false)}>
              {t('settings.hardware.cancel')}
            </Button>
            <Button loading={driverBusy} onClick={() => void handleInstallDriver()}>
              {t('settings.hardware.install')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={driverUninstallOpen} onOpenChange={setDriverUninstallOpen}>
        <DialogContent hideClose>
          <DialogHeader>
            <DialogTitle>{t('settings.hardware.uninstallTitle')}</DialogTitle>
            <DialogDescription>{t('settings.hardware.uninstallDescription')}</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDriverUninstallOpen(false)}>
              {t('settings.hardware.cancel')}
            </Button>
            <Button variant="danger" loading={driverBusy} onClick={() => void handleUninstallDriver()}>
              {t('settings.hardware.uninstall')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
