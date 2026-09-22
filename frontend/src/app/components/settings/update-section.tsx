'use client';

/** 设置分区：Update */
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

export function UpdateSection(props: SettingsSectionProps) {
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
  );
}