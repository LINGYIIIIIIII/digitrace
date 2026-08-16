//! 进程内存辅助：工作集收缩等（完整版主进程与独立监控共用）。

/// 收缩当前进程的工作集：把空闲代码页/缓存页还给系统，
/// 任务管理器里的占用（工作集）明显下降，按需访问时自动换回。
/// 适合无界面长驻进程在空闲时周期性调用。
#[cfg(windows)]
pub fn trim_working_set() {
    unsafe {
        let _ = windows::Win32::System::Threading::SetProcessWorkingSetSize(
            windows::Win32::System::Threading::GetCurrentProcess(),
            usize::MAX,
            usize::MAX,
        );
    }
}

/// 非 Windows 平台无操作（代码一致性占位）。
#[cfg(not(windows))]
pub fn trim_working_set() {}
