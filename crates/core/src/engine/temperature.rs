//! 温度监控：GPU（NVML）/ 磁盘（存储温度属性）/ CPU（PawnIO 内核驱动，可选）。
//!
//! 权限取舍（交给用户选择，与 THRM 等软件共用 PawnIO 驱动，不重复安装）：
//! - GPU 温度：NVML（nvidia-smi 同款接口），免管理员即可读取。
//! - 磁盘温度：Windows 存储温度查询接口，尽力而为；部分磁盘在非管理员下返回 0。
//! - CPU 温度：需要 PawnIO 内核驱动（已装则直接复用），且 MSR 读取要求进程以管理员
//!   身份运行；未满足时返回明确提示，不猜测、不伪造数据。

use std::ffi::c_void;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{CloseHandle, FreeLibrary, HANDLE, HMODULE};
use windows::Win32::Security::{GetTokenInformation, TOKEN_QUERY, TokenElevation};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_MODE,
    OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::System::Threading::{
    GetActiveProcessorCount, GetCurrentProcess, GetCurrentThread, OpenProcessToken,
    SetThreadAffinityMask,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD;
use windows::core::{PCWSTR, w};

use crate::oplog;

// ── PawnIO 协议常量（对应 LibreHardwareMonitor PawnIo.cs，MIT） ──
const IOCTL_PIO_LOAD_BINARY: u32 = (41394 << 16) | (0x821 << 2); // 0xA1B22084
const IOCTL_PIO_EXECUTE_FN: u32 = (41394 << 16) | (0x841 << 2); // 0xA1B22104
const FN_NAME_LENGTH: usize = 32;

// Intel 温度相关 MSR。
const MSR_IA32_THERM_STATUS: u64 = 0x19C;
const MSR_IA32_TEMPERATURE_TARGET: u64 = 0x1A2;
const MSR_IA32_PACKAGE_THERM_STATUS: u64 = 0x1B1;

const UNINSTALL_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\PawnIO";
const CPU_KEY: &str = r"HARDWARE\DESCRIPTION\System\CentralProcessor\0";

/// Intel MSR 模块（编译好的 PawnIO 脚本，直接加载进驱动）。
static INTEL_MSR_BIN: &[u8] = include_bytes!("../../resources/pawnio/IntelMSR.bin");
/// PawnIO 官方安装包（仅当用户确认安装时提取运行，运行需要管理员）。
static PAWNIO_SETUP_EXE: &[u8] = include_bytes!("../../resources/pawnio/PawnIO_setup.exe");

// 磁盘温度查询的 IOCTL（StorageDeviceTemperatureProperty）。
const IOCTL_STORAGE_QUERY_PROPERTY: u32 = 0x002D1400;
const STORAGE_PROPERTY_DEVICE_DESCRIPTOR: u32 = 0;
const STORAGE_PROPERTY_TEMPERATURE: u32 = 6;

#[repr(C)]
struct StoragePropertyQuery {
    property_id: u32,
    query_type: u32,
    additional: [u8; 1],
}

#[repr(C)]
struct StorageTemperatureDescriptor {
    version: u32,
    size: u32,
    temperature: u16,
    min_temperature: i16,
    max_temperature: i16,
    reserved: [u32; 3],
}

#[repr(C)]
struct StorageDeviceDescriptorHeader {
    version: u32,
    size: u32,
    device_type: u8,
    device_type_modifier: u8,
    removable_media: u8,
    command_queueing: u8,
    vendor_id_offset: u32,
    product_id_offset: u32,
    product_revision_offset: u32,
    serial_number_offset: u32,
    bus_type: u32,
}

/// NVMe 协议特定查询参数（StorageDeviceProtocolSpecificProperty）。
#[repr(C)]
struct StorageProtocolSpecificDataExt {
    protocol_type: u32, // ProtocolTypeNvme = 3
    data_type: u32,     // NVMeDataTypeLogPage = 2
    request_value: u32, // Log Page ID（0x04 = Temperature）
    request_subvalue: u32,
    data_offset: u32,
    data_length: u32,
    fixed_return_data: u32,
    request_subvalue2: u32,
    request_subvalue3: u32,
    request_subvalue4: u32,
    request_subvalue5: u32,
}

/// CPU 温度快照。
#[derive(Debug, Clone, Default)]
pub struct CpuTemperature {
    /// 是否成功读到至少一个温度值。
    pub available: bool,
    /// 最高核心温度（°C）。
    pub temp_celsius: Option<f64>,
    /// 封装温度（°C，部分平台无此读数）。
    pub package_celsius: Option<f64>,
    /// 各逻辑核心温度（°C）。
    pub per_core: Vec<f64>,
    /// 数据来源标识（pawnio-msr / unsupported / driver-missing / need-admin…）。
    pub source: String,
    /// PawnIO 驱动是否已安装（注册表）。
    pub driver_installed: bool,
    /// PawnIO 驱动设备是否可打开（已加载运行）。
    pub driver_running: bool,
    pub driver_version: Option<String>,
    /// 是否因为缺少管理员权限而无法读取。
    pub needs_admin: bool,
    /// 面向用户的中文说明（无温度时展示）。
    pub message: Option<String>,
}

/// GPU 温度快照。
#[derive(Debug, Clone)]
pub struct GpuTemperature {
    pub name: String,
    pub temp_celsius: Option<f64>,
    /// GPU 使用率（0-100，NVML utilization，免管理员）。
    pub usage_percent: Option<f64>,
}

/// 物理磁盘温度快照。
#[derive(Debug, Clone)]
pub struct DiskTemperature {
    pub drive: String,
    pub model: String,
    pub temp_celsius: Option<f64>,
}

/// 温度整体快照。
#[derive(Debug, Clone, Default)]
pub struct TemperatureSnapshot {
    pub cpu: CpuTemperature,
    pub gpus: Vec<GpuTemperature>,
    pub disks: Vec<DiskTemperature>,
}

/// 驱动安装/卸载等操作的结果。
#[derive(Debug, Clone)]
pub struct DriverActionResult {
    pub ok: bool,
    pub message: String,
}

/// 温度监控器：持有 NVML 句柄与磁盘缓存，多次 snapshot 复用句柄。
pub struct TemperatureMonitor {
    nvml: Option<NvmlContext>,
    disk_cache: Option<(Instant, Vec<DiskTemperature>)>,
    /// Windows 存储可靠性计数器查询结果缓存（管理员下可用，开销较高所以缓存 15 秒）。
    ps_disk_cache: Option<(Instant, Vec<DiskTemperature>)>,
}

impl TemperatureMonitor {
    pub fn new() -> Self {
        Self {
            nvml: NvmlContext::load(),
            disk_cache: None,
            ps_disk_cache: None,
        }
    }

    pub fn snapshot(&mut self) -> TemperatureSnapshot {
        TemperatureSnapshot {
            cpu: cpu_temperature(),
            gpus: self.gpu_snapshot(),
            disks: self.disk_snapshot(),
        }
    }

    fn gpu_snapshot(&mut self) -> Vec<GpuTemperature> {
        let Some(ctx) = self.nvml.as_mut() else {
            return Vec::new();
        };
        ctx.snapshot()
    }

    fn disk_snapshot(&mut self) -> Vec<DiskTemperature> {
        if let Some((at, cached)) = &self.disk_cache
            && at.elapsed() < Duration::from_secs(10)
        {
            return cached.clone();
        }
        let mut disks = probe_disk_temperatures();
        // 本地存储接口读不到温度时，回退到 Windows 存储可靠性计数器
        // （Get-StorageReliabilityCounter，需管理员；对 NVMe / SATA 都有效）。
        if disks.iter().all(|d| d.temp_celsius.is_none()) {
            let from_ps = match &self.ps_disk_cache {
                Some((at, cached)) if at.elapsed() < Duration::from_secs(15) => cached.clone(),
                _ => {
                    let fresh = query_ps_disk_temps();
                    self.ps_disk_cache = Some((Instant::now(), fresh.clone()));
                    fresh
                }
            };
            for ps in from_ps {
                if let Some(local) = disks.iter_mut().find(|d| d.drive == ps.drive) {
                    local.temp_celsius = ps.temp_celsius;
                } else {
                    disks.push(ps);
                }
            }
        }
        self.disk_cache = Some((Instant::now(), disks.clone()));
        disks
    }
}

impl Default for TemperatureMonitor {
    fn default() -> Self {
        Self::new()
    }
}

// ────────────────────────────── NVML（GPU） ──────────────────────────────

type NvmlInitFn = unsafe extern "system" fn() -> i32;
type NvmlGetHandleFn = unsafe extern "system" fn(u32, *mut *mut c_void) -> i32;
type NvmlGetNameFn = unsafe extern "system" fn(*mut c_void, *mut i8, u32) -> i32;
type NvmlGetTempFn = unsafe extern "system" fn(*mut c_void, u32, *mut u32) -> i32;
type NvmlGetUtilFn = unsafe extern "system" fn(*mut c_void, *mut NvmlUtilization) -> i32;
type NvmlShutdownFn = unsafe extern "system" fn() -> i32;

#[repr(C)]
struct NvmlUtilization {
    gpu: u32,
    memory: u32,
}

struct NvmlContext {
    _module: HMODULE,
    init: NvmlInitFn,
    get_handle: NvmlGetHandleFn,
    get_name: NvmlGetNameFn,
    get_temp: NvmlGetTempFn,
    get_util: NvmlGetUtilFn,
    shutdown: NvmlShutdownFn,
    inited: bool,
}

impl NvmlContext {
    fn load() -> Option<Self> {
        unsafe {
            let module = LoadLibraryW(w!("nvml.dll")).ok()?;
            macro_rules! load_fn {
                ($name:literal, $ty:ty) => {
                    match GetProcAddress(module, windows::core::s!($name)) {
                        Some(fp) => {
                            // GetProcAddress 返回 FARPROC（fn() -> isize），按目标签名转成函数指针。
                            std::mem::transmute::<unsafe extern "system" fn() -> isize, $ty>(fp)
                        }
                        None => {
                            let _ = FreeLibrary(module);
                            return None;
                        }
                    }
                };
            }
            let ctx = Self {
                _module: module,
                init: load_fn!("nvmlInit_v2", NvmlInitFn),
                get_handle: load_fn!("nvmlDeviceGetHandleByIndex_v2", NvmlGetHandleFn),
                get_name: load_fn!("nvmlDeviceGetName", NvmlGetNameFn),
                get_temp: load_fn!("nvmlDeviceGetTemperature", NvmlGetTempFn),
                get_util: load_fn!("nvmlDeviceGetUtilizationRates", NvmlGetUtilFn),
                shutdown: load_fn!("nvmlShutdown", NvmlShutdownFn),
                inited: false,
            };
            Some(ctx)
        }
    }

    fn snapshot(&mut self) -> Vec<GpuTemperature> {
        unsafe {
            if !self.inited {
                if (self.init)() != 0 {
                    return Vec::new();
                }
                self.inited = true;
            }
            let mut out = Vec::new();
            for index in 0..4u32 {
                let mut device = std::ptr::null_mut();
                if (self.get_handle)(index, &mut device) != 0 {
                    break;
                }
                let mut name_buf = [0i8; 256];
                (self.get_name)(device, name_buf.as_mut_ptr(), name_buf.len() as u32);
                let name = cstr_from_i8(&name_buf);
                let mut temp = 0u32;
                let ok = (self.get_temp)(device, 0, &mut temp) == 0;
                let mut util = NvmlUtilization { gpu: 0, memory: 0 };
                let util_ok = (self.get_util)(device, &mut util) == 0;
                out.push(GpuTemperature {
                    name,
                    temp_celsius: if ok { Some(temp as f64) } else { None },
                    usage_percent: if util_ok { Some(util.gpu as f64) } else { None },
                });
            }
            out
        }
    }
}

impl Drop for NvmlContext {
    fn drop(&mut self) {
        unsafe {
            if self.inited {
                let _ = (self.shutdown)();
            }
            let _ = FreeLibrary(self._module);
        }
    }
}

// NVML 句柄只经 TimeTraceApi 的 Mutex 串行访问，跨线程移动安全。
unsafe impl Send for NvmlContext {}
unsafe impl Sync for NvmlContext {}

// ────────────────────────────── 磁盘温度 ──────────────────────────────

fn probe_disk_temperatures() -> Vec<DiskTemperature> {
    let mut out = Vec::new();
    let mut consecutive_missing = 0;
    for index in 0..16u32 {
        let path = format!("\\\\.\\PhysicalDrive{index}");
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        // 温度读取通常要求管理员 + 完整读写权限；先按完整权限打开，
        // 非管理员失败时再回退到只读属性打开（只能拿到型号，拿不到温度）。
        let handle = open_physical_drive(&wide, FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0)
            .or_else(|| open_physical_drive(&wide, 0));
        let Some(handle) = handle else {
            // 打开失败：设备不存在（错误 2）连续多次即可认为没有更多盘。
            if consecutive_missing >= 4 {
                break;
            }
            consecutive_missing += 1;
            continue;
        };
        // 上一轮探测发现：某些驱动在“设备不存在”时也能开成功但查询失败；
        // 统一以查询结果为准，连续几次无有效数据即停止枚举。
        let (model, bus_type) = query_disk_model(handle);
        let temp = query_disk_temperature(handle, bus_type);
        let _ = unsafe { CloseHandle(handle) };
        if model.is_empty() && temp.is_none() {
            consecutive_missing += 1;
            if consecutive_missing >= 4 {
                break;
            }
            continue;
        }
        consecutive_missing = 0;
        out.push(DiskTemperature {
            drive: format!("PhysicalDrive{index}"),
            model,
            temp_celsius: temp,
        });
    }
    out
}

/// 通过 PowerShell 查询 Windows 存储可靠性计数器（管理员权限下可读 NVMe / SATA 温度）。
fn query_ps_disk_temps() -> Vec<DiskTemperature> {
    let ps = r#"
$out = @()
Get-PhysicalDisk -ErrorAction SilentlyContinue | ForEach-Object {
  $r = Get-StorageReliabilityCounter -PhysicalDisk $_ -ErrorAction SilentlyContinue
  if ($r -and $r.Temperature -and $r.Temperature -gt 0) {
    $out += [PSCustomObject]@{ DeviceId = [int]$_.DeviceId; Temp = [double]$r.Temperature }
  }
}
$out | ConvertTo-Json -Compress
"#;
    let Ok(output) = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            ps,
        ])
        // 与 restart_elevated / update.rs 保持一致：CREATE_NO_WINDOW 彻底不创建
        // 控制台窗口。-WindowStyle Hidden 对 GUI 进程启动的 powershell 偶尔无效，
        // 会闪出蓝色 PowerShell 窗口（磁盘温度轮询时每 10 秒闪一次）。
        .creation_flags(0x08000000)
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(text.trim()) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    // PowerShell 5.1：单个结果时 ConvertTo-Json 输出对象而非数组。
    let items: Vec<serde_json::Value> = match &parsed {
        serde_json::Value::Array(a) => a.clone(),
        serde_json::Value::Object(_) => vec![parsed],
        _ => return Vec::new(),
    };
    items
        .into_iter()
        .filter_map(|it| {
            let device_id = it.get("DeviceId").and_then(|v| v.as_i64()).unwrap_or(-1);
            let temp = it.get("Temp").and_then(|v| v.as_f64());
            (device_id >= 0 && temp.is_some()).then(|| DiskTemperature {
                drive: format!("PhysicalDrive{device_id}"),
                model: String::new(),
                temp_celsius: temp,
            })
        })
        .collect()
}

