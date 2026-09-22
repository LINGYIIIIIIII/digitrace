'use client';

import { useCallback, useEffect, useState } from 'react';
import type { ComponentType, ReactNode } from 'react';
import {
  Database,
  Download,
  FileDown,
  FileSpreadsheet,
  FileText,
  FolderOpen,
  Gamepad2,
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
import type { CpuTemperatureDto, ExportResultDto, GameEntryDto, GameLibraryResultDto } from '../types';
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

import {
  CATEGORIES,
  type SettingsCategory,
  fmtGameHours,
  SettingRow,
  THEME_MODE_OPTIONS,
  WINDOW_BLUR_OPTIONS,
  FONT_OPTIONS,
  TITLEBAR_ITEMS,
  TRAY_ITEMS,
  LANGUAGE_OPTIONS,
  POLL_INTERVAL_OPTIONS,
  IDLE_THRESHOLD_OPTIONS,
  REFRESH_INTERVAL_OPTIONS,
  LIVE_REFRESH_INTERVAL_OPTIONS,
  NETWORK_LIVE_WINDOW_OPTIONS,
} from './settings-shared';
import type { SettingsSectionProps } from './settings/types';
import { GeneralSection } from './settings/general-section';
import { AppearanceSection } from './settings/appearance-section';
import { StartupSection } from './settings/startup-section';
import { HardwareSection } from './settings/hardware-section';
import { UpdateSection } from './settings/update-section';
import { GamesSection } from './settings/games-section';
import { LogsSection } from './settings/logs-section';

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

  // 顶栏「比例」等入口应用缩放后派发 ui-zoom-change：实时同步，避免显示旧值。
  useEffect(() => {
    const onZoomChange = (e: Event) => {
      const detail = (e as CustomEvent<number>).detail;
      if (typeof detail === 'number' && Number.isFinite(detail)) {
        setUiZoom(detail);
      }
    };
    window.addEventListener('ui-zoom-change', onZoomChange);
    return () => window.removeEventListener('ui-zoom-change', onZoomChange);
  }, []);

const [clearOpen, setClearOpen] = useState(false);
const [clearing, setClearing] = useState(false);
const [exporting, setExporting] = useState(false);
const [exportResult, setExportResult] = useState<ExportResultDto | null>(null);
const [csvExporting, setCsvExporting] = useState(false);
const [csvExportResult, setCsvExportResult] = useState<ExportResultDto | null>(null);
const [games, setGames] = useState<GameEntryDto[] | null>(null);
const [gamesLoading, setGamesLoading] = useState(false);
const [manualTitle, setManualTitle] = useState('');
const [manualExe, setManualExe] = useState('');
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

  const handleExportUsageCsv = useCallback(async () => {
    setCsvExporting(true);
    setCsvExportResult(null);
    try {
      const res = await apiService.exportUsageCsv();
      setCsvExportResult(res);
      if (res.ok) {
        toast.success(t('settings.data.csvExported'));
      } else {
        toast.error(res.message ?? t('settings.data.exportFailed'));
      }
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    } finally {
      setCsvExporting(false);
    }
  }, [t]);

  const loadGames = useCallback(async () => {
    setGamesLoading(true);
    try {
      const list = await apiService.getGamesLibrary();
      setGames(list);
    } catch {
      toast.error(t('settings.games.loadFailed'));
    } finally {
      setGamesLoading(false);
    }
  }, [t]);

  const handleRefreshGames = useCallback(async () => {
    setGamesLoading(true);
    try {
      const res = await apiService.refreshGameLibrary();
      if (res.ok) {
        toast.success(res.message ?? t('settings.games.refreshed'));
        const list = await apiService.getGamesLibrary();
        setGames(list);
      } else {
        toast.error(res.message ?? t('settings.games.refreshFailed'));
      }
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    } finally {
      setGamesLoading(false);
    }
  }, [t]);

  const handleAddGameManual = useCallback(async () => {
    const title = manualTitle.trim();
    const exe = manualExe.trim();
    if (!title || !exe) {
      toast.error(t('settings.games.manualEmpty'));
      return;
    }
    const res = await apiService.addGameManual(title, exe);
    if (res.ok) {
      toast.success(t('settings.games.added'));
      setManualTitle('');
      setManualExe('');
      void loadGames();
    } else {
      toast.error(res.message ?? t('settings.games.addFailed'));
    }
  }, [manualTitle, manualExe, loadGames, t]);

  const handleRemoveGame = useCallback(
    async (id: number) => {
      const res = await apiService.removeGame(id);
      if (res.ok) {
        toast.success(t('settings.games.removed'));
        setGames((prev) => (prev ? prev.filter((g) => g.id !== id) : prev));
      } else {
        toast.error(res.message ?? t('settings.games.removeFailed'));
      }
    },
    [t],
  );

  const sectionProps: SettingsSectionProps = {
    t, config, patchConfig, isWindows, locale, setLocale: setLocale as (l: string) => void,
    autoStart, elevatedAutoStart,
    handleAutoStartToggle: handleAutoStartToggle as (v: boolean) => void,
    handleElevatedAutoStartToggle: handleElevatedAutoStartToggle as (v: boolean) => void,
    uiZoom,
    setUiZoom: (v: number) => { setUiZoom(v); applyUiZoom(v); },
    themeModeOptions, windowBlurOptions, fontOptions, titlebarItems,
    handleThemeModeChange, handleFontChange, handleWindowBlurChange,
    handleTitlebarToggle, handleTrayToggle, setRestartOpen,
    driverStatus, driverBusy, setDriverOpen, setDriverUninstallOpen,
    games, gamesLoading, manualTitle, manualExe, setManualTitle, setManualExe,
    handleAddGameManual, handleRemoveGame, handleRefreshGames, loadGames,
    logPath, clearOpen, setClearOpen, clearing, handleClearData,
    handleExportPlaintext, handleExportUsageCsv, exporting, csvExporting,
    exportResult, csvExportResult, setExportResult, setCsvExportResult,
  };
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
