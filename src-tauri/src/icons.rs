//! Extract application icons from .exe files.
//! Primary: SHGetFileInfoW (resolves shortcuts, handles most exes).
//! Fallback: ExtractIconExW (works where SHGetFileInfoW fails, e.g. UWP stubs).
//! Returns raw RGBA pixels for Flutter rendering.
//!
//! Uses manual FFI declarations to avoid windows-sys feature-gating issues.

use std::mem;

use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDIBits, GetObjectW,
    SelectObject, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

// ── Manual FFI (shell32) ──

// WinAPI 结构体字段名与系统头文件一致，保持原样避免与 SDK 混淆。
#[repr(C)]
#[allow(non_snake_case, clippy::upper_case_acronyms)]
struct SHFILEINFOW {
    hIcon: *mut std::ffi::c_void,
    iIcon: i32,
    dwAttributes: u32,
    szDisplayName: [u16; 260],
    szTypeName: [u16; 80],
}

const SHGFI_ICON: u32 = 0x0000_0100;
const SHGFI_LARGEICON: u32 = 0x0000_0020;

#[link(name = "shell32")]
unsafe extern "system" {
    fn SHGetFileInfoW(
        psz_path: *const u16,
        dw_file_attributes: u32,
        psfi: *mut SHFILEINFOW,
        cb_file_info: u32,
        u_flags: u32,
    ) -> usize;

    fn ExtractIconExW(
        lpsz_file: *const u16,
        n_icon_index: i32,
        phicon_large: *mut *mut std::ffi::c_void,
        phicon_small: *mut *mut std::ffi::c_void,
        n_icons: u32,
    ) -> u32;
}

/// Extract an icon for an exe path.
/// Returns (width, height, rgba_bytes).
pub fn extract_icon_rgba(exe_path: &str) -> Option<(i32, i32, Vec<u8>)> {
    let hicon = get_hicon(exe_path)?;
    unsafe { icon_to_rgba(hicon) }
}

/// Get a large HICON for the path, trying SHGetFileInfoW first then ExtractIconExW.
fn get_hicon(exe_path: &str) -> Option<*mut std::ffi::c_void> {
    let mut path: Vec<u16> = exe_path.encode_utf16().collect();
    path.push(0);

    unsafe {
        // Try 1: SHGetFileInfoW (resolves .lnk, handles most exes)
        let mut info: SHFILEINFOW = mem::zeroed();
        // NOTE: no SHGFI_USEFILEATTRIBUTES — that flag tells the shell NOT
        // to touch the file, so .exe icons come back generic/empty. We want
        // the real exe icon (as .NET ExtractAssociatedIcon does).
        let ret = SHGetFileInfoW(
            path.as_ptr(),
            0,
            &mut info,
            mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );
        if ret != 0 && !info.hIcon.is_null() {
            return Some(info.hIcon);
        }

        // Try 2: ExtractIconExW — direct resource extraction (UWP stubs etc.)
        let mut large: *mut std::ffi::c_void = std::ptr::null_mut();
        let count = ExtractIconExW(path.as_ptr(), 0, &mut large, std::ptr::null_mut(), 1);
        if count > 0 && !large.is_null() {
            return Some(large);
        }
        None
    }
}

