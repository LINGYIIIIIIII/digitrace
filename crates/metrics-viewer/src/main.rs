//! 数迹 Lite · 实时指标查看器
//!
//! 零依赖 Win32/GDI 原生窗口：只读共享内存 `%APPDATA%\TimeTrace\metrics.map`，
//! 每 1 秒刷新一次。无 WebView、无第三方 UI 依赖，体积 ~300KB。

#![windows_subsystem = "windows"]

use std::sync::Mutex;
use std::sync::OnceLock;

use windows_sys::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, CreateSolidBrush,
    DEFAULT_CHARSET, DEFAULT_PITCH, EndPaint, FillRect, HBRUSH, HFONT, InvalidateRect,
    OUT_DEFAULT_PRECIS, PAINTSTRUCT, SelectObject, SetBkMode, SetTextColor, TRANSPARENT, TextOutW,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{
    CREATE_NO_WINDOW, CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DispatchMessageW,
    GetClientRect, GetMessageW, IDC_ARROW, KillTimer, LoadCursorW, MSG, PostQuitMessage,
    RegisterClassW, SW_SHOW, SetTimer, ShowWindow, TranslateMessage, WM_DESTROY, WM_PAINT,
    WM_TIMER, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};

const CLASS_NAME: &str = "DigitraceLiteViewer";

/// RGB 宏等价物（windows-sys 无宏，直接拼 COLORREF = u32）。
fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

static READER: Mutex<Option<metrics::MetricsReader>> = Mutex::new(None);
// HFONT/HBRUSH 是 *mut c_void，不能放进要求 Send+Sync 的 OnceLock，存 usize 再转回。
static FONT_HEAD: OnceLock<usize> = OnceLock::new();
static FONT_BODY: OnceLock<usize> = OnceLock::new();
static BRUSH_BG: OnceLock<usize> = OnceLock::new();

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 共享内存数据是否新鲜（5 秒内有更新）。
fn data_is_fresh() -> bool {
    let reader = READER.lock().unwrap();
    let Some(r) = reader.as_ref() else {
        return false;
    };
    let Some(s) = r.read() else {
        return false;
    };
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    now_ms - s.timestamp_ms < 5000
}

/// 尝试拉起同目录下的独立监控（digitrace-monitor.exe）。
/// 完整版正在运行时（数据新鲜）不拉起；无监控可拉时返回 false（保持现有提示）。
fn try_launch_monitor() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let Some(dir) = exe.parent() else {
        return false;
    };
    let mon = dir.join("digitrace-monitor.exe");
    if !mon.exists() {
        return false;
    }
    let mut cmd = wide(&format!("\"{}\"", mon.to_string_lossy()));
    unsafe {
        let mut si: STARTUPINFOW = std::mem::zeroed();
        si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
        let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
        let ok = CreateProcessW(
            std::ptr::null(),
            cmd.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            CREATE_NO_WINDOW,
            std::ptr::null(),
            std::ptr::null(),
            &si,
            &mut pi,
        );
        if ok == 0 {
            return false;
        }
        // 不等待监控初始化；句柄由系统回收（子进程已启动）。
        let _ = pi;
        true
    }
}

fn font_head() -> HFONT {
    *FONT_HEAD.get_or_init(|| {
        unsafe {
            CreateFontW(
                -22,
                0,
                0,
                0,
                700, // FW_BOLD
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32,
                CLIP_DEFAULT_PRECIS as u32,
                CLEARTYPE_QUALITY as u32,
                DEFAULT_PITCH as u32,
                wide("Microsoft YaHei UI").as_ptr(),
            ) as usize
        }
    }) as HFONT
}

fn font_body() -> HFONT {
    *FONT_BODY.get_or_init(|| {
        unsafe {
            CreateFontW(
                -19,
                0,
                0,
                0,
                400, // FW_NORMAL
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32,
                CLIP_DEFAULT_PRECIS as u32,
                CLEARTYPE_QUALITY as u32,
                DEFAULT_PITCH as u32,
                wide("Microsoft YaHei UI").as_ptr(),
            ) as usize
        }
    }) as HFONT
}

fn brush_bg() -> HBRUSH {
    *BRUSH_BG.get_or_init(|| unsafe { CreateSolidBrush(rgb(22, 22, 26)) as usize }) as HBRUSH
}

fn fmt_bps(bps: f64) -> String {
    if bps <= 0.0 {
        return "0 B/s".to_string();
    }
    if bps >= 1_000_000.0 {
        format!("{:.2} MB/s", bps / 1_000_000.0)
    } else if bps >= 1_000.0 {
        format!("{:.1} KB/s", bps / 1_000.0)
    } else {
        format!("{:.0} B/s", bps)
    }
}

fn fmt_mb(mb: f64) -> String {
    if mb >= 1024.0 {
        format!("{:.2} GB", mb / 1024.0)
    } else {
        format!("{:.0} MB", mb)
    }
}

