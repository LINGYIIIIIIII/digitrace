'use client';

/** 设置分区共享类型（与 SettingsPage 状态/handler 对齐）。 */
import type { TFunction } from 'i18next';
import type { AppConfig, CpuTemperatureDto, ExportResultDto, GameEntryDto } from '../../types';

export type SettingsPatch = Partial<AppConfig>;

export interface SettingsSectionProps {
  t: TFunction;
  config: AppConfig | null;
  patchConfig: (patch: SettingsPatch) => Promise<void>;
  isWindows: boolean;
  locale: string;
  setLocale: (l: string) => void;
  autoStart: boolean;
  elevatedAutoStart: boolean;
  handleAutoStartToggle: (v: boolean) => void;
  handleElevatedAutoStartToggle: (v: boolean) => void;
  uiZoom: number;
  setUiZoom: (v: number) => void;
  themeModeOptions: { value: string; label: string }[];
  windowBlurOptions: { value: string; label: string }[];
  fontOptions: { value: string; label: string }[];
  titlebarItems: string[];
  handleThemeModeChange: (mode: string) => void;
  handleFontChange: (family: string) => void;
  handleWindowBlurChange: (mode: string) => void;
  handleTitlebarToggle: (value: string, enabled: boolean) => void;
  handleTrayToggle: (value: string, enabled: boolean) => void;
  setRestartOpen: (v: boolean) => void;
  driverStatus: CpuTemperatureDto | null;
  driverBusy: boolean;
  setDriverOpen: (v: boolean) => void;
  setDriverUninstallOpen: (v: boolean) => void;
  games: GameEntryDto[] | null;
  gamesLoading: boolean;
  manualTitle: string;
  manualExe: string;
  setManualTitle: (v: string) => void;
  setManualExe: (v: string) => void;
  handleAddGameManual: () => void;
  handleRemoveGame: (id: number) => void;
  handleRefreshGames: () => void;
  loadGames: () => void;
  logPath: string;
  clearOpen: boolean;
  setClearOpen: (v: boolean) => void;
  clearing: boolean;
  handleClearData: () => void;
  handleExportPlaintext: () => void;
  handleExportUsageCsv: () => void;
  exporting: boolean;
  csvExporting: boolean;
  exportResult: ExportResultDto | null;
  csvExportResult: ExportResultDto | null;
  setExportResult: (v: ExportResultDto | null) => void;
  setCsvExportResult: (v: ExportResultDto | null) => void;
}