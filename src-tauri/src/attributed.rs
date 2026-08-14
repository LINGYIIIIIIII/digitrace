//! Windows 官方「按应用流量」查询（免管理员）。
//!
//! 数据源：`ConnectionProfile.GetAttributedNetworkUsageAsync`，与 Windows 设置里
//! 「网络和 Internet → 数据使用量 → 查看各应用的使用情况」完全一致。
//! 已知限制（微软文档明确）：
//! - 非实时：Windows 按小时/天级记账，拿不到秒级实时速率；
//! - 只覆盖当前数据计费周期，更早的历史拿不到；
//! - 部分流量（系统进程等）可能归入「未归属」桶，分应用之和与总量可能不一致。

use std::collections::HashMap;

use serde::Serialize;
use sysinfo::{ProcessesToUpdate, System};

#[derive(Debug, Clone, Serialize)]
pub struct AttributedAppUsage {
    /// 归属标识（应用 ID / 包名）。Windows 可能在重置或配置变更后更换该值，
    /// 不能当作永久主键。
    pub app_id: String,
    /// 应用显示名（AttributionName）。
    pub app_name: String,
    /// 可执行文件完整路径（用于提取图标；无路径时为空）。
    pub exe_path: String,
    /// 接收字节数 = 下载。
    pub download_bytes: u64,
    /// 发送字节数 = 上传。
    pub upload_bytes: u64,
    /// 合计。
    pub total_bytes: u64,
}

/// 从 Windows 归属信息解析「显示名 + exe 路径」。
/// 桌面应用的 AttributionName 通常是完整路径，这里提取 exe 文件名作显示名。
fn normalize_app(display: &str, id: &str) -> (String, String) {
    let looks_like_path =
        |s: &str| s.contains('\\') || s.contains('/') || s.to_lowercase().ends_with(".exe");
    if looks_like_path(display) {
        let path = nt_to_drive_path(display).unwrap_or_else(|| display.to_string());
        let name = exe_stem(&path).unwrap_or_else(|| display.to_string());
        (name, path)
    } else if looks_like_path(id) {
        let path = nt_to_drive_path(id).unwrap_or_else(|| id.to_string());
        let name = exe_stem(&path).unwrap_or_else(|| {
            if display.is_empty() {
                path.clone()
            } else {
                display.to_string()
            }
        });
        (name, path)
    } else {
        (display.to_string(), String::new())
    }
}

/// Windows 数据使用量返回的路径是 NT 格式（\device\harddiskvolume3\...），
/// 不是 Win32 能直接使用的 C:\... 路径，这里用 QueryDosDevice 把盘符映射出来。
fn nt_to_drive_path(nt_path: &str) -> Option<String> {
    let normalized = nt_path.replace('/', "\\");
    let lower = normalized.to_lowercase();
    if !lower.starts_with("\\device\\") {
        return None;
    }
    for letter in b'a'..=b'z' {
        let mut drive = [0u16; 3];
        drive[0] = letter as u16;
        drive[1] = b':' as u16;
        let mut target = [0u16; 512];
        let len = unsafe {
            windows::Win32::Storage::FileSystem::QueryDosDeviceW(
                windows::core::PCWSTR(drive.as_ptr()),
                Some(&mut target[..]),
            )
        };
        if len == 0 {
            continue;
        }
        // QueryDosDeviceW 返回的长度可能包含结尾空字符，去掉后再做前缀匹配。
        let mut end = len as usize;
        while end > 0 && target[end - 1] == 0 {
            end -= 1;
        }
        let target_str = String::from_utf16_lossy(&target[..end]);
        if let Some(rest) = lower.strip_prefix(&target_str.to_lowercase()) {
            let drive_letter = (letter as char).to_ascii_uppercase();
            return Some(format!("{drive_letter}:{rest}"));
        }
    }
    None
}