/// 物理磁盘健康信息（只读，来自 Windows Storage 可靠性计数器）。
#[derive(Debug, Clone)]
pub struct DiskHealthInfo {
    pub name: String,
    /// Healthy / Warning / Unhealthy / Unknown
    pub status: String,
    /// HDD / SSD / SCM / Unknown
    pub media_type: String,
    pub temp_celsius: Option<f64>,
    /// SSD 磨损百分比（0-100，越大越接近寿命终点）
    pub wear_percent: Option<f64>,
    pub power_on_hours: Option<u64>,
    pub read_errors: Option<u64>,
    pub write_errors: Option<u64>,
}

/// 查询全部物理磁盘的健康数据（只读；失败或非管理员时返回空/缺省字段）。
/// `force=true` 时跳过缓存强制刷新（手动刷新按钮用）。
pub fn query_disk_health(force: bool) -> Vec<DiskHealthInfo> {
    // PowerShell + Storage 模块查询开销较高，自动查询一天最多一次（缓存 24 小时），
    // 需要看最新数据时手动刷新（force）。
    const CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
    static CACHE: Mutex<Option<(Instant, Vec<DiskHealthInfo>)>> = Mutex::new(None);
    if !force
        && let Ok(guard) = CACHE.lock()
        && let Some((at, cached)) = guard.as_ref()
        && at.elapsed() < CACHE_TTL
    {
        return cached.clone();
    }

    let ps = r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Import-Module Storage -ErrorAction SilentlyContinue
$out = @()
Get-PhysicalDisk -ErrorAction SilentlyContinue | ForEach-Object {
  $r = Get-StorageReliabilityCounter -PhysicalDisk $_ -ErrorAction SilentlyContinue
  $mt = switch ([int]$_.MediaType) { 3 { 'HDD' } 4 { 'SSD' } 5 { 'SCM' } default { 'Unknown' } }
  $out += [PSCustomObject]@{
    Name = $_.FriendlyName
    Status = [string]$_.HealthStatus
    MediaType = $mt
    Temp = if ($r -and $r.Temperature) { [double]$r.Temperature } else { $null }
    Wear = if ($r -and $_.MediaType -eq 4 -and $null -ne $r.Wear) { [double]$r.Wear } else { $null }
    PowerOnHours = if ($r -and $null -ne $r.PowerOnHours) { [uint64]$r.PowerOnHours } else { $null }
    ReadErrors = if ($r -and $null -ne $r.ReadErrorsTotal) { [uint64]$r.ReadErrorsTotal } else { $null }
    WriteErrors = if ($r -and $null -ne $r.WriteErrorsTotal) { [uint64]$r.WriteErrorsTotal } else { $null }
  }
}
if ($out.Count -eq 0) { '[]' } else { $out | ConvertTo-Json -Compress -Depth 3 }
"#;
    let Ok(output) = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            ps,
        ])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW：避免查询时闪现控制台窗口
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(text.trim()) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let items: Vec<serde_json::Value> = match parsed {
        serde_json::Value::Array(a) => a,
        _ => return Vec::new(),
    };
    let result: Vec<DiskHealthInfo> = items
        .into_iter()
        .map(|it| DiskHealthInfo {
            name: it
                .get("Name")
                .and_then(|v| v.as_str())
                .unwrap_or("未知磁盘")
                .to_string(),
            status: it
                .get("Status")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string(),
            media_type: it
                .get("MediaType")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string(),
            temp_celsius: it.get("Temp").and_then(|v| v.as_f64()),
            wear_percent: it.get("Wear").and_then(|v| v.as_f64()),
            power_on_hours: it.get("PowerOnHours").and_then(|v| v.as_u64()),
            read_errors: it.get("ReadErrors").and_then(|v| v.as_u64()),
            write_errors: it.get("WriteErrors").and_then(|v| v.as_u64()),
        })
        .collect();
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some((Instant::now(), result.clone()));
    }
    result
}

