//! TimeTrace self auto-start registration (HKCU Run).
//!
//! User-level only — no admin rights needed. Writes a single value under
//! `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` that points at the
//! current executable. Removing it fully unregisters.

#[cfg(windows)]
use std::os::windows::process::CommandExt;
use winreg::RegKey;
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};

use crate::config::AppConfig;

const RUN_KEY_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE_NAME: &str = "TimeTrace";
/// 旧版（Tauri 插件 / 早期版本）曾用「数迹」作为注册表值名。
const LEGACY_VALUE_NAME: &str = "数迹";
/// 管理员自启计划任务名（登录时以最高权限运行，开机静默提权）。
const TASK_NAME: &str = "DigitraceElevatedAutostart";

/// Whether TimeTrace is currently registered to auto-start at logon.
pub fn is_autostart_enabled() -> bool {
    let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN_KEY_PATH, KEY_READ)
    else {
        return false;
    };
    matches!(
        key.get_value::<String, _>(RUN_VALUE_NAME),
        Ok(v) if !v.trim().is_empty()
    )
}

/// 管理员自启计划任务是否已注册。
pub fn is_elevated_autostart_enabled() -> bool {
    let query_ok = std::process::Command::new("schtasks")
        .args(["/query", "/tn", TASK_NAME])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW：避免查询时闪控制台窗口
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    if matches!(query_ok, Ok(s) if s.success()) {
        return true;
    }
    // 兜底：直接检查任务文件是否存在（查询偶发权限/编码问题时不误判）。
    std::env::var("WINDIR")
        .ok()
        .map(|w| {
            std::path::Path::new(&w)
                .join("System32")
                .join("Tasks")
                .join(TASK_NAME)
                .exists()
        })
        .unwrap_or(false)
}

/// 注册「登录时以最高权限运行」的计划任务（需要管理员，会弹一次 UAC）。
/// 注册后开机自启以管理员身份静默运行，无需每次开机确认。
pub fn enable_elevated_autostart() -> Result<(), String> {
    if is_elevated_autostart_enabled() {
        return Ok(());
    }
    let tr = run_command()?;
    let args = format!("/create /tn \"{TASK_NAME}\" /tr \"{tr}\" /sc onlogon /rl highest /f");
    if !crate::engine::temperature::run_elevated("schtasks.exe", &args) {
        return Err("无法启动计划任务注册（UAC 被取消或被安全软件拦截）".to_string());
    }
    // 轮询等待注册完成（schtasks 异步返回）。
    for _ in 0..15 {
        if is_elevated_autostart_enabled() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    Err("计划任务注册失败（请确认已通过 UAC 授权）".to_string())
}

/// 删除管理员自启计划任务（需要管理员，会弹一次 UAC）。
pub fn disable_elevated_autostart() -> Result<(), String> {
    if !is_elevated_autostart_enabled() {
        return Ok(());
    }
    let args = format!("/delete /tn \"{TASK_NAME}\" /f");
    if !crate::engine::temperature::run_elevated("schtasks.exe", &args) {
        return Err("无法启动计划任务删除（UAC 被取消或被安全软件拦截）".to_string());
    }
    Ok(())
}

/// Register TimeTrace to auto-start at logon (current exe path).
pub fn enable_autostart() -> Result<(), String> {
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(RUN_KEY_PATH, KEY_READ | KEY_WRITE)
        .map_err(|e| format!("cannot open HKCU Run key: {e}"))?;
    let command = run_command()?;
    key.set_value(RUN_VALUE_NAME, &command)
        .map_err(|e| format!("cannot write HKCU Run\\{RUN_VALUE_NAME}: {e}"))?;
    // 清理旧版遗留的「数迹」自启值，避免开机重复启动旧版本。
    if let Ok(v) = key.get_value::<String, _>(LEGACY_VALUE_NAME)
        && is_related_command(&v)
    {
        let _ = key.delete_value(LEGACY_VALUE_NAME);
    }
    Ok(())
}

/// Remove the auto-start registration. Missing value is not an error.
pub fn disable_autostart() -> Result<(), String> {
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(RUN_KEY_PATH, KEY_READ | KEY_WRITE)
        .map_err(|e| format!("cannot open HKCU Run key: {e}"))?;
    match key.delete_value(RUN_VALUE_NAME) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("cannot remove HKCU Run\\{RUN_VALUE_NAME}: {e}")),
    }
}

