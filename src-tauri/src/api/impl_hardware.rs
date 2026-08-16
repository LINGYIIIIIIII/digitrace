//! 自 api.rs 按域拆分（纯搬迁，行为不变）。
use super::*;
use timetrace_core::*;
impl TimeTraceApi {
    /// 日历日仪表盘：某日历日（00:00–24:00，按配置时区）的硬件/温度/网络分钟级序列。
    pub fn get_day_metrics(&self, date: String) -> DayMetricsDto {
        let q = |metric: &str| {
            self.monitor_core
                .metric_day(metric, &date)
                .into_iter()
                .map(|(minute, avg)| DayMetricPointDto { minute, avg })
                .collect()
        };
        DayMetricsDto {
            cpu_percent: q("cpu_percent"),
            mem_percent: q("mem_percent"),
            cpu_temp_c: q("cpu_temp_c"),
            gpu_usage_percent: q("gpu_usage_percent"),
            gpu_temp_c: q("gpu_temp_c"),
            net_down_bps: q("net_down_bps"),
            net_up_bps: q("net_up_bps"),
        }
    }

    /// 硬件快照（CPU / 内存 / 磁盘）。
    pub fn get_hardware_snapshot(&self) -> HardwareSnapshotDto {
        let snap = self
            .hardware
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .snapshot();
        HardwareSnapshotDto {
            cpu_percent: snap.cpu_percent,
            memory_total_bytes: snap.memory_total_bytes,
            memory_used_bytes: snap.memory_used_bytes,
            disks: snap
                .disks
                .into_iter()
                .map(|d| DiskSnapshotDto {
                    drive: d.drive,
                    total_bytes: d.total_bytes,
                    available_bytes: d.available_bytes,
                })
                .collect(),
        }
    }

    /// 温度快照（CPU / GPU / 磁盘）。
    pub fn get_temperature_snapshot(&self) -> TemperatureSnapshotDto {
        let snap = self
            .temperature
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .snapshot();
        TemperatureSnapshotDto {
            cpu: CpuTemperatureDto {
                available: snap.cpu.available,
                temp_celsius: snap.cpu.temp_celsius,
                package_celsius: snap.cpu.package_celsius,
                per_core: snap.cpu.per_core,
                source: snap.cpu.source,
                driver_installed: snap.cpu.driver_installed,
                driver_running: snap.cpu.driver_running,
                driver_version: snap.cpu.driver_version,
                needs_admin: snap.cpu.needs_admin,
                message: snap.cpu.message,
            },
            gpus: snap
                .gpus
                .into_iter()
                .map(|g| GpuTemperatureDto {
                    name: g.name,
                    temp_celsius: g.temp_celsius,
                    usage_percent: g.usage_percent,
                })
                .collect(),
            disks: snap
                .disks
                .into_iter()
                .map(|d| DiskTemperatureDto {
                    drive: d.drive,
                    model: d.model,
                    temp_celsius: d.temp_celsius,
                })
                .collect(),
        }
    }

    /// 磁盘健康快照（状态 / 温度 / 磨损 / 通电时长 / 读写错误）。
    /// `force=true` 跳过 24 小时缓存强制刷新（手动刷新按钮）。
    pub fn get_disk_health(&self, force: bool) -> Vec<DiskHealthDto> {
        timetrace_core::query_disk_health(force)
            .into_iter()
            .map(|d| DiskHealthDto {
                name: d.name,
                status: d.status,
                media_type: d.media_type,
                temp_celsius: d.temp_celsius,
                wear_percent: d.wear_percent,
                power_on_hours: d.power_on_hours,
                read_errors: d.read_errors,
                write_errors: d.write_errors,
            })
            .collect()
    }

    /// 采集并发布实时指标到共享内存（供外部工具/其它语言零拷贝读取）。
    pub fn publish_metrics(&mut self) {
        let Some(publisher) = self.metrics.as_mut() else {
            return;
        };
        let (cpu, mem_used_mb, mem_percent) = {
            let mut hw = self.hardware.lock().unwrap_or_else(|p| p.into_inner());
            let s = hw.snapshot();
            let used = s.memory_used_bytes as f64;
            let total = (s.memory_total_bytes.max(1)) as f64;
            (s.cpu_percent, used / 1_048_576.0, used / total * 100.0)
        };
        let (cpu_temp, gpu_usage, gpu_temp) = {
            let mut t = self.temperature.lock().unwrap_or_else(|p| p.into_inner());
            let s = t.snapshot();
            let gpu = s.gpus.first();
            (
                s.cpu.temp_celsius.unwrap_or(-1.0),
                gpu.and_then(|g| g.usage_percent).unwrap_or(-1.0),
                gpu.and_then(|g| g.temp_celsius).unwrap_or(-1.0),
            )
        };
        let (down, up) = {
            let s = self.monitor_core.network_snapshot();
            (
                s.download_bytes_per_sec as f64,
                s.upload_bytes_per_sec as f64,
            )
        };
        let active_app = DataStore::get_active_session(&*self.db)
            .filter(|s| !s.is_idle)
            .map(|s| s.app_name)
            .unwrap_or_default();

        let mut snap = metrics::MetricsSnapshot {
            cpu_total_percent: cpu,
            cpu_temp_c: cpu_temp,
            gpu_usage_percent: gpu_usage,
            gpu_temp_c: gpu_temp,
            mem_used_mb,
            mem_percent,
            net_down_bps: down,
            net_up_bps: up,
            fps: -1.0, // 帧率预留，未实现
            ..metrics::MetricsSnapshot::default()
        };
        snap.set_active_app(&active_app);
        publisher.publish(snap);

        // 硬件/温度指标分钟级历史（复用 monitor.db 的 metric_samples 桶；
        // 无效值 -1 跳过，避免污染历史）。
        let mut items: Vec<(&str, f64)> = Vec::with_capacity(6);
        items.push(("cpu_percent", cpu));
        items.push(("mem_percent", mem_percent));
        items.push(("mem_used_mb", mem_used_mb));
        if cpu_temp >= 0.0 {
            items.push(("cpu_temp_c", cpu_temp));
        }
        if gpu_usage >= 0.0 {
            items.push(("gpu_usage_percent", gpu_usage));
        }
        if gpu_temp >= 0.0 {
            items.push(("gpu_temp_c", gpu_temp));
        }
        self.monitor_core.record_extra_metrics(&items);
    }
}
