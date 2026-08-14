//! 轻量自研自动更新（免管理员、兼容单 exe 分发）。
//!
//! 流程：
//! 1. 拉取更新清单 JSON（`{ version, url, sha256, notes }`，走 HTTPS）；
//! 2. 版本号比较，有新版 → 用系统自带 curl 下载到 %APPDATA%\TimeTrace\updates\；
//! 3. SHA-256 强校验（防篡改，不通过就删除拒绝安装）；
//! 4. 隐藏 PowerShell 辅助进程：等待旧程序退出 → 覆盖当前 exe → 启动新版。
//!
//! 安全说明：清单地址必须 HTTPS；校验失败绝不覆盖现有版本。

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

pub const UPDATE_DIR: &str = "updates";
const FIXED_UPDATE_NAME: &str = "数迹-update.exe";
const LAST_CHECK_FILE: &str = "last_update_check.txt";
const RESULT_FILE: &str = "update_result.txt";
const PENDING_TAKEOVER_FILE: &str = "pending_takeover.json";
/// 认领用的临时文件名：用 rename 原子地“认领”接管请求，
/// 避免单实例回调和轮询线程同时处理导致重复弹窗。
const PENDING_TAKEOVER_CLAIM: &str = "pending_takeover.claim";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateManifest {
    pub version: String,
    pub url: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateCheckDto {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub url: String,
    pub sha256: String,
    pub notes: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateProgressDto {
    /// -1 表示总大小未知（不确定进度）。
    pub percent: f64,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub phase: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateActionDto {
    pub ok: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingTakeover {
    pub exe_path: String,
    pub version: String,
    /// 新版是否以管理员身份运行：旧版认领后需用 RunAs 拉起，避免权限丢失。
    #[serde(default)]
    pub elevated: bool,
}

fn data_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("TimeTrace")
}

fn updates_dir() -> PathBuf {
    data_dir().join(UPDATE_DIR)
}

fn pending_takeover_path() -> PathBuf {
    data_dir().join(PENDING_TAKEOVER_FILE)
}

/// 新版进程启动时写入「待切换」标记（检测到旧版正在运行）。
/// 旧版收到单实例唤醒后读取并删除它，再在自己界面里询问用户。
pub fn write_pending_takeover(exe_path: &str, elevated: bool) {
    let p = PendingTakeover {
        exe_path: exe_path.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        elevated,
    };
    if let Ok(json) = serde_json::to_string(&p) {
        let _ = std::fs::write(pending_takeover_path(), json);
    }
}

/// 原子认领待切换标记（旧版单实例回调 / 轮询线程共用）。
/// rename 成功的一方获得处理权，另一方返回 None。
pub fn consume_pending_takeover() -> Option<PendingTakeover> {
    let src = pending_takeover_path();
    let dst = data_dir().join(PENDING_TAKEOVER_CLAIM);
    if std::fs::rename(&src, &dst).is_err() {
        return None;
    }
    let content = std::fs::read_to_string(&dst).ok();
    let _ = std::fs::remove_file(&dst);
    content.and_then(|c| serde_json::from_str(&c).ok())
}

/// 待切换标记是否仍然存在（新版进程轮询它判断旧版是否已接管）。
pub fn pending_takeover_exists() -> bool {
    pending_takeover_path().exists()
}

/// 启动时清理可能残留的待切换标记（上一次交接未完成时）。
pub fn clear_stale_pending_takeover() {
    let _ = std::fs::remove_file(pending_takeover_path());
}

/// 前端点「立即切换」：启动新版 exe 后干净退出当前进程。
#[tauri::command]
pub fn switch_to_pending(app: AppHandle, path: String, elevated: bool) -> UpdateActionDto {
    // 用隐藏 PowerShell 延迟 1 秒再启动新版：等当前进程完全退出、
    // 单实例锁释放后再拉起，避免竞态导致新版被拦下或窗口不显示。
    let exe_str = path.replace('\'', "''");
    // `--show-window`：版本交接后新实例总是展开主窗口。
    // 新版是管理员运行时，用 -Verb RunAs 拉起，保持管理员权限（会弹 UAC 确认）。
    let verb = if elevated { " -Verb RunAs" } else { "" };
    let ps_script = format!(
        "Start-Sleep -Seconds 1; Start-Process -FilePath '{}'{verb} -ArgumentList '--show-window'",
        exe_str,
    );
    match std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &ps_script,
        ])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .spawn()
    {
        Ok(_) => {
            app.exit(0);
            UpdateActionDto {
                ok: true,
                message: None,
            }
        }
        Err(e) => UpdateActionDto {
            ok: false,
            message: Some(format!("无法启动新版本：{e}")),
        },
    }
}

fn parse_version(v: &str) -> (u64, u64, u64) {
    let mut it = v
        .trim()
        .split(['.', '-', '+'])
        .map(|s| s.parse::<u64>().unwrap_or(0));
    (
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
    )
}

fn fetch_manifest(url: &str) -> Result<UpdateManifest, String> {
    let out = Command::new("curl.exe")
        .args(["-sS", "-L", "-m", "25"])
        .arg(url)
        .output()
        .map_err(|e| format!("无法启动 curl（需要 Windows 10 1803+）：{e}"))?;
    if !out.status.success() {
        return Err(format!(
            "拉取更新清单失败：{}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("更新清单格式错误：{e}"))
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("读取更新包失败：{e}"))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn content_length(url: &str) -> u64 {
    if let Ok(out) = Command::new("curl.exe")
        .args(["-sIL", "-m", "15"])
        .arg(url)
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
        for line in text.lines() {
            if let Some(v) = line.strip_prefix("content-length:") {
                if let Ok(n) = v.trim().parse::<u64>() {
                    return n;
                }
            }
        }
    }
    0
}

fn action(ok: bool, message: impl Into<String>) -> UpdateActionDto {
    UpdateActionDto {
        ok,
        message: Some(message.into()),
    }
}

/// 核心检查逻辑（供手动按钮与启动自动检查共用）。
pub fn evaluate_update() -> UpdateCheckDto {
    let config = timetrace_core::AppConfig::load();
    let current = env!("CARGO_PKG_VERSION").to_string();
    let mut dto = UpdateCheckDto {
        current_version: current,
        latest_version: String::new(),
        has_update: false,
        url: String::new(),
        sha256: String::new(),
        notes: String::new(),
        message: None,
    };
    let url = config.update_manifest_url.trim();
    if url.is_empty() {
        dto.message = Some("未配置更新地址（设置 → 更新）".to_string());
        return dto;
    }
    if !url.starts_with("https://") {
        dto.message = Some("更新地址必须使用 HTTPS（安全要求）".to_string());
        return dto;
    }
    match fetch_manifest(url) {
        Ok(m) => {
            if m.sha256.trim().is_empty() {
                dto.message = Some("更新清单缺少 sha256 字段，已拒绝（安全要求）".to_string());
                return dto;
            }
            dto.latest_version = m.version.clone();
            dto.url = m.url;
            dto.sha256 = m.sha256;
            dto.notes = m.notes;
            dto.has_update = parse_version(&m.version) > parse_version(&dto.current_version);
        }
        Err(e) => dto.message = Some(e),
    }
    dto
}

/// 手动「检查更新」。
#[tauri::command]
pub async fn check_update() -> UpdateCheckDto {
    tauri::async_runtime::spawn_blocking(evaluate_update)
        .await
        .unwrap_or_else(|e| {
            let mut d = evaluate_update();
            d.message = Some(format!("检查线程异常：{e}"));
            d
        })
}

/// 下载 + SHA-256 校验新版本（进度通过 `update-progress` 事件推给前端）。
#[tauri::command]
pub async fn download_update(app: AppHandle) -> UpdateActionDto {
    tauri::async_runtime::spawn_blocking(move || download_blocking(&app))
        .await
        .unwrap_or_else(|e| action(false, format!("下载线程异常：{e}")))
}

fn download_blocking(app: &AppHandle) -> UpdateActionDto {
    let config = timetrace_core::AppConfig::load();
    let manifest_url = config.update_manifest_url.trim().to_string();
    if manifest_url.is_empty() {
        return action(false, "未配置更新地址");
    }
    if !manifest_url.starts_with("https://") {
        return action(false, "更新地址必须使用 HTTPS（安全要求）");
    }
    let manifest = match fetch_manifest(&manifest_url) {
        Ok(m) => m,
        Err(e) => return action(false, e),
    };
    if manifest.sha256.trim().is_empty() {
        return action(false, "更新清单缺少 sha256 字段，已拒绝（安全要求）");
    }
    if !manifest.url.starts_with("https://") {
        return action(false, "下载地址必须使用 HTTPS（安全要求）");
    }

    let dir = updates_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return action(false, format!("无法创建更新目录：{e}"));
    }
    let tmp = dir.join("数迹-update.exe.part");
    let target = dir.join(FIXED_UPDATE_NAME);
    let _ = std::fs::remove_file(&tmp);

    let total = content_length(&manifest.url);
    let mut child = match Command::new("curl.exe")
        .args(["-sS", "-L", "--retry", "2", "--connect-timeout", "20"])
        .arg(&manifest.url)
        .arg("-o")
        .arg(&tmp)
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return action(false, format!("无法启动下载：{e}")),
    };

    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    let progress_app = app.clone();
    let progress_tmp = tmp.clone();
    let progress_handle = std::thread::spawn(move || {
        while !stop2.load(Ordering::Relaxed) {
            let size = std::fs::metadata(&progress_tmp)
                .map(|m| m.len())
                .unwrap_or(0);
            let percent = if total > 0 {
                (size as f64 / total as f64 * 100.0).min(100.0)
            } else {
                -1.0
            };
            let _ = progress_app.emit(
                "update-progress",
                UpdateProgressDto {
                    percent,
                    downloaded_bytes: size,
                    total_bytes: total,
                    phase: "downloading".to_string(),
                },
            );
            std::thread::sleep(Duration::from_millis(400));
        }
    });

    let status = child.wait();
    stop.store(true, Ordering::Relaxed);
    let _ = progress_handle.join();

    if !status.map(|s| s.success()).unwrap_or(false) {
        let _ = std::fs::remove_file(&tmp);
        return action(false, "下载失败（网络中断或文件不可达）");
    }

    let actual = match sha256_file(&tmp) {
        Ok(s) => s,
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            return action(false, e);
        }
    };
    // sha256 非空已在上面强制校验，这里直接比对。
    if !actual.eq_ignore_ascii_case(manifest.sha256.trim()) {
        let _ = std::fs::remove_file(&tmp);
        return action(
            false,
            format!(
                "SHA-256 校验失败，已拒绝安装（期望 {}，实际 {}）",
                manifest.sha256.trim(),
                actual
            ),
        );
    }
    if let Err(e) = std::fs::rename(&tmp, &target) {
        let _ = std::fs::remove_file(&tmp);
        return action(false, format!("保存更新包失败：{e}"));
    }
    let _ = app.emit(
        "update-progress",
        UpdateProgressDto {
            percent: 100.0,
            downloaded_bytes: total,
            total_bytes: total,
            phase: "done".to_string(),
        },
    );
    UpdateActionDto {
        ok: true,
        message: None,
    }
}

