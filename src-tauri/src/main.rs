#![windows_subsystem = "windows"]

mod api;
mod attributed;
mod health;
mod icons;
mod state;
mod tray;
mod update;
mod window_state;

use std::sync::Arc;

use state::AppState;
use tauri::{Emitter, Manager, WindowEvent};

/// 新版启动接管：检测到其它路径的旧版数迹正在运行时，写入「待切换」标记，
/// 由旧版在自己界面里弹风格统一的确认框。
/// 同权限场景由单实例唤醒旧版处理；跨权限（如旧版以管理员运行）时，
/// 旧版通过轮询该标记（spawn_takeover_poller）处理。
/// 本进程等待旧版认领：认领成功（标记消失）则静默退出；超时（旧版太旧
/// 不支持轮询）则弹提示让用户手动退出旧版，避免“双击没反应”。
#[cfg(windows)]
fn maybe_takeover_old_version() {
    use sysinfo::{ProcessesToUpdate, System};

    let current = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let has_old = sys.processes().iter().any(|(_, p)| {
        let Some(exe) = p.exe() else { return false };
        if exe == current {
            return false;
        }
        let name = exe
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        // 只把「另一个主程序」当旧版；独立监控 / Lite 查看器（同家族不同组件）
        // 必须排除——否则 monitor 常驻时启动主程序会被误判为旧版而弹框失败。
        let is_main =
            name.contains("数迹") || name.contains("timetrace") || name.contains("digitrace");
        is_main && !name.contains("monitor") && !name.contains("viewer") && !name.contains("lite")
    });
    if has_old {
        crate::update::write_pending_takeover(
            &current.to_string_lossy(),
            timetrace_core::is_elevated(),
        );
        // 最多等 3 秒：旧版认领标记后（单实例回调或轮询），本进程静默退出。
        for _ in 0..15 {
            if !crate::update::pending_takeover_exists() {
                std::process::exit(0);
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        // 3 秒内未被认领：旧版太旧（不支持轮询），提示用户手动退出。
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            MessageBoxW, MB_ICONINFORMATION, MB_OK, MB_TOPMOST,
        };
        let text = windows_sys::core::w!(
            "检测到旧版数迹正在运行且未能自动切换。\n\n请右键托盘图标选择「退出」后，再双击新版本启动。"
        );
        let title = windows_sys::core::w!("数迹");
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text,
                title,
                MB_OK | MB_ICONINFORMATION | MB_TOPMOST,
            );
        }
        std::process::exit(0);
    }
    // 同路径实例已在运行（常见于提权实例）：单实例回调会被 Windows 完整性级别
    // 隔离拦截，双击看起来"没反应"。写入「显示窗口」请求，由运行中实例的轮询器
    // 消费并唤出主窗口（走文件系统，跨提权边界可用）。
    // `--tray`（开机自启）不写：自启保持静默，只交给单实例插件处理。
    if !std::env::args().any(|a| a == "--tray") {
        let has_same_path = sys.processes().iter().any(|(pid, p)| {
            pid.as_u32() != std::process::id() && p.exe().map(|e| e == current).unwrap_or(false)
        });
        if has_same_path {
            crate::update::write_show_request(&current.to_string_lossy());
        }
    }
}

/// 后台轮询「待切换」标记：跨权限场景（如管理员旧版）下单实例通道不通，
/// 旧版通过这里接管请求——弹出自己的窗口并询问是否切换到新版本。
/// 同时消费「显示窗口」请求：双击唤出同路径/提权实例（单实例回调被隔离拦截时兜底）。
fn spawn_takeover_poller(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        if crate::update::take_show_request().is_some() {
            show_main_window(&app);
        }
        if let Some(pending) = crate::update::consume_pending_takeover() {
            let current = std::env::current_exe()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            if pending.exe_path != current {
                let _ = app.emit("takeover-pending", pending);
                show_main_window(&app);
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    });
}

