//! 每进程网络监控。
//!
//! 两种模式：
//! - 字节模式：GetExtendedTcpTable + Set/GetPerTcpConnectionEStats 读取每个
//!   TCP 连接的真实收发字节（两次采样差值得到速率）。该 API 需要管理员权限，
//!   非管理员调用 SetPerTcpConnectionEStats 会返回 ERROR_ACCESS_DENIED。
//! - 连接模式：免管理员，统计每个进程的 TCP 连接数（活跃连接 / 会话累计连接）。
//!   启动时探测一次：能开启采集就用字节模式，否则自动降级为连接模式。

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, MIB_TCPROW_LH, MIB_TCPROW_LH_0, MIB_TCPROW_OWNER_PID,
    TCP_TABLE_OWNER_PID_ALL,
};

const AF_INET: u32 = 2;
const ERROR_SUCCESS: u32 = 0;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const ERROR_ACCESS_DENIED: u32 = 5;
/// TCP_ESTATS_TYPE 枚举：TcpConnectionEstatsData = 0（windows crate 0.61 误定义为 1）。
const TCP_ESTATS_DATA: u32 = 0;
/// MIB_TCP_STATE_ESTAB，TCP 连接处于已建立状态。
const TCP_STATE_ESTAB: u32 = 5;

// iphlpapi.dll 原生绑定：windows crate 0.61 的 Set/GetPerTcpConnectionEStats
// 绑定缺少 Offset 参数（参数错位导致 RwSize=0 → ERROR_INVALID_USER_BUFFER），
// 这里按官方头文件签名直接调用。
#[link(name = "iphlpapi")]
unsafe extern "system" {
    #[link_name = "SetPerTcpConnectionEStats"]
    fn SetPerTcpConnectionEStatsRaw(
        row: *mut MIB_TCPROW_LH,
        estatstype: u32,
        rw: *const u8,
        rwversion: u32,
        offset: u32,
        rwsize: u32,
    ) -> u32;
    #[link_name = "GetPerTcpConnectionEStats"]
    fn GetPerTcpConnectionEStatsRaw(
        row: *mut MIB_TCPROW_LH,
        estatstype: u32,
        rw: *mut u8,
        rwversion: u32,
        rwoffset: u32,
        rwsize: u32,
        ros: *mut u8,
        rosversion: u32,
        rosoffset: u32,
        rossize: u32,
        rod: *mut u8,
        rodversion: u32,
        rodoffset: u32,
        rodsize: u32,
    ) -> u32;
}

/// 单个应用的网络用量快照。
#[derive(Debug, Clone)]
pub struct NetAppUsage {
    pub app_name: String,
    pub exe_path: String,
    /// 下载速率（字节模式；连接模式下为 0）。
    pub download_bps: f64,
    /// 上传速率（字节模式；连接模式下为 0）。
    pub upload_bps: f64,
    /// 会话累计下载（字节模式；连接模式下为 0）。
    pub session_download: u64,
    /// 会话累计上传（字节模式；连接模式下为 0）。
    pub session_upload: u64,
    /// 当前活跃（ESTABLISHED）TCP 连接数，两种模式都有。
    pub active_connections: u32,
    /// 会话内累计见过的连接数（去重），两种模式都有。
    pub total_connections: u64,
}