fn fmt_temp(t: f64) -> String {
    if t >= 0.0 {
        format!("{:.1} ℃", t)
    } else {
        "—".to_string()
    }
}

fn fmt_pct(p: f64) -> String {
    if p >= 0.0 {
        format!("{:.1}%", p)
    } else {
        "—".to_string()
    }
}

/// 组装显示行：(标签, 值)。
fn build_lines() -> Vec<(String, String)> {
    let reader = READER.lock().unwrap();
    let Some(r) = reader.as_ref() else {
        return vec![
            ("状态".to_string(), "未检测到监控数据".to_string()),
            (
                "提示".to_string(),
                "请确认 digitrace-monitor.exe 与本程序位于\n同一目录后重新打开。".to_string(),
            ),
        ];
    };
    let Some(s) = r.read() else {
        return vec![("状态".to_string(), "等待数据…".to_string())];
    };
    vec![
        ("CPU 占用".to_string(), fmt_pct(s.cpu_total_percent)),
        ("CPU 温度".to_string(), fmt_temp(s.cpu_temp_c)),
        ("GPU 占用".to_string(), fmt_pct(s.gpu_usage_percent)),
        ("GPU 温度".to_string(), fmt_temp(s.gpu_temp_c)),
        (
            "内存".to_string(),
            format!("{} · {:.1}%", fmt_mb(s.mem_used_mb), s.mem_percent),
        ),
        ("下行速率".to_string(), fmt_bps(s.net_down_bps)),
        ("上行速率".to_string(), fmt_bps(s.net_up_bps)),
        ("前台应用".to_string(), s.active_app_str().to_string()),
    ]
}

fn paint(hwnd: HWND) {
    unsafe {
        let mut ps: PAINTSTRUCT = std::mem::zeroed();
        let hdc = BeginPaint(hwnd, &mut ps);
        let mut rc: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut rc);
        FillRect(hdc, &rc, brush_bg());
        SetBkMode(hdc, TRANSPARENT as i32);

        // 标题
        SelectObject(hdc, font_head());
        SetTextColor(hdc, rgb(255, 110, 60));
        let head = wide("数迹 Lite · 实时指标");
        TextOutW(hdc, 16, 12, head.as_ptr(), head.len() as i32);

        // 数据行
        SelectObject(hdc, font_body());
        let lines = build_lines();
        for (i, (label, value)) in lines.iter().enumerate() {
            let y = 52 + i as i32 * 28;
            SetTextColor(hdc, rgb(150, 150, 160));
            let l = wide(label);
            TextOutW(hdc, 16, y, l.as_ptr(), l.len() as i32);
            SetTextColor(hdc, rgb(232, 232, 238));
            let v = wide(value);
            TextOutW(hdc, 150, y, v.as_ptr(), v.len() as i32);
        }

        // 底部状态：本地时间 + 数据新鲜度（超过 5 秒未刷新提示已过期）。
        let footer = {
            let reader = READER.lock().unwrap();
            match reader.as_ref().and_then(|r| r.read()) {
                Some(s) => {
                    let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or(0);
                    let secs = s.timestamp_ms.div_euclid(1000);
                    let h = secs.div_euclid(3600) % 24;
                    let m = secs.div_euclid(60) % 60;
                    let sec = secs % 60;
                    let stale = if now_ms - s.timestamp_ms > 5000 {
                        " · 数据已过期"
                    } else {
                        ""
                    };
                    format!("更新于 {h:02}:{m:02}:{sec:02}{stale}")
                }
                None => "共享内存未就绪".to_string(),
            }
        };
        SetTextColor(hdc, rgb(110, 110, 120));
        let f = wide(&footer);
        TextOutW(
            hdc,
            16,
            52 + lines.len() as i32 * 28 + 6,
            f.as_ptr(),
            f.len() as i32,
        );

        EndPaint(hwnd, &ps);
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            paint(hwnd);
            0
        }
        WM_TIMER => {
            unsafe {
                let _ = InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            0
        }
        WM_DESTROY => {
            unsafe {
                let _ = KillTimer(hwnd, 1);
                PostQuitMessage(0);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn main() {
    unsafe {
        let hinstance = GetModuleHandleW(std::ptr::null());
        let class_name = wide(CLASS_NAME);

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: std::ptr::null_mut(),
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };
        RegisterClassW(&wc);

        // 打开共享内存；若数据不新鲜（数迹完整版未运行），尝试拉起独立监控，
        // 等它写完第一帧后再重新打开读取器。
        *READER.lock().unwrap() = metrics::MetricsReader::open();
        if !data_is_fresh() && try_launch_monitor() {
            std::thread::sleep(std::time::Duration::from_millis(1500));
            *READER.lock().unwrap() = metrics::MetricsReader::open();
        }

        let title = wide("数迹 Lite · 实时指标");
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            460,
            420,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null_mut(),
        );
        if hwnd.is_null() {
            return;
        }
        ShowWindow(hwnd, SW_SHOW);
        SetTimer(hwnd, 1, 1000, None);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
