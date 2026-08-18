//! 应用生命周期编排：启动接管、单实例回调、后台轮询与常驻任务。
//!
//! main.rs 只负责「装配」（Builder / setup / invoke_handler / 窗口事件），
//! 「什么时候该做什么」都收敛在这里；新增常驻任务加一个 `start_*` 函数即可。

use crate::state::AppState;
use tauri::{AppHandle, Emitter, Manager};

#[cfg(windows)]
fn normalized_path(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('/', "\\").to_lowercase()
}

#[cfg(windows)]
fn normalized_stem(value: &str) -> String {
    std::path::Path::new(value)
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| value.trim_end_matches(".exe").to_lowercase())
}

#[cfg(windows)]
fn is_main_binary_name(name: &str) -> bool {
    let lower = normalized_stem(name);
    (lower.contains("数迹") || lower.contains("timetrace") || lower.contains("digitrace"))
        && !lower.contains("monitor")
        && !lower.contains("viewer")
        && !lower.contains("lite")
}

/// 判断进程是否是另一个版本的完整版主程序。
///
/// Windows 在跨完整性级别查询管理员进程时可能拒绝读取其 exe 路径。
/// 这时退回到进程名：正式发布包的主程序名包含版本号，而 monitor/viewer
/// 使用独立名称，因而不会被误当成需要接管的旧版。
#[cfg(windows)]
fn is_other_main_process(
    process_name: &str,
    process_exe: Option<&std::path::Path>,
    current_exe: &std::path::Path,
) -> bool {
    if let Some(exe) = process_exe {
        if normalized_path(exe) == normalized_path(current_exe) {
            return false;
        }
        return exe
            .file_stem()
            .map(|s| is_main_binary_name(&s.to_string_lossy()))
            .unwrap_or(false);
    }

    if !is_main_binary_name(process_name) {
        return false;
    }
    // 路径不可读且名称相同，最可能是同一路径的重复启动；交给单实例插件
    // 或 show_request 兜底处理，避免把它误判成版本切换。
    normalized_stem(process_name) != normalized_stem(&current_exe.to_string_lossy())
}

#[cfg(windows)]
fn is_same_path_process(
    process_name: &str,
    process_exe: Option<&std::path::Path>,
    current_exe: &std::path::Path,
) -> bool {
    process_exe
        .map(|exe| normalized_path(exe) == normalized_path(current_exe))
        .unwrap_or_else(|| {
            normalized_stem(process_name) == normalized_stem(&current_exe.to_string_lossy())
        })
}

/// 显示主窗口（托盘点击 / 单实例回调 / 显示请求共用）。
pub fn show_main_window<R: tauri::Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn is_ui_ready(app: &AppHandle) -> bool {
    use std::sync::atomic::Ordering;

    app.try_state::<AppState>()
        .map(|state| state.ui_ready.load(Ordering::Relaxed))
        .unwrap_or(false)
}

#[cfg(windows)]
fn show_native_takeover_prompt(app: &AppHandle, pending: crate::signals::PendingTakeover) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, IDYES, MB_ICONINFORMATION, MB_TOPMOST, MB_YESNO,
    };

    let message = format!(
        "检测到新版本 v{}。\n\n是否退出当前版本并启动新版本？",
        pending.version
    );
    let wide: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
    let answer = unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            wide.as_ptr(),
            windows_sys::core::w!("数迹"),
            MB_YESNO | MB_ICONINFORMATION | MB_TOPMOST,
        )
    };
    if answer == IDYES {
        let _ = crate::update::switch_to_pending(app.clone(), pending.exe_path, pending.elevated);
    }
}

fn dispatch_takeover_request(app: &AppHandle, pending: crate::signals::PendingTakeover) {
    let current = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    if pending.exe_path == current {
        return;
    }

    if is_ui_ready(app) {
        let _ = app.emit("takeover-pending", pending);
    } else {
        // 接管标记可能在 WebView 初始化、前端监听器注册前到达。此时事件会丢失，
        // 改由原生确认框完成交接，确保新版不会悄然落回旧版界面。
        #[cfg(windows)]
        show_native_takeover_prompt(app, pending);
        #[cfg(not(windows))]
        {
            let _ = app.emit("takeover-pending", pending);
        }
    }
}

/// 单实例插件的「第二实例」回调：唤醒本实例时统一处理——
/// 认领「待切换」标记（新版双击启动）并展开主窗口。
pub fn handle_second_instance(app: &AppHandle) {
    if let Some(pending) = crate::signals::consume_pending_takeover() {
        dispatch_takeover_request(app, pending);
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
        let name = p.name().to_string_lossy();
        is_other_main_process(&name, p.exe(), &current)
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
            pid.as_u32() != std::process::id()
                && is_same_path_process(&p.name().to_string_lossy(), p.exe(), &current)
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
            dispatch_takeover_request(&app, pending);
            show_main_window(&app);
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

#[cfg(all(test, windows))]
mod tests {
    use super::{is_main_binary_name, is_other_main_process, is_same_path_process};
    use std::path::Path;

    #[test]
    fn identifies_main_components_and_excludes_sidecars() {
        assert!(is_main_binary_name("digitrace-2.27.0.exe"));
        assert!(is_main_binary_name("数迹-v2.26.1-加密版.exe"));
        assert!(!is_main_binary_name("digitrace-monitor.exe"));
        assert!(!is_main_binary_name("digitrace-lite-viewer.exe"));
    }

    #[test]
    fn falls_back_to_process_name_when_exe_is_inaccessible() {
        let current = Path::new(r"C:\Apps\digitrace-2.27.0.exe");
        assert!(is_other_main_process("digitrace-2.26.1.exe", None, current));
        assert!(!is_other_main_process(
            "digitrace-2.27.0.exe",
            None,
            current
        ));
        assert!(!is_other_main_process(
            "digitrace-monitor.exe",
            None,
            current
        ));
    }

    #[test]
    fn path_detection_is_case_insensitive_and_handles_fallback() {
        let current = Path::new(r"C:\Apps\Digitrace.exe");
        assert!(!is_other_main_process(
            "Digitrace.exe",
            Some(Path::new(r"c:\apps\DIGITRACE.EXE")),
            current,
        ));
        assert!(is_other_main_process(
            "Digitrace.exe",
            Some(Path::new(r"c:\old\Digitrace.exe")),
            current,
        ));
        assert!(is_same_path_process("Digitrace.exe", None, current));
    }
}
