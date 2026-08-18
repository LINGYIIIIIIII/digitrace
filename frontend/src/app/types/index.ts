// 数迹 DTO（对应 Rust bridge 侧定义，P2 起逐步补齐字段）

export interface AppConfig {
  poll_interval_ms: number;
  idle_threshold_minutes: number;
  refresh_interval_seconds: number;
  live_refresh_interval_seconds: number;
  network_live_window_seconds: number;
  minimize_to_tray: boolean;
  start_minimized: boolean;
  auto_start_tracking: boolean;
  excluded_apps: string[];
  db_path: string;
  theme_mode: string;
  window_blur: string;
  font_family: string;
  titlebar_items: string[];
  timezone: string;
  health_reminder_enabled: boolean;
  health_reminder_minutes: number;
  health_break_minutes: number;
  games_reminder_enabled: boolean;
  games_reminder_minutes: number;
  update_check_enabled: boolean;
  update_manifest_url: string;
  update_github_repo: string;
  update_silent: boolean;
  update_check_hour: number | null;
  tray_items: string[];
  launch_show_window: boolean;
}

export interface AppUsageDto {
  app_name: string;
  active_seconds: number;
  idle_seconds: number;
  exe_path: string;
}

export interface AppPeriodUsageDto {
  app_name: string;
  today_seconds: number;
  week_seconds: number;
  month_seconds: number;
}

export interface PageDto {
  title: string;
  seconds: number;
}

export interface StatsDto {
  active_seconds: number;
  idle_seconds: number;
  total_seconds: number;
  since: string | null;
}

export interface IconDto {
  width: number;
  height: number;
  rgba: number[];
}

export interface DaySessionDto {
  app_name: string;
  is_idle: boolean;
  duration_secs: number;
  started_at: string;
}

export interface DayDetailDto {
  date: string;
  active_seconds: number;
  idle_seconds: number;
  session_count: number;
  diary: string;
  sessions: DaySessionDto[];
}

export interface DashboardDataDto {
  apps: AppUsageDto[];
  active_seconds: number;
  idle_seconds: number;
  total_seconds: number;
  since: string | null;
}

export interface StatDto {
  total_active_seconds: number;
  total_idle_seconds: number;
  app_count: number;
}

export interface NetworkSnapshotDto {
  upload_bytes_per_sec: number;
  download_bytes_per_sec: number;
  session_upload_bytes: number;
  session_download_bytes: number;
  adapter_count: number;
}

/** 秒级网络样本（实时曲线缓冲）。 */
export interface NetSampleDto {
  ts: number;
  down: number;
  up: number;
}

/** 日历日仪表盘：某日历日的分钟级指标点（avg）。 */
export interface DayMetricPointDto {
  minute: number;
  avg: number;
}

/** 日历日仪表盘：硬件/温度/网络分钟级序列。 */
export interface DayMetricsDto {
  cpu_percent: DayMetricPointDto[];
  mem_percent: DayMetricPointDto[];
  cpu_temp_c: DayMetricPointDto[];
  gpu_usage_percent: DayMetricPointDto[];
  gpu_temp_c: DayMetricPointDto[];
  net_down_bps: DayMetricPointDto[];
  net_up_bps: DayMetricPointDto[];
}

export interface NetAppUsageDto {
  app_name: string;
  exe_path: string;
  download_bps: number;
  upload_bps: number;
  session_download: number;
  session_upload: number;
  active_connections: number;
  total_connections: number;
}

export interface NetAppsSnapshotDto {
  bytes_available: boolean;
  /** true=ETW 合计模式（实时总流量，不分下载/上传）。 */
  etw_mode: boolean;
  apps: NetAppUsageDto[];
}

export interface AttributedAppUsage {
  app_id: string;
  app_name: string;
  exe_path: string;
  download_bytes: number;
  upload_bytes: number;
  total_bytes: number;
}

export interface AttributedUsageResult {
  available: boolean;
  apps: AttributedAppUsage[];
  message: string | null;
  since_local: string;
  until_local: string;
}

export interface HealthSnapshotDto {
  enabled: boolean;
  reminder_minutes: number;
  break_minutes: number;
  streak_seconds: number;
  idle_seconds: number;
  reminders_today: number;
  next_reminder_seconds: number;
  last_reminder_local: string | null;
  last_break_local: string | null;
}

export interface UpdateCheckDto {
  current_version: string;
  latest_version: string;
  has_update: boolean;
  url: string;
  sha256: string;
  notes: string;
  message: string | null;
}

export interface UpdateProgressDto {
  percent: number;
  downloaded_bytes: number;
  total_bytes: number;
  phase: string;
}

export interface UpdateActionDto {
  ok: boolean;
  message: string | null;
}

export interface PendingTakeoverDto {
  exe_path: string;
  version: string;
}

export interface ExportResultDto {
  ok: boolean;
  path: string | null;
  message: string | null;
}

export interface GameEntryDto {
  id: number;
  title: string;
  exe_path: string;
  source: string;
  today_seconds: number;
  total_seconds: number;
}

export interface GameLibraryResultDto {
  ok: boolean;
  found: number;
  message: string | null;
}

export interface GameSnapshotDto {
  enabled: boolean;
  reminder_minutes: number;
  current_game: string | null;
  streak_seconds: number;
  today_seconds: number;
  reminders_today: number;
  next_reminder_seconds: number;
}

export interface HistoryPointDto {
  day: string;
  minute: number;
  avg: number;
  max: number;
}

export interface DiskSnapshotDto {
  drive: string;
  total_bytes: number;
  available_bytes: number;
}

export interface HardwareSnapshotDto {
  cpu_percent: number;
  memory_total_bytes: number;
  memory_used_bytes: number;
  disks: DiskSnapshotDto[];
}

export interface CpuTemperatureDto {
  available: boolean;
  temp_celsius: number | null;
  package_celsius: number | null;
  per_core: number[];
  source: string;
  driver_installed: boolean;
  driver_running: boolean;
  driver_version: string | null;
  needs_admin: boolean;
  message: string | null;
}

export interface GpuTemperatureDto {
  name: string;
  temp_celsius: number | null;
  usage_percent: number | null;
}

export interface DiskTemperatureDto {
  drive: string;
  model: string;
  temp_celsius: number | null;
}

export interface DiskHealthDto {
  name: string;
  status: string;
  media_type: string;
  temp_celsius: number | null;
  wear_percent: number | null;
  power_on_hours: number | null;
  read_errors: number | null;
  write_errors: number | null;
}

export interface TemperatureSnapshotDto {
  cpu: CpuTemperatureDto;
  gpus: GpuTemperatureDto[];
  disks: DiskTemperatureDto[];
}

export interface DriverActionDto {
  ok: boolean;
  message: string;
}
