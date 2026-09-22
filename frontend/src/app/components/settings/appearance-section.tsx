'use client';

/** 设置分区：Appearance */
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
import { applyUiZoom } from '../../lib/appearance';
import type { SettingsSectionProps } from './types';

void THEME_MODE_OPTIONS; void WINDOW_BLUR_OPTIONS; void FONT_OPTIONS; void TITLEBAR_ITEMS;
void TRAY_ITEMS; void LANGUAGE_OPTIONS; void POLL_INTERVAL_OPTIONS; void IDLE_THRESHOLD_OPTIONS;
void REFRESH_INTERVAL_OPTIONS; void LIVE_REFRESH_INTERVAL_OPTIONS; void NETWORK_LIVE_WINDOW_OPTIONS;
void fmtGameHours; void UpdateCheckCard;
void Clock; void CodeXml; void Database; void Download; void FileDown; void FileSpreadsheet;
void FileText; void FolderOpen; void Gamepad2; void GitBranch; void Globe; void Languages;
void Monitor; void Palette; void Play; void RefreshCw; void Rocket; void ShieldAlert;
void ShieldCheck; void Sparkles; void Thermometer; void Timer; void Trash2; void ZoomIn;

export function AppearanceSection(props: SettingsSectionProps) {
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
  );
}