/// The command line stored in the Run key, e.g. `"C:\...\timetrace_app.exe" --tray`.
/// `--tray` 标记让开机自启固定静默驻留托盘（不弹出主窗口）。
fn run_command() -> Result<String, String> {
    let exe =
        std::env::current_exe().map_err(|e| format!("cannot resolve current executable: {e}"))?;
    Ok(format!("\"{}\" --tray", exe.display()))
}

/// 值内容是否与本应用相关（避免误删其它软件的 Run 项）。
fn is_related_command(cmd: &str) -> bool {
    let lower = cmd.to_lowercase();
    lower.contains("timetrace") || lower.contains("数迹")
}

/// 启动时自愈自启记录：
/// - 删除旧版遗留的「数迹」值；
/// - 若「TimeTrace」已存在但指向旧版本路径，迁移到当前 exe；
/// - 存在有效自启记录时确保 start_minimized（开机静默进托盘）；
/// - 管理员自启计划任务存在/启用时，用当前 exe 刷新任务路径。
pub fn heal_autostart() {
    // 管理员自启任务自愈优先执行，避免下面注册表操作提前 return 时被跳过。
    heal_elevated_autostart();

    let Ok(key) = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(RUN_KEY_PATH, KEY_READ | KEY_WRITE)
    else {
        return;
    };

    if let Ok(v) = key.get_value::<String, _>(LEGACY_VALUE_NAME)
        && is_related_command(&v)
    {
        let _ = key.delete_value(LEGACY_VALUE_NAME);
    }

    let mut should_minimize = false;
    match key.get_value::<String, _>(RUN_VALUE_NAME) {
        Ok(v) if !v.trim().is_empty() => {
            if let Ok(exe) = std::env::current_exe() {
                // 必须保留 --tray：否则开机自启会弹出主窗口而不是静默进托盘。
                let want = format!("\"{}\" --tray", exe.display());
                if v.trim() != want {
                    let _ = key.set_value(RUN_VALUE_NAME, &want);
                }
                should_minimize = true;
            }
        }
        _ => {}
    }

    if should_minimize {
        let mut config = AppConfig::load();
        if !config.start_minimized {
            config.start_minimized = true;
            let _ = config.save();
        }
    }
}

/// 自愈管理员自启计划任务（需要管理员权限）：
/// - 任务指向旧 exe 时，用当前 exe 刷新（/f 覆盖）；
/// - 配置为管理员自启但任务缺失时重建；
/// - 刷新成功后移除注册表项，避免开机双开。
fn heal_elevated_autostart() {
    if !crate::engine::temperature::is_elevated() {
        return;
    }
    let want_elevated = is_elevated_autostart_enabled() || AppConfig::load().autostart_elevated;
    if !want_elevated {
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let tr = format!("\"{}\" --tray", exe.display());
    let ok = std::process::Command::new("schtasks")
        .args([
            "/create", "/tn", TASK_NAME, "/tr", &tr, "/sc", "onlogon", "/rl", "highest", "/f",
        ])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .status();
    if matches!(ok, Ok(s) if s.success()) {
        // 管理员自启生效时移除注册表项，避免开机重复启动两个实例。
        let _ = disable_autostart();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_autostart_state_does_not_crash() {
        // Read-only: must never panic, regardless of whether TimeTrace is
        // registered on this machine.
        let _ = is_autostart_enabled();
    }
}
