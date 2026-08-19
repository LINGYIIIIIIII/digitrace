// Tauri invoke 服务封装（替代原 wailsjs）。P2 起逐个补齐 41 个 command。
import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import type {
  AppConfig,
  AppPeriodUsageDto,
  AppUsageDto,
  AttributedUsageResult,
  DashboardDataDto,
  DayDetailDto,
  DayMetricsDto,
  DriverActionDto,
  ExportResultDto,
  GameEntryDto,
  GameLibraryResultDto,
  GameSnapshotDto,
  HealthSnapshotDto,
  HistoryPointDto,
  HardwareSnapshotDto,
  IconDto,
  NetAppsSnapshotDto,
  NetAppUsageDto,
  NetSampleDto,
  NetworkSnapshotDto,
  PageDto,
  StatsDto,
  TemperatureSnapshotDto,
  DiskHealthDto,
  UpdateActionDto,
  UpdateCheckDto,
} from '../types';

const DEMO_APPS: AppUsageDto[] = [
  { app_name: '数迹 Demo', active_seconds: 3_420, idle_seconds: 180, exe_path: 'digitrace-demo.exe' },
  { app_name: 'Visual Studio Code', active_seconds: 2_180, idle_seconds: 96, exe_path: 'code.exe' },
  { app_name: '浏览器', active_seconds: 1_560, idle_seconds: 120, exe_path: 'browser.exe' },
  { app_name: 'Steam', active_seconds: 940, idle_seconds: 42, exe_path: 'steam.exe' },
];

const DEMO_CONFIG: AppConfig = {
  poll_interval_ms: 1000,
  idle_threshold_minutes: 5,
  refresh_interval_seconds: 10,
  live_refresh_interval_seconds: 2,
  network_live_window_seconds: 300,
  minimize_to_tray: true,
  start_minimized: false,
  auto_start_tracking: true,
  excluded_apps: [],
  db_path: 'demo',
  theme_mode: 'dark',
  window_blur: 'mica',
  font_family: 'system',
  titlebar_items: ['cpu', 'memory', 'network', 'active'],
  timezone: 'Asia/Shanghai',
  health_reminder_enabled: true,
  health_reminder_minutes: 60,
  health_break_minutes: 5,
  games_reminder_enabled: true,
  games_reminder_minutes: 60,
  update_check_enabled: true,
  update_manifest_url: '',
  update_github_repo: '',
  update_silent: false,
  update_check_hour: null,
  tray_items: ['cpu', 'memory', 'network', 'active'],
  launch_show_window: true,
};

export function isTauriRuntime(): boolean {
  if (typeof window === 'undefined') return false;
  const internals = (window as Window & {
    __TAURI_INTERNALS__?: {
      invoke?: unknown;
      metadata?: { transformCallback?: unknown };
    };
  }).__TAURI_INTERNALS__;
  return typeof internals?.invoke === 'function' && typeof internals.metadata?.transformCallback === 'function';
}

function demoSeries(length = 24, peak = 900): number[] {
  return Array.from({ length }, (_, index) => Math.round(Math.max(0, Math.sin((index - 4) / 3) * peak + peak * 0.18)));
}

function demoIcon(exePath: string): IconDto {
  const size = 32;
  const hash = Array.from(exePath).reduce((value, char) => (value * 31 + char.charCodeAt(0)) >>> 0, 7);
  const color = [40 + (hash % 80), 110 + ((hash >> 8) % 90), 180 + ((hash >> 16) % 60), 255];
  const rgba: number[] = [];
  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      const edge = Math.min(x, y, size - 1 - x, size - 1 - y);
      const inside = edge >= 3;
      rgba.push(...(inside ? color : [0, 0, 0, 0]));
    }
  }
  return { width: size, height: size, rgba };
}

