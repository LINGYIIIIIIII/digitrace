//! 敏感字段加密（窗口标题 / 日记内容）。
//!
//! 使用 Windows DPAPI（CryptProtectData / CryptUnprotectData），密钥由系统
//! 管理，绑定「当前 Windows 用户 + 本机」。数据库文件被拷走、换账号或换机器
//! 后都无法解密。Base64 编码用系统 API 完成，不引入任何第三方依赖。
//!
//! 存储格式：`dpapi:` + Base64(密文)。旧数据没有此前缀，视为明文直接返回
//! （兼容升级前的数据库，由存储层启动时一次性迁移）。

use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPT_STRING_BASE64, CRYPTPROTECT_UI_FORBIDDEN, CryptBinaryToStringW,
    CryptProtectData, CryptStringToBinaryW, CryptUnprotectData,
};

pub const ENC_PREFIX: &str = "dpapi:";

/// 解密失败时 UI 显示的占位符（不泄漏内容、不影响程序运行）。
pub const DECRYPT_FAILED_PLACEHOLDER: &str = "[无法解密]";

pub fn is_encrypted(s: &str) -> bool {
    s.starts_with(ENC_PREFIX)
}

/// 加密明文。空串直接返回空（无需加密）。失败返回 Err。
pub fn encrypt(plain: &str) -> Result<String, String> {
    if plain.is_empty() {
        return Ok(String::new());
    }
    let bytes = plain.as_bytes();
    let in_blob = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut out_blob = CRYPT_INTEGER_BLOB::default();
    let hr = unsafe {
        CryptProtectData(
            &in_blob,
            windows::core::PCWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        )
    };
    if let Err(e) = hr {
        return Err(format!("DPAPI 加密失败：{e}"));
    }
    let cipher =
        unsafe { std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize) }.to_vec();
    unsafe {
        let _ = LocalFree(Some(HLOCAL(out_blob.pbData.cast::<core::ffi::c_void>())));
    }
    let b64 = base64_encode(&cipher)?;
    Ok(format!("{ENC_PREFIX}{b64}"))
}

/// 解密存储值。无 `dpapi:` 前缀视为旧明文原样返回；解密失败返回 Err（调用方
/// 显示占位符）。
pub fn decrypt(stored: &str) -> Result<String, String> {
    if !is_encrypted(stored) {
        return Ok(stored.to_string());
    }
    let b64 = &stored[ENC_PREFIX.len()..];
    let cipher = base64_decode(b64)?;
    let in_blob = CRYPT_INTEGER_BLOB {
        cbData: cipher.len() as u32,
        pbData: cipher.as_ptr() as *mut u8,
    };
    let mut out_blob = CRYPT_INTEGER_BLOB::default();
    let hr = unsafe { CryptUnprotectData(&in_blob, None, None, None, None, 0, &mut out_blob) };
    if let Err(e) = hr {
        return Err(format!("DPAPI 解密失败：{e}"));
    }
    let plain =
        unsafe { std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize) }.to_vec();
    unsafe {
        let _ = LocalFree(Some(HLOCAL(out_blob.pbData.cast::<core::ffi::c_void>())));
    }
    Ok(String::from_utf8_lossy(&plain).into_owned())
}

/// 解密或返回占位符（存储层读取时兜底，绝不中断程序）。
pub fn decrypt_or_placeholder(stored: String) -> String {
    match decrypt(&stored) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("敏感字段解密失败，显示占位符：{e}");
            DECRYPT_FAILED_PLACEHOLDER.to_string()
        }
    }
}

fn base64_encode(data: &[u8]) -> Result<String, String> {
    unsafe {
        let mut len: u32 = 0;
        let _ = CryptBinaryToStringW(data, CRYPT_STRING_BASE64, None, &mut len);
        let mut buf = vec![0u16; len as usize];
        let ok = CryptBinaryToStringW(
            data,
            CRYPT_STRING_BASE64,
            Some(windows::core::PWSTR(buf.as_mut_ptr())),
            &mut len,
        );
        if ok.as_bool() {
            buf.truncate(len as usize);
            Ok(String::from_utf16_lossy(&buf)
                .trim_end_matches('\0')
                .to_string())
        } else {
            Err("Base64 编码失败".to_string())
        }
    }
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    // Windows API 需要以空字符结尾的宽字符串，否则会越界读取。
    let mut wide: Vec<u16> = s.encode_utf16().collect();
    wide.push(0);
    unsafe {
        let mut len: u32 = 0;
        let _ = CryptStringToBinaryW(&wide, CRYPT_STRING_BASE64, None, &mut len, None, None);
        let mut buf = vec![0u8; len as usize];
        CryptStringToBinaryW(
            &wide,
            CRYPT_STRING_BASE64,
            Some(buf.as_mut_ptr()),
            &mut len,
            None,
            None,
        )
        .map_err(|e| format!("Base64 解码失败：{e}"))?;
        buf.truncate(len as usize);
        Ok(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let plain = "Edge - GitHub · 数迹项目讨论";
        let enc = encrypt(plain).unwrap();
        assert!(is_encrypted(&enc));
        assert_eq!(decrypt(&enc).unwrap(), plain);
    }

    #[test]
    fn empty_string_plain() {
        assert_eq!(encrypt("").unwrap(), "");
        assert_eq!(decrypt("").unwrap(), "");
    }

    #[test]
    fn legacy_plaintext_passthrough() {
        let legacy = "这是旧版未加密的数据";
        assert!(!is_encrypted(legacy));
        assert_eq!(decrypt(legacy).unwrap(), legacy);
    }

    #[test]
    fn tampered_blob_rejected() {
        let plain = "需要加密的日记内容";
        let enc = encrypt(plain).unwrap();
        let enc_for_decode = enc.clone();
        let mut bytes = enc.into_bytes();
        // 篡改 DPAPI 文件头第 1 个字节（版本号字段），任何改动都必须被拒绝；
        // 不能改「保留字段」——它本就不参与校验（此前测试的教训）。
        let mid = ENC_PREFIX.len() + 1;
        let orig = bytes[mid];
        bytes[mid] = if orig == b'A' { b'B' } else { b'A' };
        let new_char = bytes[mid];
        let tampered = String::from_utf8(bytes).unwrap();
        // 诊断：篡改前后解码出的密文必须不同，否则说明 Base64 层没生效。
        let orig_cipher = base64_decode(&enc_for_decode[ENC_PREFIX.len()..]).unwrap();
        let tampered_cipher = base64_decode(&tampered[ENC_PREFIX.len()..]).unwrap();
        assert_ne!(
            orig_cipher, tampered_cipher,
            "Base64 解码未反映篡改（orig_char={} new_char={}）",
            orig as char, new_char as char
        );
        assert!(
            decrypt(&tampered).is_err(),
            "篡改后的密文必须解密失败（orig_char={} new_char={}）",
            orig as char,
            new_char as char
        );
    }

    #[test]
    fn long_content_roundtrip() {
        let long = "数迹".repeat(500);
        assert_eq!(decrypt(&encrypt(&long).unwrap()).unwrap(), long);
    }
}