/// 从完整路径提取 exe 文件名（去掉 .exe 扩展名，大小写不敏感）。
fn exe_stem(path: &str) -> Option<String> {
    let file = path.rsplit(['\\', '/']).next().filter(|s| !s.is_empty())?;
    let lower = file.to_lowercase();
    if lower.ends_with(".exe") {
        Some(file[..file.len() - 4].to_string())
    } else {
        Some(file.to_string())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AttributedUsageResult {
    /// 查询是否可用（系统不支持 / 无网络连接配置时为 false）。
    pub available: bool,
    pub apps: Vec<AttributedAppUsage>,
    /// 非致命说明（如空数据原因），成功且正常时为 None。
    pub message: Option<String>,
    /// 查询窗口（本地时间，YYYY-MM-DD HH:MM:SS）。
    pub since_local: String,
    pub until_local: String,
}

/// Unix 秒 → Windows FILETIME（1601-01-01 起 100ns 间隔）。
fn to_universal_time(unix_secs: i64) -> i64 {
    (unix_secs + 11_644_473_600) * 10_000_000
}

/// 构造「不可用」结果（供调用方在线程异常等场景兜底）。
pub fn unavailable(message: impl Into<String>) -> AttributedUsageResult {
    AttributedUsageResult {
        available: false,
        apps: Vec::new(),
        message: Some(message.into()),
        since_local: String::new(),
        until_local: String::new(),
    }
}

/// 查询最近 `days` 天（含今天）的按应用流量。`days` 会被限制在 1..=365。
pub fn query(days: u64) -> AttributedUsageResult {
    let now = chrono::Local::now();
    let start_date = now.date_naive() - chrono::Days::new(days.clamp(1, 365) - 1);
    let start_naive = match start_date.and_hms_opt(0, 0, 0) {
        Some(t) => t,
        None => {
            return unavailable("本地时间计算异常");
        }
    };
    let start_local = match start_naive.and_local_timezone(chrono::Local).earliest() {
        Some(t) => t,
        None => {
            return unavailable("本地时区换算异常");
        }
    };
    let since_local = start_local.format("%Y-%m-%d %H:%M:%S").to_string();
    let until_local = now.format("%Y-%m-%d %H:%M:%S").to_string();

    // WinRT 异步接口需要所在线程处于已初始化 COM 状态（MTA 即可）。
    let hr = unsafe {
        windows_sys::Win32::System::Com::CoInitializeEx(
            std::ptr::null_mut(),
            windows_sys::Win32::System::Com::COINIT_MULTITHREADED as u32,
        )
    };
    let initialized = hr == 0 || hr == 1; // S_OK / S_FALSE
    let result = query_inner(
        start_local.timestamp(),
        now.timestamp(),
        since_local,
        until_local,
    );
    if initialized {
        unsafe {
            windows_sys::Win32::System::Com::CoUninitialize();
        }
    }
    result
}

fn query_inner(
    start_ts: i64,
    end_ts: i64,
    since_local: String,
    until_local: String,
) -> AttributedUsageResult {
    use windows::Foundation::DateTime;
    use windows::Networking::Connectivity::{NetworkInformation, NetworkUsageStates, TriStates};

    let profile = match NetworkInformation::GetInternetConnectionProfile() {
        Ok(p) => p,
        Err(e) => {
            return unavailable(format!("无法获取网络连接配置（可能当前离线）：{e}"));
        }
    };

    let states = NetworkUsageStates {
        Roaming: TriStates::DoNotCare,
        Shared: TriStates::DoNotCare,
    };
    let start = DateTime {
        UniversalTime: to_universal_time(start_ts),
    };
    let end = DateTime {
        UniversalTime: to_universal_time(end_ts),
    };

    let op = match profile.GetAttributedNetworkUsageAsync(start, end, states) {
        Ok(op) => op,
        Err(e) => {
            return unavailable(format!("获取按应用流量失败：{e}"));
        }
    };
    let list = match op.get() {
        Ok(l) => l,
        Err(e) => {
            return unavailable(format!("等待按应用流量结果失败：{e}"));
        }
    };
    let size = match list.Size() {
        Ok(s) => s as usize,
        Err(e) => {
            return unavailable(format!("读取按应用流量列表失败：{e}"));
        }
    };

    let mut merged: HashMap<String, AttributedAppUsage> = HashMap::new();
    for i in 0..size {
        let item = match list.GetAt(i as u32) {
            Ok(v) => v,
            Err(e) => {
                return unavailable(format!("读取第 {i} 项应用流量失败：{e}"));
            }
        };
        let raw_name = item
            .AttributionName()
            .map(|s| s.to_string())
            .unwrap_or_default();
        let raw_id = item
            .AttributionId()
            .map(|s| s.to_string())
            .unwrap_or_default();
        let (app_name, exe_path) = normalize_app(&raw_name, &raw_id);
        let download = item.BytesReceived().unwrap_or(0);
        let upload = item.BytesSent().unwrap_or(0);

        let entry = merged
            .entry(raw_id.clone())
            .or_insert_with(|| AttributedAppUsage {
                app_id: raw_id,
                app_name: app_name.clone(),
                exe_path: exe_path.clone(),
                download_bytes: 0,
                upload_bytes: 0,
                total_bytes: 0,
            });
        entry.download_bytes += download;
        entry.upload_bytes += upload;
        entry.total_bytes += download + upload;
        if entry.app_name.is_empty() {
            entry.app_name = app_name;
        }
        if entry.exe_path.is_empty() {
            entry.exe_path = exe_path;
        }
    }

    let mut apps: Vec<AttributedAppUsage> = merged.into_values().collect();
    // 兜底：Windows 未给出可提取图标的路径时，从正在运行的进程里按 exe 名补路径。
    if apps.iter().any(|a| a.exe_path.is_empty()) {
        let mut sys = System::new();
        sys.refresh_processes(ProcessesToUpdate::All, true);
        let mut by_name: HashMap<String, String> = HashMap::new();
        for p in sys.processes().values() {
            if let Some(exe) = p.exe() {
                if let Some(stem) = exe.file_stem().map(|s| s.to_string_lossy().to_lowercase()) {
                    by_name
                        .entry(stem)
                        .or_insert_with(|| exe.to_string_lossy().to_string());
                }
            }
        }
        for app in &mut apps {
            let nt_like =
                app.exe_path.starts_with("\\device\\") || app.exe_path.starts_with("\\??\\");
            if app.exe_path.is_empty() || nt_like {
                if let Some(path) = by_name.get(&app.app_name.to_lowercase()) {
                    app.exe_path = path.clone();
                }
            }
        }
    }
    apps.sort_by_key(|a| std::cmp::Reverse(a.total_bytes));
    for a in apps.iter().take(6) {
        tracing::info!(
            "attributed app: name={} exe={} down={} up={}",
            a.app_name,
            a.exe_path,
            a.download_bytes,
            a.upload_bytes
        );
    }
    let message = if apps.is_empty() {
        Some("该时间段内 Windows 尚未记录任何应用流量。".to_string())
    } else {
        None
    };

    AttributedUsageResult {
        available: true,
        apps,
        message,
        since_local,
        until_local,
    }
}