/// 一次完整快照：是否处于字节模式 + 应用列表。
#[derive(Debug, Clone)]
pub struct NetAppSnapshot {
    pub bytes_available: bool,
    pub apps: Vec<NetAppUsage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ConnKey {
    local_addr: u32,
    local_port: u16,
    remote_addr: u32,
    remote_port: u16,
}

/// 读取单个连接字节数的结果。
enum ConnStat {
    Ok(u64, u64),
    /// 权限不足（整体不可用）。
    Denied,
    /// 该连接读取失败（单个跳过）。phase=0 表示 Set 阶段、1 表示 Get 阶段，附错误码。
    Failed {
        phase: u8,
        code: u32,
    },
}

/// 每进程网络监控器：持有上一采样点、会话累计与模式状态。
pub struct NetAppMonitor {
    prev: HashMap<ConnKey, (u64, u64)>, // key -> (bytes_in, bytes_out)，字节模式用
    session: HashMap<u32, (u64, u64)>,  // pid -> (download, upload)，字节模式用
    seen: HashMap<u32, HashSet<ConnKey>>, // pid -> 已见过的连接（去重）
    conn_totals: HashMap<u32, u64>,     // pid -> 会话累计连接数（去重）
    enabled: HashSet<ConnKey>,          // 已开启统计采集的连接
    probed: bool,                       // 是否已探测权限
    bytes_available: bool,              // 是否处于字节模式
    last_sample: Option<Instant>,
    /// 复用的进程表：避免每次快照都重建 sysinfo::System（减少内存抖动）。
    sys: sysinfo::System,
}

impl NetAppMonitor {
    pub fn new() -> Self {
        Self {
            prev: HashMap::new(),
            session: HashMap::new(),
            seen: HashMap::new(),
            conn_totals: HashMap::new(),
            enabled: HashSet::new(),
            probed: false,
            bytes_available: false,
            last_sample: None,
            sys: sysinfo::System::new(),
        }
    }

    /// 探测能否开启字节采集（SetPerTcpConnectionEStats 需要管理员）。
    fn probe_bytes_support(&mut self) {
        self.probed = true;
        let rows = get_tcp_rows();
        let estab: Vec<&MIB_TCPROW_OWNER_PID> = rows
            .iter()
            .filter(|r| r.dwState == TCP_STATE_ESTAB)
            .collect();
        let mut ok = 0;
        let mut denied = 0;
        let mut failed = 0;
        let mut not_supported = 0;
        let mut first_err: Option<u32> = None;
        let mut first_phase: Option<u8> = None;
        for row in &estab {
            match get_conn_bytes(row, &mut self.enabled) {
                ConnStat::Ok(..) => ok += 1,
                ConnStat::Denied => denied += 1,
                ConnStat::Failed { phase, code } => {
                    failed += 1;
                    if code == 50 {
                        not_supported += 1;
                    }
                    if first_err.is_none() {
                        first_err = Some(code);
                        first_phase = Some(phase);
                    }
                }
            }
        }
        // 只要有一个连接能读到字节就启用字节模式：个别连接被拒/失败不拖累整体。
        self.bytes_available = ok > 0;
        crate::oplog::log_event(
            "NETAPP",
            &format!(
                "字节模式探测: elevated={} available={} estab={} ok={} denied={} failed={} unsupported={} phase={:?} err={:?}",
                crate::is_elevated(),
                self.bytes_available,
                estab.len(),
                ok,
                denied,
                failed,
                not_supported,
                first_phase,
                first_err,
            ),
        );
    }