/// Convert a HICON to RGBA pixels.
unsafe fn icon_to_rgba(hicon: *mut std::ffi::c_void) -> Option<(i32, i32, Vec<u8>)> {
    unsafe {
        let mut icon_info: ICONINFO = mem::zeroed();
        if GetIconInfo(hicon, &mut icon_info) == 0 {
            DestroyIcon(hicon);
            return None;
        }
        let hbm = icon_info.hbmColor;
        if hbm.is_null() {
            if !icon_info.hbmMask.is_null() {
                DeleteObject(icon_info.hbmMask);
            }
            DestroyIcon(hicon);
            return None;
        }

        let mut bm: BITMAP = mem::zeroed();
        let got = GetObjectW(
            hbm,
            mem::size_of::<BITMAP>() as i32,
            &mut bm as *mut _ as *mut _,
        );
        if got == 0 || bm.bmWidth <= 0 || bm.bmHeight <= 0 {
            DeleteObject(hbm);
            if !icon_info.hbmMask.is_null() {
                DeleteObject(icon_info.hbmMask);
            }
            DestroyIcon(hicon);
            return None;
        }

        let w = bm.bmWidth;
        let h = bm.bmHeight;
        if bm.bmBitsPixel != 32 && bm.bmBitsPixel != 24 {
            DeleteObject(hbm);
            if !icon_info.hbmMask.is_null() {
                DeleteObject(icon_info.hbmMask);
            }
            DestroyIcon(hicon);
            return None;
        }

        let dc = CreateCompatibleDC(std::ptr::null_mut());
        if dc.is_null() {
            DeleteObject(hbm);
            if !icon_info.hbmMask.is_null() {
                DeleteObject(icon_info.hbmMask);
            }
            DestroyIcon(hicon);
            return None;
        }

        let mut bmi: BITMAPINFO = mem::zeroed();
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..Default::default()
        };

        let mut pixels: Vec<u8> = vec![0u8; (w * h * 4) as usize];
        let mut dib_bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let dib = CreateDIBSection(
            dc,
            &bmi,
            DIB_RGB_COLORS,
            &mut dib_bits,
            std::ptr::null_mut(),
            0,
        );
        if dib.is_null() {
            DeleteDC(dc);
            DeleteObject(hbm);
            if !icon_info.hbmMask.is_null() {
                DeleteObject(icon_info.hbmMask);
            }
            DestroyIcon(hicon);
            return None;
        }

        let mut color_rows = 0;
        if !dib.is_null() {
            let old = SelectObject(dc, dib as HGDIOBJ);
            color_rows = GetDIBits(dc, hbm, 0, h as u32, dib_bits, &mut bmi, DIB_RGB_COLORS);
            SelectObject(dc, old);
            if color_rows == h {
                std::ptr::copy_nonoverlapping(
                    dib_bits as *const u8,
                    pixels.as_mut_ptr(),
                    pixels.len(),
                );
            }
            DeleteObject(dib);
        }

        if color_rows != h {
            DeleteDC(dc);
            DeleteObject(hbm);
            if !icon_info.hbmMask.is_null() {
                DeleteObject(icon_info.hbmMask);
            }
            DestroyIcon(hicon);
            return None;
        }

        // Older icons (and many shell-provided icons) have an all-zero alpha
        // channel in hbmColor. Their transparency is encoded in the AND
        // portion of hbmMask instead, so decode that mask before releasing it.
        let mask_alpha = read_mask_alpha(dc, icon_info.hbmMask, w, h);

        DeleteDC(dc);
        DeleteObject(hbm);
        if !icon_info.hbmMask.is_null() {
            DeleteObject(icon_info.hbmMask);
        }
        DestroyIcon(hicon);

        // Canvas ImageData expects straight (non-premultiplied) RGBA. Windows
        // stores the color bitmap as BGRA; preserve its channels as-is and
        // only fall back to the monochrome mask when the alpha channel is
        // unavailable (all zero).
        let has_alpha = pixels.as_chunks::<4>().0.iter().any(|px| px[3] != 0);
        let mut rgba = Vec::with_capacity(pixels.len());
        for (index, px) in pixels.as_chunks::<4>().0.iter().enumerate() {
            let (b, g, r) = (px[0], px[1], px[2]);
            let a = if has_alpha {
                px[3]
            } else {
                mask_alpha
                    .as_ref()
                    .and_then(|alpha| alpha.get(index).copied())
                    .unwrap_or(255)
            };
            rgba.push(r);
            rgba.push(g);
            rgba.push(b);
            rgba.push(a);
        }

        Some((w, h, rgba))
    }
}

/// Read the AND mask from an ICONINFO monochrome bitmap and return one alpha
/// byte per color pixel. The mask bitmap is normally two icon heights tall;
/// only its first (AND) plane controls transparency.
unsafe fn read_mask_alpha(
    dc: *mut std::ffi::c_void,
    hbm_mask: *mut std::ffi::c_void,
    width: i32,
    height: i32,
) -> Option<Vec<u8>> {
    if hbm_mask.is_null() {
        return None;
    }

    let mut mask_bitmap: BITMAP = mem::zeroed();
    if GetObjectW(
        hbm_mask,
        mem::size_of::<BITMAP>() as i32,
        &mut mask_bitmap as *mut _ as *mut _,
    ) == 0
        || mask_bitmap.bmWidth < width
        || mask_bitmap.bmHeight <= 0
    {
        return None;
    }

    let mask_height = mask_bitmap.bmHeight;
    let and_height = if mask_height >= height.saturating_mul(2) {
        height
    } else {
        // Some modern icons expose only the AND plane (height == icon height).
        mask_height.min(height)
    };
    if and_height == 0 {
        return None;
    }

    // A 1bpp DIB scanline is padded to a 4-byte boundary.
    let stride = ((width + 31) / 32) * 4;
    let mut bits = vec![0u8; (stride * mask_height) as usize];
    let mut bmi: BITMAPINFO = mem::zeroed();
    bmi.bmiHeader = BITMAPINFOHEADER {
        biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: width,
        biHeight: -mask_height,
        biPlanes: 1,
        biBitCount: 1,
        biCompression: BI_RGB,
        ..Default::default()
    };
    let rows = GetDIBits(
        dc,
        hbm_mask,
        0,
        mask_height as u32,
        bits.as_mut_ptr() as *mut std::ffi::c_void,
        &mut bmi,
        DIB_RGB_COLORS,
    );
    if rows != mask_height {
        return None;
    }

    let mut alpha = vec![255u8; (width * height) as usize];
    for y in 0..and_height {
        for x in 0..width {
            let byte = bits[(y * stride + x / 8) as usize];
            if byte & (0x80 >> (x % 8)) != 0 {
                alpha[(y * width + x) as usize] = 0;
            }
        }
    }
    Some(alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_real_icons() {
        let paths = [
            r"C:\Windows\explorer.exe",
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Windows\system32\SecurityHealthSystray.exe",
            r"C:\Program Files\WindowsApps\Microsoft.WindowsTerminal_1.24.11911.0_x64__8wekyb3d8bbwe\WindowsTerminal.exe",
        ];
        let mut ok = 0;
        for p in paths {
            if let Some((w, h, rgba)) = extract_icon_rgba(p) {
                eprintln!("OK   {:55} -> {}x{}", p, w, h);
                assert!(w > 0 && h > 0);
                assert_eq!(rgba.len(), (w * h * 4) as usize);
                assert!(rgba.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0));
                let first = &rgba[..4];
                assert!(rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|pixel| pixel.as_slice() != first));
                ok += 1;
            } else {
                eprintln!("FAIL {:55}", p);
            }
        }
        assert!(ok >= 1, "no icons extracted at all");
    }
}