fn open_physical_drive(wide: &[u16], access: u32) -> Option<HANDLE> {
    unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            access,
            FILE_SHARE_MODE(7), // READ | WRITE | DELETE
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
        .ok()
    }
}

fn query_disk_temperature(handle: HANDLE, bus_type: u32) -> Option<f64> {
    let mut query = StoragePropertyQuery {
        property_id: STORAGE_PROPERTY_TEMPERATURE,
        query_type: 0,
        additional: [0],
    };
    let mut desc = StorageTemperatureDescriptor {
        version: 0,
        size: 0,
        temperature: 0,
        min_temperature: 0,
        max_temperature: 0,
        reserved: [0; 3],
    };
    let mut returned = 0u32;
    let ok = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            Some((&mut query as *mut StoragePropertyQuery).cast()),
            std::mem::size_of::<StoragePropertyQuery>() as u32,
            Some((&mut desc as *mut StorageTemperatureDescriptor).cast()),
            std::mem::size_of::<StorageTemperatureDescriptor>() as u32,
            Some(&mut returned),
            None,
        )
    };
    if ok.is_err() || returned < 12 {
        // 属性查询失败时，NVMe 走协议特定温度日志页兜底。
        return if bus_type == 139 {
            query_nvme_temperature(handle)
        } else {
            None
        };
    }
    let kelvin = desc.temperature;
    if kelvin == 0 || kelvin == 0xFFFF {
        return if bus_type == 139 {
            query_nvme_temperature(handle)
        } else {
            None
        };
    }
    Some(kelvin as f64 - 273.15)
}

