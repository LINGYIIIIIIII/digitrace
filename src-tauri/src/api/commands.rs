//! Tauri command 包装（薄层：取 state → 调方法）。
use super::*;

use crate::state::AppState;
use tauri::State;
#[tauri::command]
pub fn get_dashboard_data(
    state: State<'_, AppState>,
    start: String,
    end: String,
) -> DashboardDataDto {
    lock(&state).get_dashboard_data(start, end)
}

#[tauri::command]
pub fn get_usage_split(state: State<'_, AppState>, start: String, end: String) -> Vec<AppUsageDto> {
    lock(&state).get_usage_split(start, end)
}

#[tauri::command]
pub fn get_app_period_usage(
    state: State<'_, AppState>,
    app_name: String,
    date: String,
) -> AppPeriodUsageDto {
    lock(&state).get_app_period_usage(app_name, date)
}

#[tauri::command]
pub fn get_window_titles(
    state: State<'_, AppState>,
    app_name: String,
    date: String,
) -> Vec<PageDto> {
    lock(&state).get_window_titles(app_name, date)
}

#[tauri::command]
pub fn get_stats(state: State<'_, AppState>, start: String, end: String) -> StatsDto {
    lock(&state).get_stats(start, end)
}

#[tauri::command]
pub fn get_app_icon(state: State<'_, AppState>, exe_path: String) -> Option<IconDto> {
    lock(&state).get_app_icon(exe_path)
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> ConfigDto {
    lock(&state).get_config()
}

#[tauri::command]
pub fn set_config(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    config: ConfigDto,
) -> Result<(), String> {
    lock(&state).set_config(config).map_err(|e| e.to_string())?;
    // 托盘数据行配置可能变化，保存后立即重建托盘菜单。
    let _ = crate::tray::rebuild(&app);
    Ok(())
}

#[tauri::command]
pub fn get_network_snapshot(state: State<'_, AppState>) -> NetworkSnapshotDto {
    lock(&state).get_network_snapshot()
}

#[tauri::command]
pub fn get_network_live_window(
    state: State<'_, AppState>,
    seconds: Option<u64>,
) -> Vec<NetSampleDto> {
    lock(&state).get_network_live_window(seconds)
}

#[tauri::command]
pub fn get_day_metrics(state: State<'_, AppState>, date: String) -> DayMetricsDto {
    lock(&state).get_day_metrics(date)
}

#[tauri::command]
pub fn get_net_apps(state: State<'_, AppState>) -> NetAppsSnapshotDto {
    lock(&state).get_net_apps()
}

#[tauri::command]
pub fn get_network_history(state: State<'_, AppState>, mode: String) -> Vec<HistoryPointDto> {
    lock(&state).get_network_history(&mode)
}

#[tauri::command]
pub fn get_network_history_up(state: State<'_, AppState>, mode: String) -> Vec<HistoryPointDto> {
    lock(&state).get_network_history_up(&mode)
}

#[tauri::command]
pub fn get_hardware_snapshot(state: State<'_, AppState>) -> HardwareSnapshotDto {
    lock(&state).get_hardware_snapshot()
}

#[tauri::command]
pub fn get_temperature_snapshot(state: State<'_, AppState>) -> TemperatureSnapshotDto {
    lock(&state).get_temperature_snapshot()
}

#[tauri::command]
pub fn get_disk_health(state: State<'_, AppState>, force: bool) -> Vec<DiskHealthDto> {
    lock(&state).get_disk_health(force)
}

/// 可选安装 PawnIO 内核驱动（弹 UAC，安装动作写入运行日志）。
#[tauri::command]
pub fn install_pawnio_driver() -> DriverActionDto {
    let r = timetrace_core::install_pawnio_driver();
    DriverActionDto {
        ok: r.ok,
        message: r.message,
    }
}

/// 卸载 PawnIO 内核驱动（弹 UAC，卸载动作写入运行日志）。
#[tauri::command]
pub fn uninstall_pawnio_driver() -> DriverActionDto {
    let r = timetrace_core::uninstall_pawnio_driver();
    DriverActionDto {
        ok: r.ok,
        message: r.message,
    }
}

/// 以管理员身份重新启动数迹（读取 CPU 温度需要）。
#[tauri::command]
pub fn restart_elevated(app: tauri::AppHandle) -> DriverActionDto {
    let r = timetrace_core::restart_elevated();
    if r.ok {
        // 延迟重启器会在 1.5 秒后以管理员身份拉起新实例；
        // 当前进程立即干净退出，释放单实例锁，避免新实例被拦截。
        app.exit(0);
    }
    DriverActionDto {
        ok: r.ok,
        message: r.message,
    }
}

#[tauri::command]
pub fn get_log_path(state: State<'_, AppState>) -> String {
    lock(&state).get_log_path()
}

#[tauri::command]
pub fn is_auto_start(state: State<'_, AppState>) -> bool {
    lock(&state).is_auto_start()
}

#[tauri::command]
pub fn set_auto_start(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    lock(&state)
        .set_auto_start(enabled)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn is_elevated_auto_start(state: State<'_, AppState>) -> bool {
    lock(&state).is_elevated_auto_start()
}

#[tauri::command]
pub fn set_elevated_auto_start(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    lock(&state)
        .set_elevated_auto_start(enabled)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_week_totals(state: State<'_, AppState>) -> (i64, i64) {
    lock(&state).get_week_totals()
}

#[tauri::command]
pub fn get_day_detail(state: State<'_, AppState>, date: String) -> DayDetailDto {
    lock(&state).get_day_detail(date)
}

#[tauri::command]
pub fn get_day_hourly(state: State<'_, AppState>, date: String) -> Vec<i64> {
    lock(&state).get_day_hourly(date)
}

#[tauri::command]
pub fn get_year_heatmap(state: State<'_, AppState>, year: i32) -> Vec<(String, i64)> {
    lock(&state).get_year_heatmap(year)
}

#[tauri::command]
pub fn get_active_session_elapsed(state: State<'_, AppState>) -> i64 {
    lock(&state).get_active_session_elapsed()
}

#[tauri::command]
pub fn get_hour_apps(state: State<'_, AppState>, date: String, hour: u32) -> Vec<AppUsageDto> {
    lock(&state).get_hour_apps(date, hour)
}

#[tauri::command]
pub fn get_app_hourly(state: State<'_, AppState>, app_name: String, date: String) -> Vec<i64> {
    lock(&state).get_app_hourly(app_name, date)
}

#[tauri::command]
pub fn clear_data(state: State<'_, AppState>) {
    lock(&state).clear_data();
}

#[tauri::command]
pub fn export_csv(state: State<'_, AppState>, start: String, end: String) -> String {
    lock(&state).export_csv(start, end)
}

/// 重启应用（窗口材质等配置重启后生效）。
/// 实现：先启动一个独立延迟重启器（1 秒后拉起新实例），再干净退出当前进程，
/// 确保单实例锁与窗口资源先释放，避免新实例被旧实例拦截。
#[tauri::command]
pub fn restart_app(app: tauri::AppHandle) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // PowerShell 延迟重启器：等待 1 秒后拉起新实例；当前进程随后干净退出。
    // 不用 cmd timeout（无输入重定向时会报错退出），也避免 start 的弹窗问题。
    let exe_str = exe.to_string_lossy().replace('\'', "''");
    let ps_script = format!(
        "Start-Sleep -Seconds 1; Start-Process -FilePath '{}' -ArgumentList '--show-window'",
        exe_str
    );
    let spawned = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &ps_script,
        ])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .spawn();
    tracing::info!("restart helper spawned: {:?}", spawned.is_ok());
    if spawned.is_err() {
        return Err(format!("无法启动重启器：{e}", e = spawned.err().unwrap()));
    }
    app.exit(0);
    Ok(())
}

