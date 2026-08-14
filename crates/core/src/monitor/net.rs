//! 网络总量采集（免管理员）：GetIfTable 差值 → 字节/秒。
//!
//! 同一物理网卡在表里可能出现多个镜像行（WFP/QoS/过滤层），计数相同，
//! 直接求和会放大速率。因此用 GetAdaptersInfo 拿物理网卡描述，按
//! "最短包含匹配"选出行，保证每个物理网卡只计一次。

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use windows::Win32::NetworkManagement::IpHelper::{
    GetAdaptersInfo, GetIfTable, IF_OPER_STATUS_CONNECTED, IF_OPER_STATUS_OPERATIONAL, IF_TYPE_PPP,
    IF_TYPE_SLIP, IF_TYPE_SOFTWARE_LOOPBACK, IF_TYPE_TUNNEL, IP_ADAPTER_INFO, MIB_IFROW,
    MIB_IFTABLE,
};

const ERROR_SUCCESS: u32 = 0;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const ERROR_BUFFER_OVERFLOW: u32 = 111;

/// 一块网卡在某个采样时刻的累计字节计数。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CounterPair {
    pub in_bytes: u64,
    pub out_bytes: u64,
}

/// 单块网卡的当前信息。
#[derive(Debug, Clone, Default)]
pub struct AdapterInfo {
    pub index: u32,
    pub description: String,
    pub is_up: bool,
    pub is_virtual: bool,
    pub link_speed_bps: u64,
}

/// 采样线程每周期发布的不可变快照。
#[derive(Debug, Clone, Default)]
pub struct NetworkSnapshot {
    pub version: u64,
    pub timestamp_ms: u64,
    pub upload_bytes_per_sec: u64,
    pub download_bytes_per_sec: u64,
    pub session_upload_bytes: u64,
    pub session_download_bytes: u64,
    pub adapters: Vec<AdapterInfo>,
    pub selected_indices: Vec<u32>,
}

/// 计算某块网卡本次采样的增量；计数回退（重连/清零）时返回 None。
pub(crate) fn compute_delta(prev: Option<CounterPair>, cur: CounterPair) -> Option<(u64, u64)> {
    match prev {
        Some(p) if cur.in_bytes >= p.in_bytes && cur.out_bytes >= p.out_bytes => {
            Some((cur.in_bytes - p.in_bytes, cur.out_bytes - p.out_bytes))
        }
        _ => None,
    }
}