/// NVMe 温度日志页（Log Page 04h）直读：某些 NVMe 固件不通过标准
/// StorageDeviceTemperatureProperty 上报温度，需要协议特定查询。
fn query_nvme_temperature(handle: HANDLE) -> Option<f64> {
    // 输入 = STORAGE_PROPERTY_QUERY(8) + STORAGE_PROTOCOL_SPECIFIC_DATA_EXT(44)。
    let mut input = vec![0u8; 8 + std::mem::size_of::<StorageProtocolSpecificDataExt>()];
    input[0..4].copy_from_slice(&50u32.to_le_bytes()); // StorageDeviceProtocolSpecificProperty
    // query_type = PropertyStandardQuery(0)，无需写。
    let ext = StorageProtocolSpecificDataExt {
        protocol_type: 3,    // ProtocolTypeNvme
        data_type: 2,        // NVMeDataTypeLogPage
        request_value: 0x04, // Temperature log
        request_subvalue: 0,
        data_offset: 0,
        data_length: 512,
        fixed_return_data: 0,
        request_subvalue2: 0,
        request_subvalue3: 0,
        request_subvalue4: 0,
        request_subvalue5: 0,
    };
    let ext_bytes: [u8; 44] = unsafe { std::mem::transmute(ext) };
    input[8..].copy_from_slice(&ext_bytes);

    let mut output = vec![0u8; 1024];
    let mut returned = 0u32;
    let ok = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            Some(input.as_ptr().cast()),
            input.len() as u32,
            Some(output.as_mut_ptr().cast()),
            output.len() as u32,
            Some(&mut returned),
            None,
        )
    };
    if ok.is_err() || returned < 52 {
        return None;
    }
    // 输出 = 描述头(8) + STORAGE_PROTOCOL_SPECIFIC_DATA_EXT(44) + 日志数据。
    let data_offset = 8 + std::mem::size_of::<StorageProtocolSpecificDataExt>();
    let byte = output.get(data_offset).copied()?;
    kelvin_or_celsius(byte)
}

