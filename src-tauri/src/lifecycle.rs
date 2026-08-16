//! 应用生命周期编排：启动接管、单实例回调、后台轮询与常驻任务。
//!
//! main.rs 只负责「装配」（Builder / setup / invoke_handler / 窗口事件），
//! 「什么时候该做什么」都收敛在这里；新增常驻任务加一个 `start_*` 函数即可。

use crate::state::AppState;
use tauri::{AppHandle, Emitter, Manager};

/// 显示主窗口（托盘点击 / 单实例回调 / 显示请求共用）。
pub fn show_main_window<R: tauri::Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// 单实例插件的「第二实例」回调：唤醒本实例时统一处理——
/// 认领「待切换」标记（新版双击启动）并展开主窗口。
pub fn handle_second_instance(app: &AppHandle) {
    if let Some(pending) = crate::signals::consume_pending_takeover() {
        let current = std::env::current_exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        if pending.exe_path != current {
            let _ = app.emit("takeover-pending", pending);
        }
    }
    show_main_window(app);
}

/// 新版启动接管：检测到其它路径的旧版数迹正在运行时，写入「待切换」标记，
/// 由旧版在自己界面里弹风格统一的确认框。
/// 同权限场景由单实例唤醒旧版处理；跨权限（如旧版以管理员运行）时，
/// 旧版通过轮询该标记（spawn_takeover_poller）处理。
/// 本进程等待旧版认领：认领成功（标记消失）则静默退出；超时（旧版太旧
/// 不支持轮询）则弹提示让用户手动退出旧版，避免“双击没反应”。
#[cfg(windows)]
pub fn maybe_takeover_old_version() {
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
        crate::signals::write_pending_takeover(
            &current.to_string_lossy(),
            timetrace_core::is_elevated(),
        );
        // 最多等 3 秒：旧版认领标记后（单实例回调或轮询），本进程静默退出。
        for _ in 0..15 {
            if !crate::signals::pending_takeover_exists() {
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
            crate::signals::write_show_request(&current.to_string_lossy());
        }
    }
}

/// 后台轮询「待切换」标记：跨权限场景（如管理员旧版）下单实例通道不通，
/// 旧版通过这里接管请求——弹出自己的窗口并询问是否切换到新版本。
/// 同时消费「显示窗口」请求：双击唤出同路径/提权实例（单实例回调被隔离拦截时兜底）。
pub fn spawn_takeover_poller(app: AppHandle) {
    std::thread::spawn(move || loop {
        if crate::signals::take_show_request().is_some() {
            show_main_window(&app);
        }
        if let Some(pending) = crate::signals::consume_pending_takeover() {
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

/// 共享内存实时指标发布：1Hz，外部工具可零拷贝读取（见 crates/metrics/metrics.h）。
pub fn start_metrics_publisher(handle: AppHandle) {
    std::thread::spawn(move || loop {
        if let Some(state) = handle.try_state::<AppState>() {
            if let Ok(mut api) = state.api.lock() {
                api.publish_metrics();
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    });
}

/// 周期性收缩主进程工作集：长驻应用把空闲代码页/缓存页还给系统，
/// 让任务管理器里主进程的占用贴近真实（按需访问时自动换回）。
pub fn start_working_set_trimmer() {
    std::thread::spawn(|| loop {
        std::thread::sleep(std::time::Duration::from_secs(300));
        timetrace_core::mem::trim_working_set();
    });
}
