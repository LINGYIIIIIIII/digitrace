//! 自 api.rs 按域拆分（纯搬迁，行为不变）。
use super::*;
impl TimeTraceApi {
    /// 网络实时快照。
    pub fn get_network_snapshot(&self) -> NetworkSnapshotDto {
        let s = self.monitor_core.network_snapshot();
        NetworkSnapshotDto {
            upload_bytes_per_sec: s.upload_bytes_per_sec,
            download_bytes_per_sec: s.download_bytes_per_sec,
            session_upload_bytes: s.session_upload_bytes,
            session_download_bytes: s.session_download_bytes,
            adapter_count: s.adapters.len() as i64,
        }
    }

    /// 实时曲线窗口：最近 `seconds` 秒的秒级样本（缺省用配置窗口，默认 5 分钟）。
    pub fn get_network_live_window(&self, seconds: Option<u64>) -> Vec<NetSampleDto> {
        self.monitor_core
            .live_network_window(seconds)
            .into_iter()
            .map(|s| NetSampleDto {
                ts: s.ts,
                down: s.down_bps,
                up: s.up_bps,
            })
            .collect()
    }

    /// 按应用网络快照（字节模式或连接模式，自动探测）。
    pub fn get_net_apps(&self) -> NetAppsSnapshotDto {
        let mut monitor = self.net_apps.lock().unwrap_or_else(|p| p.into_inner());
        let snap = monitor.snapshot();

        // ETW 内核网络事件：管理员下提供实时按应用流量（合计，不分方向）。
        // 在 ESTATS 不可用的机器上作为字节来源。
        if let Some(rates) = self.monitor_core.etw_rates() {
            let session = self.monitor_core.etw_session_bytes().unwrap_or_default();
            let mut sys_guard = self.etw_sys.lock().unwrap_or_else(|p| p.into_inner());
            let sys = sys_guard.get_or_insert_with(sysinfo::System::new);
            sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
            let mut apps: Vec<NetAppUsageDto> = Vec::new();
            for (pid, rate) in rates {
                let mut app_name = String::new();
                let mut exe_path = String::new();
                if let Some(p) = sys.process(sysinfo::Pid::from_u32(pid)) {
                    app_name = p.name().to_string_lossy().to_string();
                    exe_path = p
                        .exe()
                        .map(|e| e.to_string_lossy().to_string())
                        .unwrap_or_default();
                }
                if app_name.is_empty() {
                    continue;
                }
                // 补充连接数（按应用名匹配连接模式的结果）。
                let (active, total) = snap
                    .apps
                    .iter()
                    .find(|a| a.app_name == app_name)
                    .map(|a| (a.active_connections, a.total_connections))
                    .unwrap_or((0, 0));
                apps.push(NetAppUsageDto {
                    app_name,
                    exe_path,
                    download_bps: rate,
                    upload_bps: 0.0,
                    session_download: session.get(&pid).copied().unwrap_or(0),
                    session_upload: 0,
                    active_connections: active,
                    total_connections: total,
                });
            }
            apps.sort_by(|a, b| {
                b.download_bps
                    .partial_cmp(&a.download_bps)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            return NetAppsSnapshotDto {
                bytes_available: true,
                etw_mode: true,
                apps,
            };
        }

        NetAppsSnapshotDto {
            bytes_available: snap.bytes_available,
            etw_mode: false,
            apps: snap
                .apps
                .into_iter()
                .map(|u| NetAppUsageDto {
                    app_name: u.app_name,
                    exe_path: u.exe_path,
                    download_bps: u.download_bps,
                    upload_bps: u.upload_bps,
                    session_download: u.session_download,
                    session_upload: u.session_upload,
                    active_connections: u.active_connections,
                    total_connections: u.total_connections,
                })
                .collect(),
        }
    }

    /// 网络历史曲线（最近 N 天，分钟级）。
    pub fn get_network_history(&self, mode: &str) -> Vec<HistoryPointDto> {
        self.monitor_core
            .network_history(mode)
            .into_iter()
            .map(|s| HistoryPointDto {
                day: s.day,
                minute: s.minute as i64,
                avg: s.avg,
                max: s.max,
            })
            .collect()
    }

    /// 上传方向历史（分钟级 avg/max）。
    pub fn get_network_history_up(&self, mode: &str) -> Vec<HistoryPointDto> {
        self.monitor_core
            .network_history_up(mode)
            .into_iter()
            .map(|s| HistoryPointDto {
                day: s.day,
                minute: s.minute as i64,
                avg: s.avg,
                max: s.max,
            })
            .collect()
    }
}