/// NVMe 温度日志页每个传感器 1 字节：值即摄氏度（bit0 有效位，
/// 0/0xFF 表示未上报），真实范围通常在 10–120°C。
fn kelvin_or_celsius(byte: u8) -> Option<f64> {
    if byte == 0 || byte == 0xFF {
        return None;
    }
    let v = byte as f64;
    if (10.0..=120.0).contains(&v) {
        Some(v)
    } else {
        None
    }
}

fn query_disk_model(handle: HANDLE) -> (String, u32) {
    let mut query = StoragePropertyQuery {
        property_id: STORAGE_PROPERTY_DEVICE_DESCRIPTOR,
        query_type: 0,
        additional: [0],
    };
    let mut buf = vec![0u8; 512];
    let mut returned = 0u32;
    let ok = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            Some((&mut query as *mut StoragePropertyQuery).cast()),
            std::mem::size_of::<StoragePropertyQuery>() as u32,
            Some(buf.as_mut_ptr().cast()),
            buf.len() as u32,
            Some(&mut returned),
            None,
        )
    };
    if ok.is_err() || returned < 28 {
        return (String::new(), 0);
    }
    let header = unsafe { &*(buf.as_ptr() as *const StorageDeviceDescriptorHeader) };
    let offset = header.product_id_offset as usize;
    if offset == 0 || offset >= buf.len() {
        return (String::new(), header.bus_type);
    }
    let end = buf[offset..]
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(buf.len() - offset);
    (
        String::from_utf8_lossy(&buf[offset..offset + end])
            .trim()
            .to_string(),
        header.bus_type,
    )
}