async function demoInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const gameSnapshot: GameSnapshotDto = {
    enabled: true,
    reminder_minutes: 60,
    current_game: '星穹铁道',
    streak_seconds: 3_840,
    today_seconds: 7_260,
    reminders_today: 1,
    next_reminder_seconds: 1_560,
  };
  const games: GameEntryDto[] = [
    { id: 1, title: '星穹铁道', exe_path: 'starrail.exe', source: 'Demo', today_seconds: 4_560, total_seconds: 82_800 },
    { id: 2, title: '艾尔登法环', exe_path: 'eldenring.exe', source: 'Steam', today_seconds: 1_680, total_seconds: 64_200 },
    { id: 3, title: 'Hades II', exe_path: 'hades2.exe', source: 'Steam', today_seconds: 1_020, total_seconds: 28_740 },
  ];
  switch (command) {
    case 'get_config': return DEMO_CONFIG as T;
    case 'set_config': return undefined as T;
    case 'get_dashboard_data': return ({ apps: DEMO_APPS, active_seconds: 8_100, idle_seconds: 438, total_seconds: 8_538, since: '09:12' } as DashboardDataDto) as T;
    case 'get_usage_split': return DEMO_APPS as T;
    case 'get_stats': return ({ active_seconds: 8_100, idle_seconds: 438, total_seconds: 8_538, since: '09:12' } as StatsDto) as T;
    case 'get_day_hourly': return demoSeries() as T;
    case 'get_year_heatmap': return [] as T;
    case 'get_active_session_elapsed': return 1_240 as T;
    case 'get_hour_apps': return DEMO_APPS.slice(0, 3) as T;
    case 'get_day_hour_apps': return Array.from({ length: 24 }, () => DEMO_APPS.slice(0, 2)) as T;
    case 'get_app_hourly': return demoSeries(24, 420) as T;
    case 'get_week_totals': return [32_400, 2_100] as T;
    case 'get_day_detail': return ({ date: String(args?.date ?? ''), active_seconds: 8_100, idle_seconds: 438, session_count: 12, diary: '', sessions: [] } as DayDetailDto) as T;
    case 'get_window_titles': return [{ title: 'Digitrace dashboard', seconds: 1_200 }] as T;
    case 'get_app_period_usage': return ({ app_name: String(args?.appName ?? 'Demo'), today_seconds: 3_420, week_seconds: 12_600, month_seconds: 42_000 } as AppPeriodUsageDto) as T;
    case 'get_app_icon': return demoIcon(String(args?.exePath ?? 'demo.exe')) as T;
    case 'get_game_snapshot': return gameSnapshot as T;
    case 'get_games_library': return games as T;
    case 'refresh_game_library': return ({ ok: true, found: games.length, message: null } as GameLibraryResultDto) as T;
    case 'add_game_manual': return ({ ok: true, found: games.length + 1, message: null } as GameLibraryResultDto) as T;
    case 'remove_game': return ({ ok: true, found: games.length - 1, message: null } as GameLibraryResultDto) as T;
    case 'get_network_snapshot': return ({ upload_bytes_per_sec: 1_200_000, download_bytes_per_sec: 8_400_000, session_upload_bytes: 840_000_000, session_download_bytes: 4_200_000_000, adapter_count: 1 } as NetworkSnapshotDto) as T;
    case 'get_net_apps': return ({ bytes_available: true, etw_mode: false, apps: [] } as NetAppsSnapshotDto) as T;
    case 'get_attributed_usage': return ({ available: true, apps: [], message: null, since_local: '', until_local: '' } as AttributedUsageResult) as T;
    case 'get_health_snapshot': return ({ enabled: true, reminder_minutes: 60, break_minutes: 5, streak_seconds: 3_840, idle_seconds: 0, reminders_today: 1, next_reminder_seconds: 1_560, last_reminder_local: null, last_break_local: null } as HealthSnapshotDto) as T;
    case 'get_network_history':
    case 'get_network_history_up': return [] as T;
    case 'get_network_live_window': return [] as T;
    case 'get_hardware_snapshot': return ({ cpu_percent: 32, memory_total_bytes: 16_000_000_000, memory_used_bytes: 8_400_000_000, disks: [] } as HardwareSnapshotDto) as T;
    case 'get_day_metrics': return ({ cpu_percent: [], mem_percent: [], cpu_temp_c: [], gpu_usage_percent: [], gpu_temp_c: [], gpu_power_watts: [], net_down_bps: [], net_up_bps: [] } as DayMetricsDto) as T;
    case 'get_temperature_snapshot': return ({ cpu: { available: true, temp_celsius: 48, package_celsius: 50, per_core: [], source: 'Demo', driver_installed: true, driver_running: true, driver_version: null, needs_admin: false, message: null }, gpus: [], disks: [] } as TemperatureSnapshotDto) as T;
    case 'get_disk_health': return [] as T;
    case 'is_auto_start':
    case 'is_elevated_auto_start': return false as T;
    case 'check_update': return ({ current_version: 'demo', latest_version: 'demo', has_update: false, url: '', sha256: '', notes: '', message: null } as UpdateCheckDto) as T;
    case 'download_update':
    case 'install_update':
    case 'switch_to_pending': return ({ ok: false, message: 'Demo mode' } as UpdateActionDto) as T;
    case 'get_log_path': return 'Demo mode' as T;
    case 'export_plaintext':
    case 'export_usage_csv': return ({ ok: false, path: null, message: 'Demo mode' } as ExportResultDto) as T;
    case 'get_active_session_elapsed': return 1_240 as T;
    default: return undefined as T;
  }
}

