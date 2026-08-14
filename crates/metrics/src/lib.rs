//! 共享内存实时指标（语言无关、零拷贝）。
//!
//! 发布方（`MetricsPublisher`）每秒把实时快照写入内存映射文件
//! `%APPDATA%\TimeTrace\metrics.map`；任何进程（`MetricsReader` 或直接用
//! `metrics.h` 的 C 结构体）都能以只读方式映射同一文件，纳秒级读取，零拷贝。
//!
//! 文件布局（固定 4096 字节）：
//! ```text
//! [ MetricsHeader (16B) ][ MetricsSnapshot (…B) ]
//! ```
//! 读者先校验 header 的 magic/version，再读快照；`seq` 单调递增用于检测更新。

#![cfg(windows)]

use std::path::PathBuf;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_ALWAYS,
};
use windows_sys::Win32::System::Memory::{
    CreateFileMappingW, FILE_MAP_READ, FILE_MAP_WRITE, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile,
    PAGE_READONLY, PAGE_READWRITE, UnmapViewOfFile,
};

/// 魔数："DMTC"（little-endian）。
pub const METRICS_MAGIC: u32 = 0x4354_4D44;
/// 结构体版本：结构变化时 +1，读者据此判断兼容性。
pub const METRICS_VERSION: u32 = 1;
/// 映射文件总大小（页大小）。
pub const METRICS_FILE_SIZE: usize = 4096;
const ACTIVE_APP_LEN: usize = 128;
// GENERIC_READ | GENERIC_WRITE（windows-sys 未导出独立常量，用 Windows 标准值）。
const GENERIC_RW: u32 = 0x8000_0000 | 0x4000_0000;

const fn header_size() -> usize {
    std::mem::size_of::<MetricsHeader>()
}

/// 文件头：固定 16 字节，写入一次。
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MetricsHeader {
    pub magic: u32,
    pub version: u32,
    pub snapshot_size: u32,
    pub reserved: u32,
}

/// 实时指标快照（`#[repr(C)]` 固定布局，可直接映射读取）。
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MetricsSnapshot {
    /// 快照序号（单调递增，读者检测是否更新）。
    pub seq: u64,
    /// 采样时刻（Unix 毫秒）。
    pub timestamp_ms: i64,
    /// CPU 总占用（0-100）。
    pub cpu_total_percent: f64,
    /// CPU 温度（℃，无传感器时 -1）。
    pub cpu_temp_c: f64,
    /// GPU 使用率（0-100，无 N 卡时 -1）。
    pub gpu_usage_percent: f64,
    /// GPU 温度（℃，无数据时 -1）。
    pub gpu_temp_c: f64,
    /// 内存已用（MB）。
    pub mem_used_mb: f64,
    /// 内存占用百分比（0-100）。
    pub mem_percent: f64,
    /// 下行速率（B/s）。
    pub net_down_bps: f64,
    /// 上行速率（B/s）。
    pub net_up_bps: f64,
    /// 帧率（预留：-1 表示未实现）。
    pub fps: f64,
    /// 当前前台应用名（UTF-8，0 填充）。
    pub active_app: [u8; ACTIVE_APP_LEN],
}

impl Default for MetricsSnapshot {
    fn default() -> Self {
        Self {
            seq: 0,
            timestamp_ms: 0,
            cpu_total_percent: 0.0,
            cpu_temp_c: -1.0,
            gpu_usage_percent: -1.0,
            gpu_temp_c: -1.0,
            mem_used_mb: 0.0,
            mem_percent: 0.0,
            net_down_bps: 0.0,
            net_up_bps: 0.0,
            fps: -1.0,
            active_app: [0u8; ACTIVE_APP_LEN],
        }
    }
}

impl MetricsSnapshot {
    /// 填充 active_app（UTF-8，超长截断，末尾补 0）。
    pub fn set_active_app(&mut self, name: &str) {
        let mut buf = [0u8; ACTIVE_APP_LEN];
        let bytes = name.as_bytes();
        let n = bytes.len().min(ACTIVE_APP_LEN - 1);
        buf[..n].copy_from_slice(&bytes[..n]);
        self.active_app = buf;
    }

    /// active_app 的 UTF-8 字符串（截到首个 0）。
    pub fn active_app_str(&self) -> &str {
        let end = self
            .active_app
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(ACTIVE_APP_LEN);
        std::str::from_utf8(&self.active_app[..end]).unwrap_or("")
    }
}

fn metrics_path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("TimeTrace")
        .join("metrics.map")
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 发布方：写映射文件（读/写）。
pub struct MetricsPublisher {
    _file: HANDLE,
    mapping: HANDLE,
    base: *mut u8,
    seq: u64,
}
// 句柄与指针按独占访问使用；跨线程移动发布方是安全的。
unsafe impl Send for MetricsPublisher {}

impl MetricsPublisher {
    pub fn open() -> Option<Self> {
        Self::open_at(&metrics_path())
    }