// ────────────────────────────── PawnIO / CPU ──────────────────────────────

/// PawnIO 会话：打开设备、加载模块、按函数名执行。
struct PawnIoSession {
    handle: HANDLE,
}

impl PawnIoSession {
    /// 打开设备。返回 Err(code) 时 code 为 Win32 错误码（5=拒绝访问/需管理员）。
    fn open() -> Result<Self, u32> {
        let handle = unsafe {
            CreateFileW(
                w!("\\\\.\\PawnIO"),
                FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0,
                FILE_SHARE_MODE(3), // READ | WRITE
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        };
        match handle {
            Ok(handle) => Ok(Self { handle }),
            Err(e) => Err((e.code().0 & 0xFFFF) as u32),
        }
    }

    fn load_module(&self, bin: &[u8]) -> bool {
        unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_PIO_LOAD_BINARY,
                Some(bin.as_ptr().cast()),
                bin.len() as u32,
                None,
                0,
                None,
                None,
            )
            .is_ok()
        }
    }

    fn execute(&self, name: &str, input: &[i64], out_len: usize) -> Vec<i64> {
        let mut in_buf = vec![0u8; FN_NAME_LENGTH + input.len() * 8];
        let name_bytes = name.as_bytes();
        let n = name_bytes.len().min(FN_NAME_LENGTH - 1);
        in_buf[..n].copy_from_slice(&name_bytes[..n]);
        for (i, v) in input.iter().enumerate() {
            let start = FN_NAME_LENGTH + i * 8;
            in_buf[start..start + 8].copy_from_slice(&v.to_le_bytes());
        }
        let mut out_buf = vec![0u8; out_len * 8];
        let mut returned = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_PIO_EXECUTE_FN,
                Some(in_buf.as_ptr().cast()),
                in_buf.len() as u32,
                Some(out_buf.as_mut_ptr().cast()),
                out_buf.len() as u32,
                Some(&mut returned),
                None,
            )
        };
        if ok.is_err() {
            return Vec::new();
        }
        let count = (returned as usize) / 8;
        (0..count)
            .map(|i| {
                let start = i * 8;
                i64::from_le_bytes(out_buf[start..start + 8].try_into().unwrap())
            })
            .collect()
    }

    fn read_msr(&self, msr: u64) -> Option<u64> {
        let out = self.execute("ioctl_read_msr", &[msr as i64], 1);
        out.first().map(|&v| v as u64)
    }
}

