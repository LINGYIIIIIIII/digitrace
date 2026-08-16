//! 监控核心：1s 采样线程（网络流量）、分钟级历史存储、
//! 秒级实时曲线环形缓冲，以及给桥接层读取的最新网络快照。

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use std::time::Duration;

use chrono::{DateTime, Utc};

use super::net::{NetworkSnapshot, WindowsCollector};
use super::store::MetricStore;

/// 秒级网络样本（实时曲线缓冲条目，内存态）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NetSample {
    /// Unix 毫秒。
    pub ts: i64,
    pub down_bps: u64,
    pub up_bps: u64,
}

/// 最新快照（采样线程写、桥接读）。
pub struct MonitorState {
    pub network: RwLock<NetworkSnapshot>,
    /// 秒级实时曲线环形缓冲（采样线程写、桥接读）。
    pub live_net: RwLock<VecDeque<NetSample>>,
}

impl Default for MonitorState {
    fn default() -> Self {
        Self {
            network: RwLock::new(NetworkSnapshot::default()),
            live_net: RwLock::new(VecDeque::new()),
        }
    }
}

pub struct MonitorCore {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    state: Arc<MonitorState>,
    store: Arc<Mutex<MetricStore>>,
    etw: Option<crate::etw_net::EtwNetMonitor>,
    /// 本次启动时刻（UTC），供「本次启动」历史窗口使用。
    started_at: DateTime<Utc>,
    /// 实时曲线留存窗口（秒），采样线程每轮读取，配置变更实时生效。
    live_window_secs: Arc<AtomicU64>,
}

impl MonitorCore {
    /// 启动监控。`db_path` 为 monitor.db 的完整路径；失败静默降级。
    pub fn start(db_path: PathBuf) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::new(MonitorState::default());
        let started_at = Utc::now();
        let live_window_secs = Arc::new(AtomicU64::new(
            crate::AppConfig::load()
                .live
                .network_live_window_seconds
                .clamp(60, 600),
        ));
        let store = Arc::new(Mutex::new(MetricStore::open(db_path, 90).unwrap_or_else(
            |_| {
                MetricStore::open(std::env::temp_dir().join("digitrace_monitor.db"), 90)
                    .expect("fallback monitor db")
            },
        )));
        // ETW 内核网络事件采集（仅管理员权限下可用，用于 ESTATS 不可用时的实时按应用流量）。
        let etw = if crate::is_elevated() {
            crate::oplog::log_event("ETW", "管理员模式，尝试启动内核网络事件采集");
            Some(crate::etw_net::EtwNetMonitor::start())
        } else {
            crate::oplog::log_event("ETW", "非管理员，跳过内核网络事件采集");
            None
        };
        // ── 采样线程 ──
        let stop_flag = stop.clone();
        let s_state = state.clone();
        let s_store = store.clone();
        let s_live_window = live_window_secs.clone();
        let handle = std::thread::Builder::new()
            .name("digitrace-monitor".to_string())
            .spawn(move || {
                crate::oplog::log_event("MONITOR", "监控采样线程启动");
                let mut collector = WindowsCollector::new();
                let mut last_flush = chrono::Local::now();
                // 时区缓存：每 60 秒重读配置，采样写库的"日历日"跟随设置。
                let mut timezone = crate::AppConfig::load().timezone;
                let mut last_tz_check = chrono::Local::now();

                while !stop_flag.load(Ordering::Relaxed) {
                    // 网络快照
                    let net = collector.poll();
                    *s_state.network.write().unwrap() = net.clone();

                    // 秒级实时曲线环形缓冲（按配置窗口裁剪，跨权限可读的秒级历史）
                    {
                        let ts_ms = Utc::now().timestamp_millis();
                        let keep_ms = (s_live_window.load(Ordering::Relaxed) * 1000) as i64;
                        let mut buf = s_state.live_net.write().unwrap();
                        buf.push_back(NetSample {
                            ts: ts_ms,
                            down_bps: net.download_bytes_per_sec,
                            up_bps: net.upload_bytes_per_sec,
                        });
                        while let Some(front) = buf.front() {
                            if ts_ms - front.ts > keep_ms {
                                buf.pop_front();
                            } else {
                                break;
                            }
                        }
                    }

                    // 分钟级历史
                    let now = chrono::Local::now();
                    if (now - last_tz_check).num_seconds() >= 60 {
                        timezone = crate::AppConfig::load().timezone;
                        last_tz_check = now;
                    }
                    let now_fixed = crate::time_util::now_in_for(&timezone);
                    if let Ok(mut store) = s_store.lock() {
                        store.record(
                            &now_fixed,
                            "net_down_bps",
                            net.download_bytes_per_sec as f64,
                        );
                        store.record(&now_fixed, "net_up_bps", net.upload_bytes_per_sec as f64);
                        if (now - last_flush).num_seconds() >= 60 {
                            let _ = store.flush();
                            last_flush = now;
                        }
                    }

                    std::thread::sleep(Duration::from_secs(1));
                }
                // 退出前落盘
                if let Ok(mut store) = s_store.lock() {
                    let _ = store.flush();
                }
                crate::oplog::log_event("MONITOR", "监控采样线程退出");
            })
            .expect("failed to spawn monitor thread");

