//! 硬件监控：CPU / 内存 / 磁盘快照（基于 sysinfo，跨平台）。

use sysinfo::{Disks, System};

/// 单个磁盘分区快照。
#[derive(Debug, Clone)]
pub struct DiskSnapshot {
    pub drive: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
}

/// 硬件整体快照。
#[derive(Debug, Clone)]
pub struct HardwareSnapshot {
    pub cpu_percent: f64,
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
    pub disks: Vec<DiskSnapshot>,
}

/// 硬件监控器：持有 sysinfo 状态，多次调用 snapshot 得到平滑的 CPU 使用率。
pub struct HardwareMonitor {
    sys: System,
    disks: Disks,
    cpu_ready: bool,
}

impl HardwareMonitor {
    pub fn new() -> Self {
        let mut sys = System::new();
        // 首次采样用于建立基线，第二次调用才返回真实 CPU 使用率。
        sys.refresh_cpu_usage();
        let disks = Disks::new_with_refreshed_list();
        Self {
            sys,
            disks,
            cpu_ready: false,
        }
    }

    pub fn snapshot(&mut self) -> HardwareSnapshot {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.disks.refresh(false);

        let cpu_percent = if self.cpu_ready {
            self.sys.global_cpu_usage() as f64
        } else {
            self.cpu_ready = true;
            0.0
        };

        let disks = self
            .disks
            .iter()
            .filter(|d| d.total_space() > 0)
            .map(|d| DiskSnapshot {
                drive: d
                    .name()
                    .to_string_lossy()
                    .to_string()
                    .chars()
                    .take(16)
                    .collect(),
                total_bytes: d.total_space(),
                available_bytes: d.available_space(),
            })
            .collect();

        HardwareSnapshot {
            cpu_percent: (cpu_percent * 10.0).round() / 10.0,
            memory_total_bytes: self.sys.total_memory(),
            memory_used_bytes: self.sys.used_memory(),
            disks,
        }
    }
}

impl Default for HardwareMonitor {
    fn default() -> Self {
        Self::new()
    }
}