/// 显示主窗口（托盘点击 / 单实例回调共用）。
fn show_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// 按配置启用 Windows 11 窗口材质（Mica / Acrylic / Tabbed / Off）。
/// 只在启动时调用一次，改配置需重启生效（与 THRM 一致，避免运行时切换导致闪烁）。
#[cfg(windows)]
fn apply_backdrop(window: &tauri::WebviewWindow, window_blur: &str) {
    use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;

    const DWMWA_SYSTEMBACKDROP_TYPE: u32 = 38;
    const DWMSBT_NONE: i32 = 1;
    const DWMSBT_MAINWINDOW: i32 = 2;
    const DWMSBT_TABBEDWINDOW: i32 = 3;
    const DWMSBT_TRANSIENTWINDOW: i32 = 4; // Acrylic

    let backdrop: i32 = match window_blur {
        "tabbed" => DWMSBT_TABBEDWINDOW,
        "acrylic" => DWMSBT_TRANSIENTWINDOW,
        "off" => DWMSBT_NONE,
        _ => DWMSBT_MAINWINDOW, // auto / mica / 其它
    };

    if let Ok(hwnd) = window.hwnd() {
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd.0,
                DWMWA_SYSTEMBACKDROP_TYPE,
                &backdrop as *const i32 as *const core::ffi::c_void,
                std::mem::size_of::<i32>() as u32,
            );
        }
    }
}