    /// 采样一次：返回 (pid, download_bps, upload_bps, active_connections)。
    fn sample(&mut self) -> Vec<(u32, f64, f64, u32)> {
        let now = Instant::now();
        let dt = self
            .last_sample
            .map(|t| (now - t).as_secs_f64())
            .unwrap_or(1.0);
        self.last_sample = Some(now);

        let rows = get_tcp_rows();
        let mut delta: HashMap<u32, (i64, i64)> = HashMap::new();
        let mut active: HashMap<u32, u32> = HashMap::new();
        let mut current: HashMap<ConnKey, (u64, u64)> = HashMap::new();

        for row in rows {
            let pid = row.dwOwningPid;
            let key = conn_key(&row);

            // 连接累计（去重）：首次见到该连接才 +1。
            if self.seen.entry(pid).or_default().insert(key) {
                *self.conn_totals.entry(pid).or_insert(0) += 1;
            }

            if row.dwState == TCP_STATE_ESTAB {
                *active.entry(pid).or_insert(0) += 1;
            }

            if self.bytes_available
                && let ConnStat::Ok(b_in, b_out) = get_conn_bytes(&row, &mut self.enabled)
            {
                let bytes = (b_in, b_out);
                current.insert(key, bytes);
                if let Some(prev) = self.prev.get(&key) {
                    let d_in = bytes.0.saturating_sub(prev.0);
                    let d_out = bytes.1.saturating_sub(prev.1);
                    let e = delta.entry(pid).or_insert((0, 0));
                    e.0 += d_in as i64;
                    e.1 += d_out as i64;
                }
            }
        }
        self.prev = current;

        for (pid, (d_in, d_out)) in &delta {
            let s = self.session.entry(*pid).or_insert((0, 0));
            s.0 = s.0.saturating_add((*d_in).max(0) as u64);
            s.1 = s.1.saturating_add((*d_out).max(0) as u64);
        }

        // 返回所有本采样出现过的进程（含无字节增量但存在连接的）。
        let mut pids: HashSet<u32> = HashSet::new();
        pids.extend(delta.keys().copied());
        pids.extend(active.keys().copied());
        pids.into_iter()
            .map(|pid| {
                let (d_in, d_out) = delta.get(&pid).copied().unwrap_or((0, 0));
                (
                    pid,
                    d_in as f64 / dt,
                    d_out as f64 / dt,
                    active.get(&pid).copied().unwrap_or(0),
                )
            })
            .collect()
    }

    /// 快照：每应用连接数（+ 字节模式下的速率/累计），按活跃连接降序。
    pub fn snapshot(&mut self) -> NetAppSnapshot {
        if !self.probed {
            self.probe_bytes_support();
        }
        let rates = self.sample();
        // 复用进程表，仅刷新需要的字段（exe 路径；进程名始终刷新），
        // 避免每 2 秒重建 System。
        self.sys.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            true,
            sysinfo::ProcessRefreshKind::nothing()
                .with_exe(sysinfo::UpdateKind::OnlyIfNotSet),
        );
        let sys = &self.sys;

        let mut merged: HashMap<u32, (f64, f64, u32)> = HashMap::new();
        for (pid, down, up, active) in rates {
            merged.insert(pid, (down, up, active));
        }
        // 保留本会话有连接记录的应用（速率/活跃可能为 0）。
        for pid in self.conn_totals.keys() {
            merged.entry(*pid).or_insert((0.0, 0.0, 0));
        }

        // 清理已退出的进程，避免累计表无限增长。
        let alive = |pid: u32| sys.process(sysinfo::Pid::from_u32(pid)).is_some();
        self.conn_totals.retain(|pid, _| alive(*pid));
        self.session.retain(|pid, _| alive(*pid));
        self.seen.retain(|pid, _| alive(*pid));

        let per_pid: Vec<NetAppUsage> = merged
            .into_iter()
            .filter_map(|(pid, (down, up, active))| {
                let proc = sys.process(sysinfo::Pid::from_u32(pid))?;
                let (sd, su) = self.session.get(&pid).copied().unwrap_or((0, 0));
                let total = self.conn_totals.get(&pid).copied().unwrap_or(0);
                Some(NetAppUsage {
                    app_name: proc.name().to_string_lossy().to_string(),
                    exe_path: proc
                        .exe()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    download_bps: down,
                    upload_bps: up,
                    session_download: sd,
                    session_upload: su,
                    active_connections: active,
                    total_connections: total,
                })
            })
            .collect();

