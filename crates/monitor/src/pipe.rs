//! Read-only local metrics endpoint for external UI integrations.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use metrics::MetricsSnapshot;
use serde::Serialize;
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_PIPE_CONNECTED, GetLastError, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{PIPE_ACCESS_OUTBOUND, WriteFile};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_MESSAGE, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_MESSAGE, PIPE_WAIT,
};

/// Versioned endpoint consumed by DeskBox and other local UI clients.
pub const PIPE_NAME: &str = r"\\.\pipe\DigitraceMetricsV1";
pub const PROTOCOL_VERSION: u32 = 1;

const PIPE_BUFFER_SIZE: u32 = 4096;

#[derive(Debug, Serialize)]
struct PipeMetrics {
    version: u32,
    timestamp_ms: i64,
    sequence: u64,
    cpu_percent: f64,
    memory_percent: f64,
    memory_used_mb: f64,
    cpu_temperature_c: Option<f64>,
    gpu_percent: Option<f64>,
    gpu_temperature_c: Option<f64>,
    gpu_power_watts: Option<f64>,
    network_down_bps: f64,
    network_up_bps: f64,
    active_app: String,
}

#[derive(Clone, Copy)]
pub struct Snapshot {
    pub metrics: MetricsSnapshot,
    pub gpu_power_watts: f64,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            metrics: MetricsSnapshot::default(),
            gpu_power_watts: -1.0,
        }
    }
}

impl From<Snapshot> for PipeMetrics {
    fn from(current: Snapshot) -> Self {
        let snapshot = current.metrics;
        Self {
            version: PROTOCOL_VERSION,
            timestamp_ms: snapshot.timestamp_ms,
            sequence: snapshot.seq,
            cpu_percent: snapshot.cpu_total_percent,
            memory_percent: snapshot.mem_percent,
            memory_used_mb: snapshot.mem_used_mb,
            cpu_temperature_c: valid_metric(snapshot.cpu_temp_c),
            gpu_percent: valid_metric(snapshot.gpu_usage_percent),
            gpu_temperature_c: valid_metric(snapshot.gpu_temp_c),
            gpu_power_watts: valid_metric(current.gpu_power_watts),
            network_down_bps: snapshot.net_down_bps,
            network_up_bps: snapshot.net_up_bps,
            active_app: snapshot.active_app_str().to_owned(),
        }
    }
}

fn valid_metric(value: f64) -> Option<f64> {
    value
        .is_finite()
        .then_some(value)
        .filter(|value| *value >= 0.0)
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Starts a detached server. Each client receives one current JSON snapshot and
/// the outbound-only pipe is then closed, making the endpoint read-only.
pub fn spawn(snapshot: Arc<Mutex<Snapshot>>) {
    let _ = thread::Builder::new()
        .name("digitrace-metrics-pipe".to_owned())
        .spawn(move || {
            loop {
                serve_once(&snapshot);
            }
        });
}

fn serve_once(snapshot: &Arc<Mutex<Snapshot>>) {
    let name = wide(PIPE_NAME);
    let pipe = unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            PIPE_ACCESS_OUTBOUND,
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            PIPE_BUFFER_SIZE,
            0,
            1000,
            std::ptr::null(),
        )
    };

    if pipe == INVALID_HANDLE_VALUE {
        thread::sleep(Duration::from_secs(1));
        return;
    }

    let connected = unsafe { ConnectNamedPipe(pipe, std::ptr::null_mut()) != 0 };
    let connected = connected || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
    if connected
        && let Ok(current) = snapshot.lock()
        && let Ok(mut payload) = serde_json::to_vec(&PipeMetrics::from(*current))
    {
        payload.push(b'\n');
        let _ = write_all(pipe, &payload);
    }

    unsafe {
        CloseHandle(pipe);
    }
}

fn write_all(pipe: HANDLE, payload: &[u8]) -> bool {
    let mut offset = 0usize;
    while offset < payload.len() {
        let mut written = 0u32;
        let ok = unsafe {
            WriteFile(
                pipe,
                payload[offset..].as_ptr(),
                (payload.len() - offset) as u32,
                &mut written,
                std::ptr::null_mut(),
            ) != 0
        };
        if !ok || written == 0 {
            return false;
        }
        offset += written as usize;
    }
    true
}
