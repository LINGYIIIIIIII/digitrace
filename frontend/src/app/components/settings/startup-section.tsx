'use client';

/** 设置分区：Startup */
import {
  Button,
  Card,
  Select,
  ToggleSwitch,
} from '../ui/index';
import { Input } from '@/components/ui/input';
import { Slider } from '@/components/ui/slider';
import {
  SettingRow,
  fmtGameHours,
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
} from '../settings-shared';
import {
  Clock,
  CodeXml,
  Database,
  Download,
  FileDown,
  FileSpreadsheet,
  FileText,
  FolderOpen,
  Gamepad2,
  GitBranch,
  Globe,
  Languages,
  Monitor,
  Palette,
  Play,
  RefreshCw,
  Rocket,
  ShieldAlert,
  ShieldCheck,
  Sparkles,
  Thermometer,
  Timer,
  Trash2,
  ZoomIn,
} from 'lucide-react';
import { UpdateCheckCard } from '../UpdateCheckCard';
import type { SettingsSectionProps } from './types';

void THEME_MODE_OPTIONS; void WINDOW_BLUR_OPTIONS; void FONT_OPTIONS; void TITLEBAR_ITEMS;
void TRAY_ITEMS; void LANGUAGE_OPTIONS; void POLL_INTERVAL_OPTIONS; void IDLE_THRESHOLD_OPTIONS;
void REFRESH_INTERVAL_OPTIONS; void LIVE_REFRESH_INTERVAL_OPTIONS; void NETWORK_LIVE_WINDOW_OPTIONS;
void fmtGameHours; void UpdateCheckCard;
void Clock; void CodeXml; void Database; void Download; void FileDown; void FileSpreadsheet;
void FileText; void FolderOpen; void Gamepad2; void GitBranch; void Globe; void Languages;
void Monitor; void Palette; void Play; void RefreshCw; void Rocket; void ShieldAlert;
void ShieldCheck; void Sparkles; void Thermometer; void Timer; void Trash2; void ZoomIn;

export function StartupSection(props: SettingsSectionProps) {
  const {
    t, config, isWindows, locale, setLocale, patchConfig,
    autoStart, elevatedAutoStart, handleAutoStartToggle, handleElevatedAutoStartToggle,
    uiZoom, setUiZoom, themeModeOptions, windowBlurOptions, fontOptions, titlebarItems,
    handleThemeModeChange, handleFontChange, handleWindowBlurChange,
    handleTitlebarToggle, handleTrayToggle, setRestartOpen,
    driverStatus, driverBusy, setDriverOpen, setDriverUninstallOpen,
    games, gamesLoading, manualTitle, manualExe, setManualTitle, setManualExe,
    handleAddGameManual, handleRemoveGame, handleRefreshGames, loadGames,
    logPath, clearOpen, setClearOpen, clearing, handleClearData,
    handleExportPlaintext, handleExportUsageCsv, exporting, csvExporting,
    exportResult, csvExportResult, setExportResult, setCsvExportResult,
  } = props;
  void isWindows; void locale; void setLocale; void autoStart; void elevatedAutoStart;
  void handleAutoStartToggle; void handleElevatedAutoStartToggle; void uiZoom; void setUiZoom;
  void themeModeOptions; void windowBlurOptions; void fontOptions; void titlebarItems;
  void handleThemeModeChange; void handleFontChange; void handleWindowBlurChange;
  void handleTitlebarToggle; void handleTrayToggle; void setRestartOpen;
  void driverStatus; void driverBusy; void setDriverOpen; void setDriverUninstallOpen;
  void games; void gamesLoading; void manualTitle; void manualExe; void setManualTitle; void setManualExe;
  void handleAddGameManual; void handleRemoveGame; void handleRefreshGames; void loadGames;
  void logPath; void clearOpen; void setClearOpen; void clearing; void handleClearData;
  void handleExportPlaintext; void handleExportUsageCsv; void exporting; void csvExporting;
  void exportResult; void csvExportResult; void setExportResult; void setCsvExportResult;
  void patchConfig; void config; void t;
  return (
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
  );
}