/// Windows 官方按应用流量查询（免管理员、非实时累计字节）。
/// 跨进程调用可能耗时数百毫秒，放到阻塞线程执行，避免卡住界面。
#[tauri::command]
pub async fn get_attributed_usage(days: u64) -> AttributedUsageResult {
    tauri::async_runtime::spawn_blocking(move || crate::attributed::query(days))
        .await
        .unwrap_or_else(|e| crate::attributed::unavailable(format!("查询线程异常：{e}")))
}

/// 导出敏感数据明文（日记 + 窗口标题）。
#[tauri::command]
pub fn export_plaintext(state: State<'_, AppState>) -> ExportResultDto {
    lock(&state).export_plaintext()
}

/// 导出使用数据 CSV（应用名/日期/活跃秒/空闲秒，全量）到 export 目录。
#[tauri::command]
pub fn export_usage_csv(state: State<'_, AppState>) -> ExportResultDto {
    lock(&state).export_usage_csv()
}

/// 在资源管理器中定位某个文件（导出/日志等）。
#[tauri::command]
pub fn reveal_in_explorer(path: String) -> Result<(), String> {
    std::process::Command::new("explorer.exe")
        .arg(format!("/select,{path}"))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// 用系统默认浏览器打开外部链接（关于页仓库链接等）。
/// 用 ShellExecuteW 而非 cmd start：避免命令注入面与控制台窗口。
#[tauri::command]
pub fn open_external_url(url: String) -> Result<(), String> {
    let wide: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
    let result = unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            windows_sys::core::w!("open"),
            wide.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        )
    };
    // ShellExecuteW 返回值 > 32 表示成功（失败时是错误码）。
    let code = result as isize;
    if code > 32 {
        Ok(())
    } else {
        Err(format!("无法打开链接（错误码 {code}）"))
    }
}

/// 前端首帧渲染完成后调用：按启动参数决定是否显示主窗口。
/// 延迟到此时再显示，避免 WebView 尚未加载完成时的纯黑首帧闪烁。
#[tauri::command]
pub fn mark_ui_ready(app: tauri::AppHandle, state: State<'_, AppState>) {
    use std::sync::atomic::Ordering;
    if state.should_show.load(Ordering::Relaxed) {
        crate::lifecycle::show_main_window(&app);
    }
}