        // 聚合同一应用的多进程（按 exe 路径；无路径时按名称），展示"按应用分布"。
        let mut by_app: std::collections::BTreeMap<String, NetAppUsage> =
            std::collections::BTreeMap::new();
        for u in per_pid {
            let key = if u.exe_path.is_empty() {
                u.app_name.clone()
            } else {
                u.exe_path.clone()
            };
            let e = by_app.entry(key).or_insert_with(|| NetAppUsage {
                app_name: u.app_name.clone(),
                exe_path: u.exe_path.clone(),
                download_bps: 0.0,
                upload_bps: 0.0,
                session_download: 0,
                session_upload: 0,
                active_connections: 0,
                total_connections: 0,
            });
            e.download_bps += u.download_bps;
            e.upload_bps += u.upload_bps;
            e.session_download = e.session_download.saturating_add(u.session_download);
            e.session_upload = e.session_upload.saturating_add(u.session_upload);
            e.active_connections += u.active_connections;
            e.total_connections += u.total_connections;
        }
        let mut out: Vec<NetAppUsage> = by_app.into_values().collect();

        // 活跃连接多的排前面；字节模式下按总速率排。
        out.sort_by(|a, b| {
            if self.bytes_available {
                (b.download_bps + b.upload_bps)
                    .partial_cmp(&(a.download_bps + a.upload_bps))
                    .unwrap_or(Ordering::Equal)
            } else {
                b.active_connections
                    .cmp(&a.active_connections)
                    .then(b.total_connections.cmp(&a.total_connections))
            }
        });

        NetAppSnapshot {
            bytes_available: self.bytes_available,
            apps: out,
        }
    }
}

impl Default for NetAppMonitor {
    fn default() -> Self {
        Self::new()
    }
}

fn conn_key(row: &MIB_TCPROW_OWNER_PID) -> ConnKey {
    ConnKey {
        local_addr: row.dwLocalAddr,
        local_port: row.dwLocalPort as u16,
        remote_addr: row.dwRemoteAddr,
        remote_port: row.dwRemotePort as u16,
    }
}

fn get_tcp_rows() -> Vec<MIB_TCPROW_OWNER_PID> {
    unsafe {
        let mut buf: Vec<u8> = Vec::new();
        let mut size = 0u32;
        loop {
            let ret = GetExtendedTcpTable(
                if buf.is_empty() {
                    None
                } else {
                    Some(buf.as_mut_ptr() as *mut _)
                },
                &mut size,
                false,
                AF_INET,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            );
            if ret == ERROR_SUCCESS {
                break;
            }
            // 第一次调用（NULL 缓冲）预期返回 ERROR_INSUFFICIENT_BUFFER 并给出所需大小。
            if ret != ERROR_INSUFFICIENT_BUFFER || size == 0 {
                return Vec::new();
            }
            buf.resize(size as usize, 0);
        }
        if size < 4 {
            return Vec::new();
        }
        let num = *(buf.as_ptr() as *const u32);
        let base = buf.as_ptr() as usize;
        let step = std::mem::size_of::<MIB_TCPROW_OWNER_PID>();
        let mut rows = Vec::with_capacity(num as usize);
        for i in 0..num as usize {
            let ptr = (base + 4 + i * step) as *const MIB_TCPROW_OWNER_PID;
            rows.push(ptr.read());
        }
        rows
    }
}

