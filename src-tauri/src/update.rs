//! 轻量自研自动更新（免管理员、兼容单 exe 分发）。
//!
//! 两种更新源（配置 `update_github_repo` 优先）：
//! 1. **GitHub Releases 直读（类似 THRM）**：读取公开仓库最新 Release——
//!    取 tag 作版本、`.exe` 附件作安装包、同 Release 内的 `.sha256` 附件作校验；
//! 2. **更新清单 JSON**：`{ version, url, sha256, notes }`，地址必须 HTTPS。
//!
//! 统一流程：版本号比较 → 有新版 → curl 下载到 %APPDATA%\TimeTrace\updates\
//! → SHA-256 强校验（防篡改，不通过就删除拒绝安装）
//! → 隐藏 PowerShell 辅助进程：等待旧程序退出 → 覆盖当前 exe → 启动新版。
//!
//! 安全说明：校验文件缺失或校验失败绝不覆盖现有版本。

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
/// 静默更新待安装标记：静默模式下载完成后写入，进程退出时消费（替换 exe）。
const SILENT_PENDING_FILE: &str = "silent_pending.txt";
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

/// GitHub Releases API 响应（只取需要的字段）。
#[derive(Debug, Clone, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    assets: Vec<GhAsset>,
    #[serde(default)]
    body: String,
}

#[derive(Debug, Clone, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Clone, Serialize, Default)]
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

/// 把 Release tag 转成纯数字版本串（"v2.20.0" / "release-2.20.0" → "2.20.0"）。
fn tag_to_version_str(tag: &str) -> String {
    tag.trim()
        .trim_start_matches(|c: char| !c.is_ascii_digit())
        .to_string()
}

fn version_from_tag(tag: &str) -> (u64, u64, u64) {
    parse_version(&tag_to_version_str(tag))
}