impl Drop for PawnIoSession {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

// PawnIO 设备句柄同样只经 Mutex 串行访问。
unsafe impl Send for PawnIoSession {}
unsafe impl Sync for PawnIoSession {}

/// 查询 PawnIO 驱动状态（已安装 / 设备可打开 / 版本）。
pub fn pawnio_status() -> (bool, bool, Option<String>) {
    let installed = reg_string(UNINSTALL_KEY, "DisplayVersion");
    let version = installed.clone();
    let running = match PawnIoSession::open() {
        Ok(_) => true,
        Err(code) => code != 2 && code != 3, // 除“找不到设备”外都视为驱动已加载
    };
    (version.is_some(), running, version)
}

fn cpu_temperature() -> CpuTemperature {
    let vendor = reg_string(CPU_KEY, "VendorIdentifier").unwrap_or_default();
    let (installed, running, version) = pawnio_status();
    let mut out = CpuTemperature {
        driver_installed: installed,
        driver_running: running,
        driver_version: version,
        source: "unavailable".to_string(),
        ..Default::default()
    };

    if vendor != "GenuineIntel" {
        out.message = Some(if vendor.to_ascii_uppercase().contains("AMD") {
            "AMD 平台 CPU 温度暂未支持（后续版本加入）".to_string()
        } else {
            "未识别到 Intel CPU，无法读取 CPU 温度".to_string()
        });
        out.source = "unsupported".to_string();
        return out;
    }
    if !out.driver_installed {
        out.message =
            Some("未安装 PawnIO 内核驱动，CPU 温度不可用（可在设置中可选安装）".to_string());
        out.source = "driver-missing".to_string();
        return out;
    }
    if !out.driver_running {
        out.message = Some("PawnIO 驱动未运行，请重新安装或重启系统后重试".to_string());
        out.source = "driver-stopped".to_string();
        return out;
    }
    if !is_elevated() {
        out.needs_admin = true;
        out.message = Some("读取 CPU 温度需要管理员权限：请以管理员身份重新运行数迹".to_string());
        out.source = "need-admin".to_string();
        return out;
    }

    let session = match PawnIoSession::open() {
        Ok(s) => s,
        Err(_) => {
            out.message = Some("无法打开 PawnIO 设备".to_string());
            return out;
        }
    };
    if !session.load_module(INTEL_MSR_BIN) {
        out.message = Some("PawnIO 模块加载失败（驱动可能被安全软件拦截）".to_string());
        return out;
    }

    // TjMax：MSR 0x1A2 的 23:16 位；异常时回退 100。
    let tjmax = session
        .read_msr(MSR_IA32_TEMPERATURE_TARGET)
        .map(|v| (v >> 16) & 0xFF)
        .filter(|&v| (1..=127).contains(&v))
        .unwrap_or(100);

    let core_count = unsafe { GetActiveProcessorCount(0) } as usize;
    let core_count = if core_count == 0 || core_count > 64 {
        1
    } else {
        core_count
    };
    let mut per_core = Vec::new();
    // 第一次设置同时返回原始亲和性掩码；结束后恢复。
    let original_affinity = unsafe { SetThreadAffinityMask(GetCurrentThread(), 1) };
    if original_affinity != 0 {
        for index in 0..core_count {
            if index > 0
                && unsafe { SetThreadAffinityMask(GetCurrentThread(), 1usize << index) } == 0
            {
                break;
            }
            if let Some(status) = session.read_msr(MSR_IA32_THERM_STATUS)
                && let Some(temp) = temp_from_therm_status(status, tjmax)
            {
                per_core.push(temp);
            }
        }
        let _ = unsafe { SetThreadAffinityMask(GetCurrentThread(), original_affinity) };
    }

    let package = session
        .read_msr(MSR_IA32_PACKAGE_THERM_STATUS)
        .and_then(|v| temp_from_therm_status(v, tjmax));

    let avg_core = if per_core.is_empty() {
        f64::NAN
    } else {
        per_core.iter().sum::<f64>() / per_core.len() as f64
    };
    out.per_core = per_core;
    out.package_celsius = package;
    if avg_core.is_finite() {
        out.available = true;
        out.temp_celsius = Some(avg_core);
        out.source = "pawnio-msr".to_string();
    } else if let Some(p) = package {
        out.available = true;
        out.temp_celsius = Some(p);
        out.source = "pawnio-msr".to_string();
    } else {
        out.message = Some("未能读到 CPU 温度传感器（MSR 无有效读数）".to_string());
    }
    out
}

fn temp_from_therm_status(status: u64, tjmax: u64) -> Option<f64> {
    if status & (1u64 << 31) == 0 {
        return None;
    }
    let reading = (status >> 16) & 0xFF;
    Some((tjmax.saturating_sub(reading)) as f64)
}

// ────────────────────────────── 驱动安装/卸载 ──────────────────────────────

/// 安装 PawnIO 内核驱动（可选安装）。
///
/// 共用原则：已安装（例如 THRM/OpenRGB 装过）则直接提示复用，不重复安装；
/// 未安装时提取官方安装包并以管理员身份启动，安装动作写入运行日志。
pub fn install_pawnio_driver() -> DriverActionResult {
    let (installed, _, version) = pawnio_status();
    if installed {
        return DriverActionResult {
            ok: true,
            message: format!(
                "PawnIO 内核驱动已安装（v{}），与其他软件共用，无需重复安装。",
                version.unwrap_or_default()
            ),
        };
    }

    let dir = std::env::temp_dir().join("数迹");
    if std::fs::create_dir_all(&dir).is_err() {
        return DriverActionResult {
            ok: false,
            message: "无法创建临时目录，安装失败".to_string(),
        };
    }
    let setup_path = dir.join("PawnIO_setup.exe");
    if let Err(e) = std::fs::write(&setup_path, PAWNIO_SETUP_EXE) {
        return DriverActionResult {
            ok: false,
            message: format!("写入安装程序失败：{e}"),
        };
    }

    match run_elevated(&setup_path.to_string_lossy(), "") {
        true => {
            oplog::log_event(
                "DRIVER",
                &format!(
                    "已启动 PawnIO 内核驱动安装程序（{}），等待用户完成安装",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
                ),
            );
            DriverActionResult {
                ok: true,
                message: "已启动 PawnIO 安装程序，请在弹出的窗口中完成安装（可能需要几分钟）"
                    .to_string(),
            }
        }
        false => DriverActionResult {
            ok: false,
            message: "无法启动安装程序（可能被安全软件拦截或被用户取消）".to_string(),
        },
    }
}

/// 卸载 PawnIO 内核驱动。
///
/// 只调用官方卸载程序；若其它软件（THRM 等）也在使用，由用户自行判断。
pub fn uninstall_pawnio_driver() -> DriverActionResult {
    let Some(uninstall_string) = reg_string(UNINSTALL_KEY, "UninstallString") else {
        return DriverActionResult {
            ok: false,
            message: "未找到 PawnIO 卸载信息，驱动可能已被移除".to_string(),
        };
    };
    let (exe, args) = split_command(&uninstall_string);
    match run_elevated(&exe, &args) {
        true => {
            oplog::log_event(
                "DRIVER",
                &format!(
                    "已启动 PawnIO 内核驱动卸载程序（{}）",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
                ),
            );
            DriverActionResult {
                ok: true,
                message: "已启动 PawnIO 卸载程序，请在弹出的窗口中确认卸载".to_string(),
            }
        }
        false => DriverActionResult {
            ok: false,
            message: "无法启动卸载程序（可能被安全软件拦截或被用户取消）".to_string(),
        },
    }
}

/// 以管理员身份重新启动当前程序（用于提升权限读取 CPU 温度）。
pub fn restart_elevated() -> DriverActionResult {
    let Ok(exe) = std::env::current_exe() else {
        return DriverActionResult {
            ok: false,
            message: "无法定位当前程序路径".to_string(),
        };
    };
    // 延迟 1.5 秒再以管理员身份拉起新实例（-Verb RunAs 弹 UAC）。
    // 等待期间旧实例干净退出、释放单实例锁，避免新实例被旧实例拦截。
    let exe_str = exe.to_string_lossy().replace('\'', "''");
    let ps_script = format!(
        "Start-Sleep -Seconds 1.5; Start-Process -FilePath '{}' -Verb RunAs -ArgumentList '--show-window'",
        exe_str
    );
    let spawned = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &ps_script,
        ])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .spawn();
    if spawned.is_err() {
        return DriverActionResult {
            ok: false,
            message: "无法启动提权重启器（可能被安全软件拦截）".to_string(),
        };
    }
    oplog::log_event("DRIVER", "已请求以管理员身份重启数迹");
    DriverActionResult {
        ok: true,
        message: "正在以管理员身份重新启动数迹…".to_string(),
    }
}

