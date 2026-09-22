'use client';

/** 设置分区：Games */
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

export function GamesSection(props: SettingsSectionProps) {
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
                  icon={<Gamepad2 className="h-4 w-4" />}
                  title={t('settings.games.reminder')}
                  description={t('settings.games.reminderDescription')}
                >
                  <ToggleSwitch
                    enabled={config?.games_reminder_enabled ?? false}
                    onChange={(v) => void patchConfig({ games_reminder_enabled: v })}
                  />
                </SettingRow>
                <div className="border-t border-border/60">
                  <SettingRow
                    icon={<Clock className="h-4 w-4" />}
                    title={t('settings.games.reminderMinutes')}
                    description={t('settings.games.reminderMinutesDescription')}
                  >
                    <div className="w-32">
                      <Select
                        value={String(config?.games_reminder_minutes ?? 120)}
                        onChange={(v) => void patchConfig({ games_reminder_minutes: Number(v) })}
                        options={[30, 45, 60, 90, 120, 180, 240].map((m) => ({
                          value: String(m),
                          label: t('settings.games.minutes', { minutes: m }),
                        }))}
                        size="sm"
                      />
                    </div>
                  </SettingRow>
                </div>
              </Card>

              <Card padding="none" className="overflow-hidden">
                <div className="flex items-center justify-between px-5 py-4">
                  <div className="flex items-start gap-3">
                    <span className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-primary/15 bg-primary/10 text-primary">
                      <Gamepad2 className="h-4 w-4" />
                    </span>
                    <div>
                      <div className="text-sm font-medium text-foreground">
                        {t('settings.games.library')}
                      </div>
                      <p className="mt-0.5 text-xs leading-relaxed text-muted-foreground">
                        {t('settings.games.libraryDescription')}
                      </p>
                    </div>
                  </div>
                  <Button
                    variant="outline"
                    size="sm"
                    loading={gamesLoading}
                    onClick={() => void handleRefreshGames()}
                  >
                    <RefreshCw className="mr-1 h-3.5 w-3.5" />
                    {t('settings.games.refresh')}
                  </Button>
                </div>
                <div className="border-t border-border/60">
                  {games === null ? (
                    <div className="px-5 py-4">
                      <Button variant="outline" size="sm" onClick={() => void loadGames()}>
                        {t('settings.games.load')}
                      </Button>
                    </div>
                  ) : games.length === 0 ? (
                    <p className="px-5 py-4 text-xs text-muted-foreground">
                      {t('settings.games.empty')}
                    </p>
                  ) : (
                    <ul className="max-h-72 divide-y divide-border/60 overflow-y-auto">
                      {games.map((g) => (
                        <li key={g.id} className="flex items-center gap-2 px-5 py-2.5">
                          <div className="min-w-0 flex-1">
                            <div className="flex items-center gap-2">
                              <span className="truncate text-sm font-medium text-foreground">
                                {g.title}
                              </span>
                              <span className="shrink-0 rounded border border-border/60 bg-muted/40 px-1 py-px text-[10px] text-muted-foreground">
                                {g.source}
                              </span>
                            </div>
                            <div className="truncate font-mono text-[11px] text-muted-foreground">
                              {g.exe_path}
                            </div>
                          </div>
                          <div className="shrink-0 text-right text-xs text-muted-foreground">
                            <div>
                              {t('settings.games.today')} {fmtGameHours(g.today_seconds)}
                            </div>
                            <div>
                              {t('settings.games.total')} {fmtGameHours(g.total_seconds)}
                            </div>
                          </div>
                          <Button
                            variant="ghost"
                            size="sm"
                            className="shrink-0"
                            onClick={() => void handleRemoveGame(g.id)}
                          >
                            <Trash2 className="h-3.5 w-3.5" />
                          </Button>
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              </Card>

              <Card padding="none" className="overflow-hidden">
                <div className="px-5 py-4">
                  <div className="flex items-start gap-3">
                    <span className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-primary/15 bg-primary/10 text-primary">
                      <Gamepad2 className="h-4 w-4" />
                    </span>
                    <div className="min-w-0 flex-1">
                      <div className="text-sm font-medium text-foreground">
                        {t('settings.games.manualAdd')}
                      </div>
                      <p className="mt-0.5 text-xs leading-relaxed text-muted-foreground">
                        {t('settings.games.manualAddDescription')}
                      </p>
                      <div className="mt-2 flex flex-col gap-2">
                        <Input
                          className="h-8 font-mono text-xs"
                          placeholder={t('settings.games.manualTitlePlaceholder')}
                          value={manualTitle}
                          onChange={(e) => setManualTitle(e.target.value)}
                        />
                        <Input
                          className="h-8 font-mono text-xs"
                          placeholder={t('settings.games.manualExePlaceholder')}
                          value={manualExe}
                          onChange={(e) => setManualExe(e.target.value)}
                        />
                      </div>
                    </div>
                  </div>
                </div>
                <div className="flex justify-end border-t border-border/60 px-5 py-3">
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => void handleAddGameManual()}
                  >
                    {t('settings.games.add')}
                  </Button>
                </div>
              </Card>
    </>
  );
}