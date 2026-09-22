'use client';

/** 设置分区：Logs */
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
import { apiService } from '../../services/api';
import type { SettingsSectionProps } from './types';

void THEME_MODE_OPTIONS; void WINDOW_BLUR_OPTIONS; void FONT_OPTIONS; void TITLEBAR_ITEMS;
void TRAY_ITEMS; void LANGUAGE_OPTIONS; void POLL_INTERVAL_OPTIONS; void IDLE_THRESHOLD_OPTIONS;
void REFRESH_INTERVAL_OPTIONS; void LIVE_REFRESH_INTERVAL_OPTIONS; void NETWORK_LIVE_WINDOW_OPTIONS;
void fmtGameHours; void UpdateCheckCard;
void Clock; void CodeXml; void Database; void Download; void FileDown; void FileSpreadsheet;
void FileText; void FolderOpen; void Gamepad2; void GitBranch; void Globe; void Languages;
void Monitor; void Palette; void Play; void RefreshCw; void Rocket; void ShieldAlert;
void ShieldCheck; void Sparkles; void Thermometer; void Timer; void Trash2; void ZoomIn;

export function LogsSection(props: SettingsSectionProps) {
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
            <div className="px-5 py-4">
              <div className="flex items-start gap-3">
                <span className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-primary/15 bg-primary/10 text-primary">
                  <FileSpreadsheet className="h-4 w-4" />
                </span>
                <div className="min-w-0 flex-1">
                  <div className="text-sm font-medium text-foreground">
                    {t('settings.data.csvExportTitle')}
                  </div>
                  <p className="mt-0.5 text-xs leading-relaxed text-muted-foreground">
                    {t('settings.data.csvExportDescription')}
                  </p>
                  {csvExportResult?.ok && csvExportResult.path && (
                    <div className="mt-2 flex flex-wrap items-center gap-2">
                      <span className="break-all rounded-lg border border-border/60 bg-background/60 px-2.5 py-1.5 font-mono text-[11px] text-muted-foreground">
                        {csvExportResult.path}
                      </span>
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => void apiService.revealInExplorer(csvExportResult.path!)}
                      >
                        <FolderOpen className="mr-1 h-3.5 w-3.5" />
                        {t('settings.data.openFolder')}
                      </Button>
                    </div>
                  )}
                  {csvExportResult && !csvExportResult.ok && (
                    <p className="mt-2 text-xs text-destructive">
                      {csvExportResult.message ?? t('settings.data.exportFailed')}
                    </p>
                  )}
                </div>
              </div>
            </div>
            <div className="flex justify-end border-t border-border/60 px-5 py-3">
              <Button variant="outline" size="sm" loading={csvExporting} onClick={() => void handleExportUsageCsv()}>
                {t('settings.data.csvExport')}
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
  );
}