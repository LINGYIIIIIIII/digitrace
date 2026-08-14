//! 托盘：悬停提示显示版本号 + 可配置数据行；右键菜单只保留操作项 + 版本号。
//!
//! 数据行内容由「设置 → 启动与托盘 → 托盘显示」决定（cpu/memory/network/active/temp），
//! 保存配置后通过 `rebuild` 立即更新；版本号固定显示在悬停提示与菜单末尾。

use std::time::Duration;

use tauri::{
    menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent, TrayIconId},
    AppHandle, Manager,
};

use timetrace_core::AppConfig;

/// 托盘数据行状态（后台线程周期性更新文本）。
pub struct TrayDataState<R: tauri::Runtime> {
    pub autostart: CheckMenuItem<R>,
    /// 当前启用的数据行，顺序与 DATA_KEYS 一致（cpu/memory/network/active/temp）。
    pub enabled: [bool; 5],
    pub tray_id: TrayIconId,
}

fn is_enabled(config: &AppConfig, key: &str) -> bool {
    config.tray_items.iter().any(|s| s == key)
}

/// 构建托盘菜单（仅操作项 + 末尾版本号）与状态。
pub fn build_menu_and_state<R: tauri::Runtime>(
    app: &AppHandle<R>,
) -> tauri::Result<(Menu<R>, TrayDataState<R>)> {
    let config = AppConfig::load();

    let enabled = [
        is_enabled(&config, "cpu"),
        is_enabled(&config, "memory"),
        is_enabled(&config, "network"),
        is_enabled(&config, "active"),
        is_enabled(&config, "temp"),
    ];

    let mut refs: Vec<&dyn IsMenuItem<R>> = Vec::new();
    let show_item = MenuItem::with_id(app, "show", "显示主界面", true, None::<&str>)?;
    let autostart = CheckMenuItem::with_id(
        app,
        "autostart",
        "开机自启动",
        true,
        // 普通注册表自启或管理员计划任务任一存在都视为已开启，
        // 否则托盘勾选状态与设置页不一致。
        timetrace_core::is_autostart_enabled() || timetrace_core::is_elevated_autostart_enabled(),
        None::<&str>,
    )?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    refs.push(&show_item);
    refs.push(&autostart);
    refs.push(&quit_item);
    let version_sep = PredefinedMenuItem::separator(app)?;
    refs.push(&version_sep);
    let version_item = MenuItem::with_id(
        app,
        "data-version",
        format!("数迹 v{}", env!("CARGO_PKG_VERSION")),
        false,
        None::<&str>,
    )?;
    refs.push(&version_item);

    let menu = Menu::with_items(app, &refs)?;
    Ok((
        menu,
        TrayDataState {
            autostart,
            enabled,
            tray_id: TrayIconId::from("main-tray"),
        },
    ))
}

/// 按最新配置重建托盘菜单（设置页保存「托盘显示」后调用）。
pub fn rebuild(app: &AppHandle) -> tauri::Result<()> {
    let (menu, mut state) = build_menu_and_state(app)?;
    if let Some(tray) = app.tray_by_id(&state.tray_id) {
        tray.set_menu(Some(menu))?;
        if let Some(old) = app.try_state::<TrayDataState<tauri::Wry>>() {
            state.tray_id = old.tray_id.clone();
        }
        app.manage(state);
    }
    Ok(())
}

/// 后台线程：每 2 秒采集一次硬件/网络/今日活跃，更新托盘悬浮提示
/// （第一行版本号，随后是已启用的数据行，每行一条）。
pub fn spawn_updater(handle: AppHandle) {
    std::thread::spawn(move || loop {
        let mut cpu_text = String::new();
        let mut mem_text = String::new();
        let mut net_text = String::new();
        let mut active_text = String::new();
        let mut temp_text = String::new();

        if let Some(state) = handle.try_state::<crate::state::AppState>() {
            if let Ok(api) = state.api.lock() {
                let hw = api.get_hardware_snapshot();
                cpu_text = format!("CPU {:.0}%", hw.cpu_percent);
                let mem_pct = if hw.memory_total_bytes > 0 {
                    (hw.memory_used_bytes as f64 / hw.memory_total_bytes as f64 * 100.0).round()
                        as u64
                } else {
                    0
                };
                mem_text = format!("内存 {}% · {}", mem_pct, format_bytes(hw.memory_used_bytes));

                let net = api.get_network_snapshot();
                net_text = format!(
                    "网速 ↓{}/s ↑{}/s",
                    format_bytes(net.download_bytes_per_sec),
                    format_bytes(net.upload_bytes_per_sec)
                );

                let temp = api.get_temperature_snapshot();
                let mut parts: Vec<String> = Vec::new();
                if let Some(c) = temp.cpu.temp_celsius {
                    parts.push(format!("CPU {c:.0}°C"));
                }
                if let Some(g) = temp.gpus.first().and_then(|g| g.temp_celsius) {
                    parts.push(format!("GPU {g:.0}°C"));
                }
                if !parts.is_empty() {
                    temp_text = format!("温度 {}", parts.join(" · "));
                }

                let now = chrono::Local::now();
                let today = now.format("%Y-%m-%d").to_string();
                let dash = api.get_dashboard_data(today.clone(), today);
                active_text = format!("活跃 {}", format_duration(dash.active_seconds));
            }
        }

        if let Some(data) = handle.try_state::<TrayDataState<tauri::Wry>>() {
            // 悬浮提示：版本号 + 已启用的数据行，分行显示更整齐。
            let values = [&cpu_text, &mem_text, &net_text, &active_text, &temp_text];
            let mut lines: Vec<String> = vec![format!("数迹 v{}", env!("CARGO_PKG_VERSION"))];
            for (i, v) in values.iter().enumerate() {
                if data.enabled.get(i).copied().unwrap_or(false) && !v.is_empty() {
                    lines.push((*v).clone());
                }
            }
            let tooltip = lines.join("\n");
            if let Some(tray) = handle.tray_by_id(&data.tray_id) {
                let _ = tray.set_tooltip(Some(tooltip));
            }
        }

        std::thread::sleep(Duration::from_secs(2));
    });
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1}GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.0}KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes}B")
    }
}

fn format_duration(seconds: i64) -> String {
    if seconds <= 0 {
        return "0分".to_string();
    }
    let h = seconds / 3600;
    let m = (seconds % 3600) / 60;
    if h > 0 {
        format!("{h}小时{m}分")
    } else {
        format!("{m}分")
    }
}

/// 供 main.rs 在 setup 中创建托盘图标（事件处理写在内部）。
pub fn create_tray_icon(
    app: &mut tauri::App,
    menu: &Menu<tauri::Wry>,
) -> tauri::Result<tauri::tray::TrayIcon> {
    let tray = TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().expect("缺省窗口图标缺失").clone())
        .menu(menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "show" => crate::show_main_window(app),
            "quit" => {
                crate::health::cleanup_toast_icon();
                // 先销毁主窗口让 WebView2 干净收尾，避免子进程残留占用内存。
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.destroy();
                }
                app.exit(0);
            }
            "autostart" => {
                if let Some(data) = app.try_state::<TrayDataState<tauri::Wry>>() {
                    let checked = data.autostart.is_checked().unwrap_or(false);
                    if let Some(state) = app.try_state::<crate::state::AppState>() {
                        if let Ok(api) = state.api.lock() {
                            let _ = api.set_auto_start(checked);
                        }
                    }
                }
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                crate::show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(tray)
}