fn fetch_manifest(url: &str) -> Result<UpdateManifest, String> {
    let out = Command::new("curl.exe")
        .args(["-sS", "-L", "-m", "25"])
        .arg(url)
        .creation_flags(0x08000000) // CREATE_NO_WINDOW：避免检查更新时闪控制台窗口
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

/// 读取公开仓库的最新 Release（GitHub API，匿名即可；需要 User-Agent）。
fn fetch_github_release(repo: &str) -> Result<GhRelease, String> {
    if !repo.contains('/') {
        return Err("GitHub 仓库格式应为「所有者/仓库名」，如 LINGYIIIIIIII/digitrace".to_string());
    }
    let api_url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let out = Command::new("curl.exe")
        .args(["-sS", "-L", "-m", "25", "-H", "User-Agent: Digitrace"])
        .arg(&api_url)
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output()
        .map_err(|e| format!("无法启动 curl（需要 Windows 10 1803+）：{e}"))?;
    if !out.status.success() {
        return Err(format!(
            "读取 GitHub Release 失败（请确认仓库为公开且已发布 Release，私有仓库无法匿名读取）：{}",
            String::from_utf8_lossy(&out.stderr).trim().chars().take(120).collect::<String>()
        ));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("GitHub Release 数据格式错误：{e}"))
}

/// 取 Release 里的安装包附件：优先选择与当前程序同名的 `.exe`；
/// 排除 viewer / lite / monitor / update / setup / installer 等非主程序附件。
fn pick_exe_asset(rel: &GhRelease) -> Option<&GhAsset> {
    let current = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    let candidates: Vec<&GhAsset> = rel
        .assets
        .iter()
        .filter(|a| {
            let n = a.name.to_ascii_lowercase();
            n.ends_with(".exe")
                && !n.contains("viewer")
                && !n.contains("lite")
                && !n.contains("monitor")
                && !n.contains("update")
                && !n.contains("setup")
                && !n.contains("installer")
        })
        .collect();
    if let Some(cur) = &current {
        if let Some(asset) = candidates
            .iter()
            .find(|a| a.name.to_ascii_lowercase() == *cur)
        {
            return Some(asset);
        }
    }
    candidates.first().copied()
}

/// 取 Release 里的 SHA-256 校验附件（`*.sha256` 或 `*.sha256.txt`），内容为 64 位十六进制。
fn fetch_sha256_asset(rel: &GhRelease) -> Result<String, String> {
    let candidate = rel
        .assets
        .iter()
        .find(|a| {
            let n = a.name.to_ascii_lowercase();
            n.ends_with(".sha256") || n.ends_with(".sha256.txt")
        })
        .ok_or_else(|| {
            "该 Release 缺少 .sha256 校验文件（安全要求，拒绝更新；请在发布时一并上传）".to_string()
        })?;
    let out = Command::new("curl.exe")
        .args([
            "-sS",
            "-L",
            "-m",
            "30",
            "--retry",
            "2",
            "--connect-timeout",
            "10",
        ])
        .arg(&candidate.browser_download_url)
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output()
        .map_err(|e| format!("无法下载校验文件：{e}"))?;
    if !out.status.success() {
        let detail = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if detail.is_empty() {
            "下载 .sha256 校验文件失败（网络异常）".to_string()
        } else {
            format!("下载 .sha256 校验文件失败：{detail}")
        });
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let hash = text
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Release 的 .sha256 文件内容不是合法的 64 位十六进制哈希".to_string());
    }
    Ok(hash)
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
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
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
    // GitHub 仓库模式优先（类似 THRM：直接读公开仓库 Release）。
    let repo = config.update_github_repo.trim();
    if !repo.is_empty() {
        return evaluate_github(repo, &mut dto);
    }
    let url = config.update_manifest_url.trim();
    if url.is_empty() {
        dto.message = Some("未配置更新源（设置 → 更新）".to_string());
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

/// GitHub Releases 模式检查：最新 Release 的 tag + `.exe` 附件 + `.sha256` 附件。
fn evaluate_github(repo: &str, dto: &mut UpdateCheckDto) -> UpdateCheckDto {
    let rel = match fetch_github_release(repo) {
        Ok(r) => r,
        Err(e) => {
            dto.message = Some(e);
            return std::mem::take(dto);
        }
    };
    let Some(exe) = pick_exe_asset(&rel) else {
        dto.message = Some("该 Release 没有 .exe 安装包".to_string());
        return std::mem::take(dto);
    };
    // 检查阶段只确认校验文件存在，不实际下载（下载阶段才拉取并强校验），
    // 避免"检查更新"被网络波动卡住。
    let has_sha = rel.assets.iter().any(|a| {
        let n = a.name.to_ascii_lowercase();
        n.ends_with(".sha256") || n.ends_with(".sha256.txt")
    });
    if !has_sha {
        dto.message = Some(
            "该 Release 缺少 .sha256 校验文件（安全要求，拒绝更新；请在发布时一并上传）"
                .to_string(),
        );
        return std::mem::take(dto);
    }
    dto.latest_version = tag_to_version_str(&rel.tag_name);
    dto.url = exe.browser_download_url.clone();
    dto.notes = rel.body.trim().chars().take(500).collect();
    dto.has_update = version_from_tag(&rel.tag_name) > parse_version(&dto.current_version);
    std::mem::take(dto)
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
    // GitHub 仓库模式优先。
    let repo = config.update_github_repo.trim().to_string();
    if !repo.is_empty() {
        return download_github_blocking(app, &repo);
    }
    let manifest_url = config.update_manifest_url.trim().to_string();
    if manifest_url.is_empty() {
        return action(false, "未配置更新源（设置 → 更新）");
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
    download_and_verify(app, &manifest.url, &manifest.sha256)
}

/// GitHub 模式下载：取最新 Release 的 `.exe` 附件 + `.sha256` 校验文件后下载。
fn download_github_blocking(app: &AppHandle, repo: &str) -> UpdateActionDto {
    let rel = match fetch_github_release(repo) {
        Ok(r) => r,
        Err(e) => return action(false, e),
    };
    let Some(exe) = pick_exe_asset(&rel) else {
        return action(false, "该 Release 没有 .exe 安装包");
    };
    let expected = match fetch_sha256_asset(&rel) {
        Ok(s) => s,
        Err(e) => return action(false, e),
    };
    if !exe.browser_download_url.starts_with("https://") {
        return action(false, "下载地址必须使用 HTTPS（安全要求）");
    }
    download_and_verify(app, &exe.browser_download_url, &expected)
}

/// 下载 → SHA-256 强校验 → 落盘（两种更新源共用）。
fn download_and_verify(app: &AppHandle, url: &str, expected_sha256: &str) -> UpdateActionDto {
    let dir = updates_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return action(false, format!("无法创建更新目录：{e}"));
    }
    let tmp = dir.join("数迹-update.exe.part");
    let target = dir.join(FIXED_UPDATE_NAME);
    let _ = std::fs::remove_file(&tmp);

    let total = content_length(url);
    let mut child = match Command::new("curl.exe")
        .args(["-sS", "-L", "--retry", "2", "--connect-timeout", "20"])
        .arg(url)
        .arg("-o")
        .arg(&tmp)
        .creation_flags(0x08000000) // CREATE_NO_WINDOW：下载全程无控制台窗口
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
    // sha256 非空已由调用方强制校验，这里直接比对。
    if !actual.eq_ignore_ascii_case(expected_sha256.trim()) {
        let _ = std::fs::remove_file(&tmp);
        return action(
            false,
            format!(
                "SHA-256 校验失败，已拒绝安装（期望 {}，实际 {}）",
                expected_sha256.trim(),
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
    match spawn_installer_ps(false) {
        Ok(()) => {
            clear_silent_pending();
            app.exit(0);
            UpdateActionDto {
                ok: true,
                message: None,
            }
        }
        Err(e) => action(false, e),
    }
}

/// 隐藏 PowerShell 辅助进程：轮询等待旧 exe 退出（最多 25 秒）→ 覆盖 →
/// 记录结果 → 按模式拉起新版（`--show-window` 前台 / `--tray` 静默托盘）。
fn spawn_installer_ps(to_tray: bool) -> Result<(), String> {
    let target = updates_dir().join(FIXED_UPDATE_NAME);
    if !target.exists() {
        return Err("未找到已下载的更新包，请先下载".to_string());
    }
    let current = std::env::current_exe().map_err(|e| format!("无法定位当前程序：{e}"))?;
    let new_path = target.to_string_lossy().replace('\'', "''");
    let cur_path = current.to_string_lossy().replace('\'', "''");
    let result_file = data_dir().join(RESULT_FILE);
    let result_path = result_file.to_string_lossy().replace('\'', "''");
    let relaunch_arg = if to_tray {
        "'--tray'"
    } else {
        "'--show-window'"
    };
    let ps = format!(
        "$d=(Get-Date).AddSeconds(25); while((Get-Date) -lt $d){{ $p=Get-Process | Where-Object {{ $_.Path -eq '{cur_path}' }}; if(-not $p){{ break }}; Start-Sleep -Milliseconds 300 }}; try {{ Copy-Item -LiteralPath '{new_path}' -Destination '{cur_path}' -Force; Set-Content -LiteralPath '{result_path}' -Value 'ok' }} catch {{ Set-Content -LiteralPath '{result_path}' -Value ('fail:' + $_.Exception.Message) }}; Start-Process -FilePath '{cur_path}' -ArgumentList {relaunch_arg}"
    );
    let spawned = Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .spawn();
    spawned
        .map(|_| ())
        .map_err(|e| format!("无法启动更新辅助进程：{e}"))
}

/// 静默模式：后台下载新版本并落盘，写入「待安装」标记，不弹任何提示。
/// 进程退出时由 `install_silent_pending` 消费。返回是否已下载完成。
pub fn silent_download_and_mark(app: &AppHandle) -> bool {
    let ok = download_blocking(app).ok;
    if ok {
        let _ = std::fs::write(
            data_dir().join(SILENT_PENDING_FILE),
            env!("CARGO_PKG_VERSION"),
        );
    }
    ok
}

/// 静默安装（进程退出时调用）：以托盘模式替换并拉起新版，无任何界面。
pub fn install_silent_pending() -> bool {
    spawn_installer_ps(true).is_ok()
}

/// 是否存在待安装的静默更新。
pub fn silent_pending_exists() -> bool {
    data_dir().join(SILENT_PENDING_FILE).exists()
}

/// 清除「待安装」标记。
pub fn clear_silent_pending() {
    let _ = std::fs::remove_file(data_dir().join(SILENT_PENDING_FILE));
}

/// 启动后台自动检查（配置了更新源且开关打开时生效）。
/// 检查频率：固定时刻（update_check_hour，0-23 点）→ 睡到当天该小时；
/// 否则每 6 小时轮询。两者都按自然日去重（每天最多检查一次），
/// 且启动后约 6 秒会先查一次（当天未查过时）。
pub fn start_background_check(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(6));
        loop {
            let config = timetrace_core::AppConfig::load();
            let has_source = !config.update_manifest_url.trim().is_empty()
                || !config.update_github_repo.trim().is_empty();
            if config.update_check_enabled && has_source {
                let today = chrono::Local::now().format("%Y-%m-%d").to_string();
                let last =
                    std::fs::read_to_string(data_dir().join(LAST_CHECK_FILE)).unwrap_or_default();
                if last.trim() != today {
                    let dto = evaluate_update();
                    if dto.has_update {
                        if config.update_silent {
                            // 静默模式：直接后台下载，不弹任何提示。
                            let _ = silent_download_and_mark(&app);
                        } else {
                            let _ = app.emit("update-available", dto);
                        }
                    }
                    let _ = std::fs::write(data_dir().join(LAST_CHECK_FILE), today);
                }
            }
            // 下一次检查间隔：固定时刻 → 睡到当天该小时（已过则明天）；
            // 否则每 6 小时轮询。
            let wait = if let Some(hour) = config.update_check_hour.filter(|h| *h <= 23) {
                sleep_until_hour(hour)
            } else {
                Duration::from_secs(6 * 3600)
            };
            std::thread::sleep(wait);
        }
    });
}

/// 距下一次「指定小时（本地时间，加 5 分钟余量）」的等待时长。
fn sleep_until_hour(hour: u32) -> Duration {
    let now = chrono::Local::now();
    let mut next = now
        .date_naive()
        .and_hms_opt(hour, 5, 0)
        .unwrap_or_else(|| now.date_naive().and_hms_opt(0, 0, 0).unwrap())
        .and_local_timezone(chrono::Local)
        .earliest()
        .unwrap_or(now);
    if next <= now {
        next += chrono::Duration::days(1);
    }
    (next - now).to_std().unwrap_or(Duration::from_secs(3600))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_version_parsing() {
        assert_eq!(tag_to_version_str("v2.20.0"), "2.20.0");
        assert_eq!(tag_to_version_str("release-2.20.0"), "2.20.0");
        assert_eq!(tag_to_version_str("2.19.1"), "2.19.1");
        assert!(parse_version("2.20.0") > parse_version("2.19.1"));
        assert!(version_from_tag("v2.20.0") > parse_version("2.19.1"));
        assert!(version_from_tag("v2.20.0") == parse_version("2.20.0"));
    }

    #[test]
    fn exe_asset_picking() {
        let rel = GhRelease {
            tag_name: "v1.0.0".to_string(),
            assets: vec![
                GhAsset {
                    name: "readme.md".to_string(),
                    browser_download_url: "https://x/readme.md".to_string(),
                },
                GhAsset {
                    name: "digitrace-v1.0.0.exe".to_string(),
                    browser_download_url: "https://x/digitrace-v1.0.0.exe".to_string(),
                },
                GhAsset {
                    name: "digitrace-v1.0.0.exe.sha256".to_string(),
                    browser_download_url: "https://x/digitrace-v1.0.0.exe.sha256".to_string(),
                },
            ],
            body: String::new(),
        };
        let exe = pick_exe_asset(&rel).expect("应选中 .exe 附件");
        assert_eq!(exe.name, "digitrace-v1.0.0.exe");
    }

    #[test]
    fn sha256_asset_name_matches() {
        let rel = GhRelease {
            tag_name: "v1.0.0".to_string(),
            assets: vec![GhAsset {
                name: "digitrace-v1.0.0.exe.SHA256".to_string(),
                browser_download_url: "https://x/hash".to_string(),
            }],
            body: String::new(),
        };
        assert!(rel.assets.iter().any(|a| {
            let n = a.name.to_ascii_lowercase();
            n.ends_with(".sha256") || n.ends_with(".sha256.txt")
        }));
    }
}