function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauriRuntime()) return tauriInvoke<T>(command, args);
  return demoInvoke<T>(command, args);
}

class ApiService {
  async getConfig(): Promise<AppConfig> {
    return invoke<AppConfig>('get_config');
  }

  async setConfig(config: AppConfig): Promise<void> {
    return invoke('set_config', { config });
  }

  async isAutoStart(): Promise<boolean> {
    return invoke<boolean>('is_auto_start');
  }

  async setAutoStart(enabled: boolean): Promise<void> {
    return invoke('set_auto_start', { enabled });
  }

  async isElevatedAutoStart(): Promise<boolean> {
    return invoke<boolean>('is_elevated_auto_start');
  }

  async setElevatedAutoStart(enabled: boolean): Promise<void> {
    return invoke('set_elevated_auto_start', { enabled });
  }

  async getLogPath(): Promise<string> {
    return invoke<string>('get_log_path');
  }

  async clearData(): Promise<void> {
    return invoke('clear_data');
  }

  async getDashboardData(start: string, end: string): Promise<DashboardDataDto> {
    return invoke<DashboardDataDto>('get_dashboard_data', { start, end });
  }

  async getUsageSplit(start: string, end: string): Promise<AppUsageDto[]> {
    return invoke<AppUsageDto[]>('get_usage_split', { start, end });
  }

  async getAppPeriodUsage(appName: string, date: string): Promise<AppPeriodUsageDto> {
    return invoke<AppPeriodUsageDto>('get_app_period_usage', { appName, date });
  }

  async getStats(start: string, end: string): Promise<StatsDto> {
    return invoke<StatsDto>('get_stats', { start, end });
  }

  async getAppIcon(exePath: string): Promise<IconDto | null> {
    return invoke<IconDto | null>('get_app_icon', { exePath });
  }

  async getDayHourly(date: string): Promise<number[]> {
    return invoke<number[]>('get_day_hourly', { date });
  }

  async getYearHeatmap(year: number): Promise<[string, number][]> {
    return invoke<[string, number][]>('get_year_heatmap', { year });
  }

  async getActiveSessionElapsed(): Promise<number> {
    return invoke<number>('get_active_session_elapsed');
  }

  async getHourApps(date: string, hour: number): Promise<AppUsageDto[]> {
    return invoke<AppUsageDto[]>('get_hour_apps', { date, hour });
  }

  async getDayHourApps(date: string): Promise<AppUsageDto[][]> {
    return invoke<AppUsageDto[][]>('get_day_hour_apps', { date });
  }

  async getAppHourly(appName: string, date: string): Promise<number[]> {
    return invoke<number[]>('get_app_hourly', { appName, date });
  }

  async getWeekTotals(): Promise<[number, number]> {
    return invoke<[number, number]>('get_week_totals');
  }

  async getDayDetail(date: string): Promise<DayDetailDto> {
    return invoke<DayDetailDto>('get_day_detail', { date });
  }

  async getWindowTitles(appName: string, date: string): Promise<PageDto[]> {
    return invoke<PageDto[]>('get_window_titles', { appName, date });
  }

  async getNetworkSnapshot(): Promise<NetworkSnapshotDto> {
    return invoke<NetworkSnapshotDto>('get_network_snapshot');
  }

  async getNetApps(): Promise<NetAppsSnapshotDto> {
    return invoke<NetAppsSnapshotDto>('get_net_apps');
  }

  async getAttributedUsage(days: number): Promise<AttributedUsageResult> {
    return invoke<AttributedUsageResult>('get_attributed_usage', { days });
  }

