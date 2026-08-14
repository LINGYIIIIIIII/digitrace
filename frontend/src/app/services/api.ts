// Tauri invoke 服务封装（替代原 wailsjs）。P2 起逐个补齐 41 个 command。
import { invoke } from '@tauri-apps/api/core';
import type {
  AppConfig,
  AppUsageDto,
  AttributedUsageResult,
  DashboardDataDto,
  DayDetailDto,
  DriverActionDto,
  ExportResultDto,
  HealthSnapshotDto,
  HistoryPointDto,
  HardwareSnapshotDto,
  IconDto,
  NetAppsSnapshotDto,
  NetAppUsageDto,
  NetworkSnapshotDto,
  PageDto,
  StatsDto,
  TemperatureSnapshotDto,
  DiskHealthDto,
  UpdateActionDto,
  UpdateCheckDto,
} from '../types';

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

  async getStats(start: string, end: string): Promise<StatsDto> {
    return invoke<StatsDto>('get_stats', { start, end });
  }

  async getAppIcon(exePath: string): Promise<IconDto | null> {
    return invoke<IconDto | null>('get_app_icon', { exePath });
  }

  async getDayHourly(date: string): Promise<number[]> {
    return invoke<number[]>('get_day_hourly', { date });
  }

  async getHourApps(date: string, hour: number): Promise<AppUsageDto[]> {
    return invoke<AppUsageDto[]>('get_hour_apps', { date, hour });
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

  /** 打开浏览器开发者工具（UI 调试：点选元素实时改样式，相当于网页版 Figma）。 */
  async openDevTools(): Promise<void> {
    await invoke('open_devtools');
  }

  /** 前端首帧渲染完成，通知后端显示主窗口（避免启动时的纯黑首帧闪烁）。 */
  async markUiReady(): Promise<void> {
    await invoke('mark_ui_ready');
  }
}

export const apiService = new ApiService();