/// 把增量换算成字节/秒。
pub(crate) fn speed_from_delta(delta: Option<(u64, u64)>, elapsed_ms: u64) -> (u64, u64) {
    match delta {
        Some((in_bytes, out_bytes)) if elapsed_ms > 0 => {
            (in_bytes * 1000 / elapsed_ms, out_bytes * 1000 / elapsed_ms)
        }
        _ => (0, 0),
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn row_desc(row: &MIB_IFROW) -> String {
    let bytes: Vec<u8> = row
        .bDescr
        .iter()
        .take_while(|&&b| b != 0)
        .copied()
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// 读取 GetIfTable 全部接口行（行数据从偏移 0 起，dwNumEntries 在表头）。
fn read_if_table() -> Result<Vec<MIB_IFROW>, u32> {
    unsafe {
        let mut table: Vec<MIB_IFTABLE> = Vec::with_capacity(32);
        let mut size = (table.capacity() * std::mem::size_of::<MIB_IFTABLE>()) as u32;
        loop {
            let ret = GetIfTable(Some(table.as_mut_ptr()), &mut size, false);
            if ret == ERROR_SUCCESS {
                let first = &*table.as_ptr();
                let count = first.dwNumEntries as usize;
                let rows = std::slice::from_raw_parts(first.table.as_ptr(), count);
                return Ok(rows.to_vec());
            }
            if ret != ERROR_INSUFFICIENT_BUFFER {
                return Err(ret);
            }
            let needed = size as usize;
            let elements = needed.div_ceil(std::mem::size_of::<MIB_IFTABLE>()).max(1);
            table.resize(elements, std::mem::zeroed());
        }
    }
}

/// 用 GetAdaptersInfo 枚举物理网卡描述（过滤层镜像行不在这里出现）。
fn read_adapter_descriptions() -> Vec<String> {
    unsafe {
        let mut size: u32 = 0;
        let mut ret = GetAdaptersInfo(None, &mut size);
        if ret != ERROR_BUFFER_OVERFLOW && ret != ERROR_SUCCESS {
            return Vec::new();
        }
        let elements = (size as usize)
            .div_ceil(std::mem::size_of::<IP_ADAPTER_INFO>())
            .max(1);
        let mut info: Vec<IP_ADAPTER_INFO> = vec![std::mem::zeroed(); elements];
        ret = GetAdaptersInfo(Some(info.as_mut_ptr()), &mut size);
        if ret != ERROR_SUCCESS {
            return Vec::new();
        }

        let mut out = Vec::new();
        let mut cur = info.as_ptr();
        while !cur.is_null() {
            let item = &*cur;
            let bytes: Vec<u8> = item
                .Description
                .iter()
                .take_while(|&&b| b != 0)
                .map(|&b| b as u8)
                .collect();
            let desc = String::from_utf8_lossy(&bytes).into_owned();
            if !out.contains(&desc) {
                out.push(desc);
            }
            cur = item.Next;
        }
        out
    }
}

/// 在接口行里为物理网卡描述找最佳匹配行（最短包含匹配，
/// 过滤层镜像行描述更长，天然被排除）。
fn find_best_row<'a>(rows: &'a [MIB_IFROW], desc: &str) -> Option<&'a MIB_IFROW> {
    rows.iter()
        .filter(|r| row_desc(r).contains(desc))
        .min_by_key(|r| row_desc(r).len())
}

fn is_virtual(row: &MIB_IFROW) -> bool {
    matches!(
        row.dwType,
        IF_TYPE_PPP | IF_TYPE_SOFTWARE_LOOPBACK | IF_TYPE_SLIP | IF_TYPE_TUNNEL
    )
}

fn is_up(row: &MIB_IFROW) -> bool {
    row.dwOperStatus == IF_OPER_STATUS_CONNECTED || row.dwOperStatus == IF_OPER_STATUS_OPERATIONAL
}

/// Windows 计数器轮询采集器。
pub struct WindowsCollector {
    prev: HashMap<u32, CounterPair>,
    session_in: u64,
    session_out: u64,
    last_poll_ms: Option<u64>,
    version: u64,
}

impl Default for WindowsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowsCollector {
    pub fn new() -> Self {
        Self {
            prev: HashMap::new(),
            session_in: 0,
            session_out: 0,
            last_poll_ms: None,
            version: 0,
        }
    }

    pub fn poll(&mut self) -> NetworkSnapshot {
        let now = now_ms();
        let elapsed = self
            .last_poll_ms
            .map(|t| now.saturating_sub(t))
            .unwrap_or(0);
        self.last_poll_ms = Some(now);

        let rows = read_if_table().unwrap_or_default();
        let physical_descs = read_adapter_descriptions();

        // 每个物理网卡只保留一个匹配行（去重镜像计数）。
        let mut adapters = Vec::new();
        let mut matched_index = Vec::new();
        for desc in &physical_descs {
            if let Some(row) = find_best_row(&rows, desc) {
                if matched_index.contains(&row.dwIndex) {
                    continue;
                }
                adapters.push(AdapterInfo {
                    index: row.dwIndex,
                    description: desc.clone(),
                    is_up: is_up(row),
                    is_virtual: is_virtual(row),
                    link_speed_bps: row.dwSpeed as u64,
                });
                matched_index.push(row.dwIndex);
            }
        }

        // 回退：GetAdaptersInfo 失败/描述不匹配时，退回原始行（过滤虚拟网卡）。
        if adapters.is_empty() {
            for row in &rows {
                if is_virtual(row) {
                    continue;
                }
                if matched_index.contains(&row.dwIndex) {
                    continue;
                }
                adapters.push(AdapterInfo {
                    index: row.dwIndex,
                    description: row_desc(row),
                    is_up: is_up(row),
                    is_virtual: false,
                    link_speed_bps: row.dwSpeed as u64,
                });
                matched_index.push(row.dwIndex);
            }
        }

        let selected: Vec<u32> = adapters
            .iter()
            .filter(|a| a.is_up && !a.is_virtual)
            .map(|a| a.index)
            .collect();

        let mut delta_in = 0u64;
        let mut delta_out = 0u64;
        for row in rows.iter().filter(|r| matched_index.contains(&r.dwIndex)) {
            if !selected.contains(&row.dwIndex) {
                continue;
            }
            let cur = CounterPair {
                in_bytes: row.dwInOctets as u64,
                out_bytes: row.dwOutOctets as u64,
            };
            let prev = self.prev.get(&row.dwIndex).copied();
            if let Some((d_in, d_out)) = compute_delta(prev, cur) {
                delta_in += d_in;
                delta_out += d_out;
            }
            self.prev.insert(row.dwIndex, cur);
        }
        self.prev.retain(|idx, _| matched_index.contains(idx));

        let (down, up) = speed_from_delta(Some((delta_in, delta_out)), elapsed);
        self.session_in += delta_in;
        self.session_out += delta_out;
        self.version += 1;

        NetworkSnapshot {
            version: self.version,
            timestamp_ms: now,
            upload_bytes_per_sec: up,
            download_bytes_per_sec: down,
            session_upload_bytes: self.session_out,
            session_download_bytes: self.session_in,
            adapters,
            selected_indices: selected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delta_and_speed() {
        assert_eq!(
            compute_delta(
                Some(CounterPair {
                    in_bytes: 100,
                    out_bytes: 50
                }),
                CounterPair {
                    in_bytes: 200,
                    out_bytes: 80
                }
            ),
            Some((100, 30))
        );
        // 计数回退 → None → 速率 0
        assert_eq!(
            compute_delta(
                Some(CounterPair {
                    in_bytes: 200,
                    out_bytes: 80
                }),
                CounterPair {
                    in_bytes: 100,
                    out_bytes: 90
                }
            ),
            None
        );
        assert_eq!(speed_from_delta(Some((1000, 500)), 1000), (1000, 500));
        assert_eq!(speed_from_delta(None, 1000), (0, 0));
        assert_eq!(speed_from_delta(Some((1000, 500)), 0), (0, 0));
    }
}