  async getHealthSnapshot(): Promise<HealthSnapshotDto> {
    return invoke<HealthSnapshotDto>('get_health_snapshot');
  }

  async testHealthNotification(): Promise<void> {
    return invoke('test_health_notification');
  }

  async checkForUpdate(): Promise<UpdateCheckDto> {
    return invoke<UpdateCheckDto>('check_update');
  }

  async downloadUpdate(): Promise<UpdateActionDto> {
    return invoke<UpdateActionDto>('download_update');
  }

  async installUpdate(): Promise<UpdateActionDto> {
    return invoke<UpdateActionDto>('install_update');
  }

  async switchToPending(path: string, elevated = false): Promise<UpdateActionDto> {
    return invoke<UpdateActionDto>('switch_to_pending', { path, elevated });
  }

  async exportPlaintext(): Promise<ExportResultDto> {
    return invoke<ExportResultDto>('export_plaintext');
  }

  async exportUsageCsv(): Promise<ExportResultDto> {
    return invoke<ExportResultDto>('export_usage_csv');
  }

  async getGameSnapshot(): Promise<GameSnapshotDto> {
    return invoke<GameSnapshotDto>('get_game_snapshot');
  }

  async getGamesLibrary(): Promise<GameEntryDto[]> {
    return invoke<GameEntryDto[]>('get_games_library');
  }

  async refreshGameLibrary(): Promise<GameLibraryResultDto> {
    return invoke<GameLibraryResultDto>('refresh_game_library');
  }

  async addGameManual(title: string, exePath: string): Promise<GameLibraryResultDto> {
    return invoke<GameLibraryResultDto>('add_game_manual', { title, exePath });
  }

  async removeGame(id: number): Promise<GameLibraryResultDto> {
    return invoke<GameLibraryResultDto>('remove_game', { id });
  }

  async revealInExplorer(path: string): Promise<void> {
    return invoke('reveal_in_explorer', { path });
  }

  async openExternalUrl(url: string): Promise<void> {
    return invoke('open_external_url', { url });
  }

  async getNetworkHistory(mode: string): Promise<HistoryPointDto[]> {
    return invoke<HistoryPointDto[]>('get_network_history', { mode });
  }

  async getNetworkHistoryUp(mode: string): Promise<HistoryPointDto[]> {
    return invoke<HistoryPointDto[]>('get_network_history_up', { mode });
  }

  async getHardwareSnapshot(): Promise<HardwareSnapshotDto> {
    return invoke<HardwareSnapshotDto>('get_hardware_snapshot');
  }

  /** 实时曲线窗口：最近 seconds 秒的秒级网络样本（缺省 5 分钟）。 */
  async getNetworkLiveWindow(seconds?: number): Promise<NetSampleDto[]> {
    return invoke<NetSampleDto[]>('get_network_live_window', { seconds: seconds ?? null });
  }

  /** 日历日仪表盘：某日历日的硬件/温度/网络分钟级序列。 */
  async getDayMetrics(date: string): Promise<DayMetricsDto> {
    return invoke<DayMetricsDto>('get_day_metrics', { date });
  }

  async getTemperatureSnapshot(): Promise<TemperatureSnapshotDto> {
    return invoke<TemperatureSnapshotDto>('get_temperature_snapshot');
  }

  async getDiskHealth(force = false): Promise<DiskHealthDto[]> {
    return invoke<DiskHealthDto[]>('get_disk_health', { force });
  }

  async installPawnioDriver(): Promise<DriverActionDto> {
    return invoke<DriverActionDto>('install_pawnio_driver');
  }

  async uninstallPawnioDriver(): Promise<DriverActionDto> {
    return invoke<DriverActionDto>('uninstall_pawnio_driver');
  }

  async restartElevated(): Promise<DriverActionDto> {
    return invoke<DriverActionDto>('restart_elevated');
  }

  async exportCsv(start: string, end: string): Promise<string> {
    return invoke<string>('export_csv', { start, end });
  }

  async restartApp(): Promise<void> {
    return invoke('restart_app');
  }

  /** 前端首帧渲染完成，通知后端显示主窗口（避免启动时的纯黑首帧闪烁）。 */
  async markUiReady(): Promise<void> {
    await invoke('mark_ui_ready');
  }
}

export const apiService = new ApiService();