    fn open_at(path: &std::path::Path) -> Option<Self> {
        let wide = to_wide(&path.to_string_lossy());
        unsafe {
            let file = CreateFileW(
                wide.as_ptr(),
                GENERIC_RW,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_ALWAYS,
                FILE_ATTRIBUTE_NORMAL,
                std::ptr::null_mut(),
            );
            if file == INVALID_HANDLE_VALUE {
                return None;
            }
            let mapping = CreateFileMappingW(
                file,
                std::ptr::null(),
                PAGE_READWRITE,
                0,
                METRICS_FILE_SIZE as u32,
                std::ptr::null(),
            );
            if mapping.is_null() {
                CloseHandle(file);
                return None;
            }
            let view = MapViewOfFile(
                mapping,
                FILE_MAP_READ | FILE_MAP_WRITE,
                0,
                0,
                METRICS_FILE_SIZE,
            );
            if view.Value.is_null() {
                CloseHandle(mapping);
                CloseHandle(file);
                return None;
            }
            let base = view.Value as *mut u8;
            let header = MetricsHeader {
                magic: METRICS_MAGIC,
                version: METRICS_VERSION,
                snapshot_size: std::mem::size_of::<MetricsSnapshot>() as u32,
                reserved: 0,
            };
            std::ptr::copy_nonoverlapping(
                &header as *const MetricsHeader as *const u8,
                base,
                header_size(),
            );
            Some(Self {
                _file: file,
                mapping,
                base,
                seq: 0,
            })
        }
    }

    pub fn publish(&mut self, mut snap: MetricsSnapshot) {
        self.seq = self.seq.wrapping_add(1);
        snap.seq = self.seq;
        snap.timestamp_ms = now_ms();
        unsafe {
            let dst = self.base.add(header_size()) as *mut MetricsSnapshot;
            std::ptr::copy_nonoverlapping(&snap, dst, 1);
        }
    }
}

impl Drop for MetricsPublisher {
    fn drop(&mut self) {
        unsafe {
            if !self.base.is_null() {
                let _ = UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                    Value: self.base as *mut core::ffi::c_void,
                });
            }
            if !self.mapping.is_null() {
                CloseHandle(self.mapping);
            }
            if self._file != INVALID_HANDLE_VALUE {
                CloseHandle(self._file);
            }
        }
    }
}

/// 读取方：只读映射（供外部工具/其它语言参考；亦可直接用 C 结构体读文件）。
pub struct MetricsReader {
    _file: HANDLE,
    mapping: HANDLE,
    base: *mut u8,
}
unsafe impl Send for MetricsReader {}

impl MetricsReader {
    pub fn open() -> Option<Self> {
        Self::open_at(&metrics_path())
    }

    fn open_at(path: &std::path::Path) -> Option<Self> {
        let wide = to_wide(&path.to_string_lossy());
        unsafe {
            let file = CreateFileW(
                wide.as_ptr(),
                GENERIC_RW,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_ALWAYS,
                FILE_ATTRIBUTE_NORMAL,
                std::ptr::null_mut(),
            );
            if file == INVALID_HANDLE_VALUE {
                return None;
            }
            let mapping = CreateFileMappingW(
                file,
                std::ptr::null(),
                PAGE_READONLY,
                0,
                METRICS_FILE_SIZE as u32,
                std::ptr::null(),
            );
            if mapping.is_null() {
                CloseHandle(file);
                return None;
            }
            let view = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, METRICS_FILE_SIZE);
            if view.Value.is_null() {
                CloseHandle(mapping);
                CloseHandle(file);
                return None;
            }
            Some(Self {
                _file: file,
                mapping,
                base: view.Value as *mut u8,
            })
        }
    }

    /// 读取最新快照（未初始化或文件损坏时返回 None）。
    pub fn read(&self) -> Option<MetricsSnapshot> {
        unsafe {
            let header = &*(self.base as *const MetricsHeader);
            if header.magic != METRICS_MAGIC || header.version != METRICS_VERSION {
                return None;
            }
            Some(std::ptr::read(
                self.base.add(header_size()) as *const MetricsSnapshot
            ))
        }
    }
}

impl Drop for MetricsReader {
    fn drop(&mut self) {
        unsafe {
            if !self.base.is_null() {
                let _ = UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                    Value: self.base as *mut core::ffi::c_void,
                });
            }
            if !self.mapping.is_null() {
                CloseHandle(self.mapping);
            }
            if self._file != INVALID_HANDLE_VALUE {
                CloseHandle(self._file);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_read_roundtrip() {
        let dir =
            std::env::temp_dir().join(format!("digitrace_metrics_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("metrics.map");

        let mut pub_ = MetricsPublisher::open_at(&path).expect("open publisher");
        let mut snap = MetricsSnapshot {
            cpu_total_percent: 42.5,
            ..Default::default()
        };
        snap.net_down_bps = 1234.0;
        snap.set_active_app("Visual Studio Code");
        pub_.publish(snap);
        drop(pub_);

        let reader = MetricsReader::open_at(&path).expect("open reader");
        let got = reader.read().expect("read snapshot");
        assert_eq!(got.cpu_total_percent, 42.5);
        assert_eq!(got.net_down_bps, 1234.0);
        assert_eq!(got.active_app_str(), "Visual Studio Code");
        assert_eq!(got.fps, -1.0);
        drop(reader);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