/// 安装更新：等待旧进程退出 → 覆盖当前 exe → 启动新版 → 退出当前进程。
#[tauri::command]
pub fn install_update(app: AppHandle) -> UpdateActionDto {
    let target = updates_dir().join(FIXED_UPDATE_NAME);
    if !target.exists() {
        return action(false, "未找到已下载的更新包，请先下载");
    }
    let current = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => return action(false, format!("无法定位当前程序：{e}")),
    };
    let new_path = target.to_string_lossy().replace('\'', "''");
    let cur_path = current.to_string_lossy().replace('\'', "''");
    let result_file = data_dir().join(RESULT_FILE);
    let result_path = result_file.to_string_lossy().replace('\'', "''");
    // PowerShell 辅助进程：轮询等待旧 exe 退出（最多 25 秒）→ 覆盖 → 记录结果 → 启动新版。
    let ps = format!(
        "$d=(Get-Date).AddSeconds(25); while((Get-Date) -lt $d){{ $p=Get-Process | Where-Object {{ $_.Path -eq '{cur_path}' }}; if(-not $p){{ break }}; Start-Sleep -Milliseconds 300 }}; try {{ Copy-Item -LiteralPath '{new_path}' -Destination '{cur_path}' -Force; Set-Content -LiteralPath '{result_path}' -Value 'ok' }} catch {{ Set-Content -LiteralPath '{result_path}' -Value ('fail:' + $_.Exception.Message) }}; Start-Process -FilePath '{cur_path}' -ArgumentList '--show-window'"
    );
    let spawned = Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .spawn();
    if spawned.is_err() {
        return action(false, "无法启动更新辅助进程");
    }
    app.exit(0);
    UpdateActionDto {
        ok: true,
        message: None,
    }
}

/// 启动后台自动检查（每天最多一次，仅当配置了更新地址且开关打开）。
pub fn start_background_check(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(6));
        loop {
            let config = timetrace_core::AppConfig::load();
            if config.update_check_enabled && !config.update_manifest_url.trim().is_empty() {
                let today = chrono::Local::now().format("%Y-%m-%d").to_string();
                let last =
                    std::fs::read_to_string(data_dir().join(LAST_CHECK_FILE)).unwrap_or_default();
                if last.trim() != today {
                    let dto = evaluate_update();
                    if dto.has_update {
                        let _ = app.emit("update-available", dto);
                    }
                    let _ = std::fs::write(data_dir().join(LAST_CHECK_FILE), today);
                }
            }
            std::thread::sleep(Duration::from_secs(6 * 3600));
        }
    });
}

/// 启动时读取上次更新结果（ok/fail:...），返回给调用方处理（显示通知）。
pub fn take_update_result() -> Option<String> {
    let path = data_dir().join(RESULT_FILE);
    let content = std::fs::read_to_string(&path).ok()?;
    let _ = std::fs::remove_file(&path);
    let v = content.trim().to_string();
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}