/// 读取某个 TCP 连接的真实收发字节；新连接先开启统计采集。
#[allow(clippy::field_reassign_with_default)] // 先清零 union padding 再填字段（见下方注释）
fn get_conn_bytes(row: &MIB_TCPROW_OWNER_PID, enabled: &mut HashSet<ConnKey>) -> ConnStat {
    unsafe {
        // 先清零再填字段：MIB_TCPROW_LH 的 union 带 padding，未初始化内存可能
        // 导致 Set/GetPerTcpConnectionEStats 返回 ERROR_INVALID_USER_BUFFER。
        let mut lh: MIB_TCPROW_LH = Default::default();
        lh.Anonymous = MIB_TCPROW_LH_0 {
            dwState: row.dwState,
        };
        lh.dwLocalAddr = row.dwLocalAddr;
        lh.dwLocalPort = row.dwLocalPort;
        lh.dwRemoteAddr = row.dwRemoteAddr;
        lh.dwRemotePort = row.dwRemotePort;
        let key = conn_key(row);

        // 按文档要求：读取数据前先用 SetPerTcpConnectionEStats 开启采集。
        if !enabled.contains(&key) {
            // 注意：windows crate 0.61 的 TCP_ESTATS_DATA_RW_v0 只生成了 1 个字段
            // （缺 EnableCollectionOnProtocolEvent），直接使用会让 rw 缓冲区只有 1 字节、
            // Set 返回 ERROR_INVALID_PARAMETER。Windows 要求 v0 的 RW 结构为 2 字节：
            // EnableCollection=1 + EnableCollectionOnProtocolEvent=0。
            let rw_bytes = [1u8, 0u8];
            let ret = SetPerTcpConnectionEStatsRaw(
                &mut lh,
                TCP_ESTATS_DATA,
                rw_bytes.as_ptr(),
                0,
                0,
                rw_bytes.len() as u32,
            );
            if ret == ERROR_ACCESS_DENIED {
                return ConnStat::Denied;
            }
            if ret != ERROR_SUCCESS {
                return ConnStat::Failed {
                    phase: 0,
                    code: ret,
                };
            }
            enabled.insert(key);
        }

        // Get 的 rw 缓冲区同样必须匹配 rwversion=0（2 字节）。
        // rod 用固定的大缓冲区：windows crate 0.61 的 TCP_ESTATS_DATA_ROD_v0 被截断
        // （约 88 字节），Windows 要求完整结构（约 256 字节），否则返回
        // ERROR_INVALID_USER_BUFFER (1784)。
        let mut rw = [0u8; 2];
        // 足够大的固定缓冲区（TCP_ESTATS_DATA_ROD_v0 约 240+ 字节，给足余量）。
        let mut rod = [0u8; 1024];
        let ret = GetPerTcpConnectionEStatsRaw(
            &mut lh,
            TCP_ESTATS_DATA,
            rw.as_mut_ptr(),
            0,
            0,
            rw.len() as u32,
            std::ptr::null_mut(),
            0,
            0,
            0,
            rod.as_mut_ptr(),
            0,
            0,
            rod.len() as u32,
        );
        if ret == ERROR_ACCESS_DENIED {
            return ConnStat::Denied;
        }
        if ret != ERROR_SUCCESS {
            return ConnStat::Failed {
                phase: 1,
                code: ret,
            };
        }
        // C 布局：DataBytesOut 偏移 0，DataSegsOut 偏移 8，DataBytesIn 偏移 16。
        let data_bytes_out = u64::from_le_bytes(rod[0..8].try_into().unwrap());
        let data_bytes_in = u64::from_le_bytes(rod[16..24].try_into().unwrap());
        ConnStat::Ok(data_bytes_in, data_bytes_out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn tcp_rows_are_readable() {
        let rows = get_tcp_rows();
        println!("tcp rows: {}", rows.len());
        assert!(!rows.is_empty(), "本机应存在 TCP 连接");
    }

    #[test]
    fn snapshot_counts_connections_without_bytes() {
        let mut m = NetAppMonitor::new();
        let first = m.snapshot();
        println!(
            "first snapshot: bytes={} entries={}",
            first.bytes_available,
            first.apps.len()
        );
        std::thread::sleep(Duration::from_millis(300));
        let second = m.snapshot();
        println!("second snapshot: entries={}", second.apps.len());
        for e in second.apps.iter().take(10) {
            println!(
                "  {}  活跃={} 累计={}  ↓{:.1}B/s ↑{:.1}B/s",
                e.app_name, e.active_connections, e.total_connections, e.download_bps, e.upload_bps
            );
        }
        assert!(
            second.apps.iter().any(|a| a.total_connections > 0),
            "应至少统计到一个应用的连接数"
        );
    }
}
