'use client';

/** 设置分区：General */
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

export function GeneralSection(props: SettingsSectionProps) {
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
  );
}