fn main() {
    #[cfg(windows)]
    maybe_takeover_old_version();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 新版双击启动时：单实例唤醒旧版，旧版读取待切换标记并在自己界面询问。
            if let Some(pending) = crate::update::consume_pending_takeover() {
                let current = std::env::current_exe()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                if pending.exe_path != current {
                    let _ = app.emit("takeover-pending", pending);
                }
            }
            show_main_window(app);
        }))
        .setup(|app| {
            // 沿用现有规则：数据目录 %APPDATA%/TimeTrace/，数据库 time.db。
            let db_dir = dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("TimeTrace");
            std::fs::create_dir_all(&db_dir).ok();
            let db_path = db_dir.join("time.db");
            let api = match api::TimeTraceApi::create(db_path.to_string_lossy().to_string()) {
                Ok(api) => api,
                Err(e) => {
                    // 数据目录初始化失败必须让用户可见，而不是静默退出。
                    use windows_sys::Win32::UI::WindowsAndMessaging::{
                        MessageBoxW, MB_ICONERROR, MB_OK, MB_TOPMOST,
                    };
                    let msg = format!(
                        "数迹无法初始化数据目录（{}），将退出。\n\n错误：{e}",
                        db_dir.display()
                    );
                    let wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
                    unsafe {
                        MessageBoxW(
                            std::ptr::null_mut(),
                            wide.as_ptr(),
                            windows_sys::core::w!("数迹"),
                            MB_OK | MB_ICONERROR | MB_TOPMOST,
                        );
                    }
                    std::process::exit(1);
                }
            };
            let health_tracker = health::HealthTracker::start(app.handle().clone());
            app.manage(health_tracker);
            let config = timetrace_core::AppConfig::load();
            // 启动显示规则：
            // - `--show-window`（重启 / 更新 / 管理员重启 / 版本交接）→ 总是展开主窗口；
            // - `--tray`（开机自启）→ 静默驻留托盘；
            // - 其余（双击 / 手动启动）→ 遵循「启动显示主界面」。
            // 不在 setup 里直接 show：等前端渲染完首帧、调用 mark_ui_ready 后再显示，
            // 避免窗口先出现时 WebView 还是空白（纯黑首帧闪烁）。
            let args: Vec<String> = std::env::args().collect();
            let force_show = args.iter().any(|a| a == "--show-window");
            let from_autostart = args.iter().any(|a| a == "--tray");
            let should_show = force_show || (!from_autostart && config.launch_show_window);
            app.manage(AppState {
                api: std::sync::Mutex::new(api),
                should_show: std::sync::atomic::AtomicBool::new(should_show),
            });

            // 自动更新：后台每天最多检查一次；启动时提示上次更新的结果。
            update::start_background_check(app.handle().clone());
            // 清理可能残留的待切换标记（上一次交接未完成时）。
            update::clear_stale_pending_takeover();
            // 自启（--tray）必须静默：清理残留的「显示窗口」请求，避免登录时误弹窗口。
            if from_autostart {
                update::clear_stale_show_request();
            }
            // 跨权限接管兜底：轮询新版写入的「待切换」标记并弹窗询问。
            spawn_takeover_poller(app.handle().clone());
            if !config.update_silent {
                if let Some(result) = update::take_update_result() {
                    if result.starts_with("ok") {
                        health::show_toast(app.handle(), "数迹 · 更新完成", "已更新到最新版本。");
                    } else if let Some(msg) = result.strip_prefix("fail:") {
                        health::show_toast(app.handle(), "数迹 · 更新失败", msg);
                    }
                }
            } else {
                // 静默模式：吞掉更新结果，不弹通知。
                let _ = update::take_update_result();
            }

            if let Some(window) = app.get_webview_window("main") {
                apply_backdrop(&window, &config.window_blur);
                // 恢复上次的大小 / 位置 / 最大化状态。
                window_state::apply(&window);
            }

            // 托盘：可配置实时数据行 + 操作项 + 末尾版本号（见 tray.rs）。
            let (tray_menu, mut tray_state) = crate::tray::build_menu_and_state(app.handle())?;
            let tray_icon = crate::tray::create_tray_icon(app, &tray_menu)?;
            tray_state.tray_id = tray_icon.id().clone();
            app.manage(tray_state);
            crate::tray::spawn_updater(app.handle().clone());

            // 共享内存实时指标发布：1Hz，外部工具可零拷贝读取（见 crates/metrics/metrics.h）。
            {
                let handle = app.handle().clone();
                std::thread::spawn(move || loop {
                    if let Some(state) = handle.try_state::<AppState>() {
                        if let Ok(mut api) = state.api.lock() {
                            api.publish_metrics();
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_secs(1));
                });
            }

            // 周期性收缩主进程工作集：长驻应用把空闲代码页/缓存页还给系统，
            // 让任务管理器里主进程的占用贴近真实（按需访问时自动换回）。
            std::thread::spawn(|| loop {
                std::thread::sleep(std::time::Duration::from_secs(300));
                unsafe {
                    let _ = windows_sys::Win32::System::Threading::SetProcessWorkingSetSize(
                        windows_sys::Win32::System::Threading::GetCurrentProcess(),
                        usize::MAX,
                        usize::MAX,
                    );
                }
            });

            Ok(())
        })
        // 关闭窗口时隐藏到托盘，进程继续后台运行；托盘菜单「退出」才真正退出。
        .on_window_event(|window, event| {
            // 通知小窗口等辅助窗口不参与“关闭即隐藏到托盘”的逻辑。
            if window.label() != "main" {
                return;
            }
            match event {
                // 移动 / 缩放 / 最大化 / 还原时，防抖保存窗口状态。
                WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
                    window_state::schedule_save(window.app_handle());
                }
                WindowEvent::CloseRequested { api, .. } => {
                    window_state::schedule_save(window.app_handle());
                    let _ = window.hide();
                    api.prevent_close();
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            api::get_dashboard_data,
            api::get_usage_split,
            api::get_window_titles,
            api::get_stats,
            api::get_app_icon,
            api::get_config,
            api::set_config,
            api::get_network_snapshot,
            api::get_network_live_window,
            api::get_day_metrics,
            api::get_net_apps,
            api::get_attributed_usage,
            health::get_health_snapshot,
            health::test_health_notification,
            update::check_update,
            update::download_update,
            update::install_update,
            update::switch_to_pending,
            api::export_plaintext,
            api::reveal_in_explorer,
            api::open_external_url,
            api::get_network_history,
            api::get_network_history_up,
            api::get_hardware_snapshot,
            api::get_temperature_snapshot,
            api::get_disk_health,
            api::install_pawnio_driver,
            api::uninstall_pawnio_driver,
            api::restart_elevated,
            api::get_log_path,
            api::is_auto_start,
            api::set_auto_start,
            api::is_elevated_auto_start,
            api::set_elevated_auto_start,
            api::get_week_totals,
            api::get_day_detail,
            api::get_day_hourly,
            api::get_year_heatmap,
            api::get_active_session_elapsed,
            api::get_hour_apps,
            api::get_app_hourly,
            api::clear_data,
            api::export_csv,
            api::restart_app,
            api::mark_ui_ready,
        ])
        .build(tauri::generate_context!())
        .expect("数迹应用构建失败")
        .run(|app, event| {
            // 退出前清理通知气泡图标，避免托盘残留「幽灵图标」。
            if let tauri::RunEvent::Exit = event {
                health::cleanup_toast_icon();
                if let Some(tracker) = app.try_state::<Arc<health::HealthTracker>>() {
                    tracker.persist_now();
                }
                // 静默更新：退出时替换 exe，并以托盘模式静默拉起新版（无窗口无弹窗）。
                if crate::update::silent_pending_exists() {
                    crate::update::clear_silent_pending();
                    let _ = crate::update::install_silent_pending();
                }
            }
        });
}