/// 用 ShellExecuteW + runas 以管理员身份启动程序；成功返回 true。
pub(crate) fn run_elevated(exe: &str, args: &str) -> bool {
    let exe_wide: Vec<u16> = exe.encode_utf16().chain(std::iter::once(0)).collect();
    let args_wide: Vec<u16> = args.encode_utf16().chain(std::iter::once(0)).collect();
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("runas"),
            PCWSTR(exe_wide.as_ptr()),
            if args.is_empty() {
                PCWSTR::null()
            } else {
                PCWSTR(args_wide.as_ptr())
            },
            PCWSTR::null(),
            SHOW_WINDOW_CMD(1),
        )
    };
    // ShellExecuteW 返回值 > 32 表示成功。
    (result.0 as isize) > 32
}

/// 把 `"C:\path\a.exe" -arg x` 拆成 (exe, args)。
fn split_command(cmd: &str) -> (String, String) {
    let cmd = cmd.trim();
    if cmd.starts_with('"')
        && let Some(end) = cmd[1..].find('"')
    {
        let exe = cmd[1..1 + end].to_string();
        let args = cmd[2 + end..].trim().to_string();
        return (exe, args);
    }
    match cmd.find(' ') {
        Some(idx) => (cmd[..idx].to_string(), cmd[idx + 1..].trim().to_string()),
        None => (cmd.to_string(), String::new()),
    }
}

// ────────────────────────────── 通用辅助 ──────────────────────────────

pub fn is_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevated = 0u32;
        let mut returned = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some((&mut elevated as *mut u32).cast()),
            4,
            &mut returned,
        );
        let _ = CloseHandle(token);
        ok.is_ok() && elevated != 0
    }
}

fn reg_string(subkey: &str, value: &str) -> Option<String> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY};
    winreg::RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(subkey, KEY_READ | KEY_WOW64_64KEY)
        .and_then(|key| key.get_value(value))
        .ok()
}

fn cstr_from_i8(buf: &[i8]) -> String {
    let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    let bytes: Vec<u8> = buf[..len].iter().map(|&b| b as u8).collect();
    String::from_utf8_lossy(&bytes).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_from_therm_status_formula() {
        // 100°C TjMax，数字读数 40 → 60°C。
        let status = (1u64 << 31) | (40u64 << 16);
        assert_eq!(temp_from_therm_status(status, 100), Some(60.0));
        // 未置有效位 → None。
        assert_eq!(temp_from_therm_status(40u64 << 16, 100), None);
    }

    #[test]
    fn split_command_parses_quoted() {
        let (exe, args) = split_command("\"C:\\Program Files\\PawnIO\\uninstall.exe\" -uninstall");
        assert_eq!(exe, "C:\\Program Files\\PawnIO\\uninstall.exe");
        assert_eq!(args, "-uninstall");
    }

    #[test]
    fn split_command_parses_plain() {
        let (exe, args) = split_command("C:\\tools\\setup.exe /S");
        assert_eq!(exe, "C:\\tools\\setup.exe");
        assert_eq!(args, "/S");
    }

    #[test]
    fn kelvin_or_celsius_handles_both_semantics() {
        // 直接摄氏度。
        assert_eq!(kelvin_or_celsius(40), Some(40.0));
        // 无效值。
        assert_eq!(kelvin_or_celsius(0), None);
        assert_eq!(kelvin_or_celsius(0xFF), None);
    }

    #[test]
    fn monitor_new_does_not_crash() {
        let mut m = TemperatureMonitor::new();
        let _ = m.snapshot();
    }
}