        Self {
            stop,
            handle: Some(handle),
            state,
            store,
            etw,
            started_at,
            live_window_secs,
        }
    }

    pub fn network_snapshot(&self) -> NetworkSnapshot {
        self.state.network.read().unwrap().clone()
    }

    /// ETW 每进程实时速率（合计，不分方向）；ETW 不可用时返回 None。
    pub fn etw_rates(&self) -> Option<HashMap<u32, f64>> {
        self.etw.as_ref().and_then(|e| e.rates())
    }

    /// ETW 每进程会话累计字节（合计）。
    pub fn etw_session_bytes(&self) -> Option<HashMap<u32, u64>> {
        self.etw.as_ref().and_then(|e| e.session_bytes())
    }

    /// 下载方向历史（分钟级 avg/max）。mode：24h / today / session / 7d / 30d。
    pub fn network_history(&self, mode: &str) -> Vec<crate::monitor::store::Sample> {
        self.store
            .lock()
            .ok()
            .and_then(|s| {
                let config = crate::AppConfig::load();
                let (start_day, start_minute, end_day) =
                    crate::time_util::history_window(&config, mode, self.started_at);
                s.query_window("net_down_bps", &start_day, start_minute, &end_day)
                    .ok()
            })
            .unwrap_or_default()
    }

    /// 上传方向历史（分钟级 avg/max）。
    pub fn network_history_up(&self, mode: &str) -> Vec<crate::monitor::store::Sample> {
        self.store
            .lock()
            .ok()
            .and_then(|s| {
                let config = crate::AppConfig::load();
                let (start_day, start_minute, end_day) =
                    crate::time_util::history_window(&config, mode, self.started_at);
                s.query_window("net_up_bps", &start_day, start_minute, &end_day)
                    .ok()
            })
            .unwrap_or_default()
    }

    /// 批量记录自定义指标到分钟级历史（供硬件/温度等扩展，如 cpu_percent、cpu_temp_c）。
    /// 值由调用方保证有效（无效值不调用即可）。
    pub fn record_extra_metrics(&self, items: &[(&str, f64)]) {
        if items.is_empty() {
            return;
        }
        if let Ok(mut store) = self.store.lock() {
            let config = crate::AppConfig::load();
            let now_fixed = crate::time_util::now_in_for(&config.timezone);
            for (metric, value) in items {
                store.record(&now_fixed, metric, *value);
            }
        }
    }

    /// 指定指标的历史（分钟级 avg/max）。mode：24h / today / session / 7d / 30d。
    pub fn metric_history(&self, metric: &str, mode: &str) -> Vec<crate::monitor::store::Sample> {
        self.store
            .lock()
            .ok()
            .and_then(|s| {
                let config = crate::AppConfig::load();
                let (start_day, start_minute, end_day) =
                    crate::time_util::history_window(&config, mode, self.started_at);
                s.query_window(metric, &start_day, start_minute, &end_day)
                    .ok()
            })
            .unwrap_or_default()
    }

    /// 实时曲线窗口：返回最近 `seconds` 秒（缺省用配置窗口）的秒级样本，升序。
    pub fn live_network_window(&self, seconds: Option<u64>) -> Vec<NetSample> {
        let window_ms = (seconds.unwrap_or_else(|| self.live_window_secs.load(Ordering::Relaxed))
            * 1000) as i64;
        let now_ms = Utc::now().timestamp_millis();
        self.state
            .live_net
            .read()
            .unwrap()
            .iter()
            .filter(|s| now_ms - s.ts <= window_ms)
            .cloned()
            .collect()
    }

    /// 更新实时曲线留存窗口（配置变更时调用，采样线程下一轮生效）。
    pub fn set_live_window(&self, seconds: u64) {
        self.live_window_secs
            .store(seconds.clamp(60, 600), Ordering::Relaxed);
    }

    /// 指定指标某日历日（00:00–24:00，按配置时区）的分钟级样本，返回 (minute, avg)。
    pub fn metric_day(&self, metric: &str, date: &str) -> Vec<(u32, f64)> {
        self.store
            .lock()
            .ok()
            .and_then(|s| {
                s.query_window(metric, date, 0, date).ok().map(|rows| {
                    rows.into_iter()
                        .filter(|sm| sm.day == date)
                        .map(|sm| (sm.minute, sm.avg))
                        .collect()
                })
            })
            .unwrap_or_default()
    }
}

impl Drop for MonitorCore {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(etw) = &self.etw {
            etw.stop();
        }
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        if let Ok(mut store) = self.store.lock() {
            let _ = store.flush();
        }
    }
}
