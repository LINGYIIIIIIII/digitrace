'use client';

/** 设置分区：Hardware */
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

export function HardwareSection(props: SettingsSectionProps) {
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
  );
}