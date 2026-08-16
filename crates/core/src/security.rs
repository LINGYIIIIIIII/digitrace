//! 敏感数据加密（窗口标题 / 日记 / 应用名 / exe 路径 / 配置文件 / 日志）。
//!
//! 两层：
//! 1. **主密钥**：32 字节随机密钥，用 Windows DPAPI（CryptProtectData）加密后
//!    存到 `%APPDATA%\TimeTrace\key.bin`——绑定「当前 Windows 用户 + 本机」，
//!    换账号/换机器都解不开。
//! 2. **数据加密**：用主密钥做 AES-256-GCM（随机 nonce）。
//!    - 字段：`aes:` + Base64(nonce || 密文+tag)，零依赖随机数由 OS 提供。
//!    - 整文件（config / 日志块）：`dgc1` 魔数 + nonce + 密文+tag。
//!
//! 兼容：旧格式 `dpapi:`（DPAPI 字段加密）仍可解密；无前缀视为旧明文。

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use rand::RngCore;
use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPT_STRING_BASE64, CRYPTPROTECT_UI_FORBIDDEN, CryptBinaryToStringW,
    CryptProtectData, CryptStringToBinaryW, CryptUnprotectData,
};

pub const ENC_PREFIX: &str = "dpapi:";
/// 新版字段加密前缀（AES-256-GCM，主密钥 DPAPI 托管）。
pub const AES_PREFIX: &str = "aes:";
/// 整文件/日志加密的魔数（config.json、日志文件头）。
pub const FILE_MAGIC: &[u8; 4] = b"dgc1";

/// 解密失败时 UI 显示的占位符（不泄漏内容、不影响程序运行）。
pub const DECRYPT_FAILED_PLACEHOLDER: &str = "[无法解密]";

fn key_path() -> std::path::PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(base)
        .join("TimeTrace")
        .join("key.bin")
}

/// 主密钥（32 字节）：首次生成并用 DPAPI 加密落盘，之后每次 DPAPI 解密读取。
/// 进程内缓存，避免重复解密。失败返回 Err（调用方应明确报错而不是静默降级）。
pub fn master_key() -> Result<[u8; 32], String> {
    static KEY: std::sync::OnceLock<[u8; 32]> = std::sync::OnceLock::new();
    if let Some(k) = KEY.get() {
        return Ok(*k);
    }
    let path = key_path();
    let key: [u8; 32] = if let Ok(blob) = std::fs::read(&path) {
        let v = dpapi_unprotect(&blob).map_err(|e| format!("读取主密钥失败：{e}"))?;
        v.try_into()
            .map_err(|_| "主密钥长度错误（应为 32 字节）".to_string())?
    } else {
        // 生成新密钥并用 DPAPI 保护后落盘。
        let mut key = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut key);
        let blob = dpapi_protect(&key).map_err(|e| format!("保护主密钥失败：{e}"))?;
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(&path, &blob).map_err(|e| format!("写入主密钥失败：{e}"))?;
        key
    };
    let _ = KEY.set(key);
    Ok(key)
}

/// AES-256-GCM 加密任意字节，返回 `nonce(12) || 密文+tag`。
pub fn encrypt_blob(key: &[u8; 32], plain: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new(key.into());
    let mut nonce = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce), plain)
        .map_err(|e| format!("AES 加密失败：{e}"))?;
    let mut out = Vec::with_capacity(12 + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// AES-256-GCM 解密（输入为 `encrypt_blob` 的输出）。校验失败返回 None。
pub fn decrypt_blob(key: &[u8; 32], blob: &[u8]) -> Option<Vec<u8>> {
    if blob.len() <= 12 {
        return None;
    }
    let cipher = Aes256Gcm::new(key.into());
    cipher
        .decrypt(Nonce::from_slice(&blob[..12]), &blob[12..])
        .ok()
}

/// 整文件加密：`dgc1` 魔数 + nonce + 密文+tag。
pub fn file_encrypt(key: &[u8; 32], plain: &[u8]) -> Result<Vec<u8>, String> {
    let blob = encrypt_blob(key, plain)?;
    let mut out = Vec::with_capacity(4 + blob.len());
    out.extend_from_slice(FILE_MAGIC);
    out.extend_from_slice(&blob);
    Ok(out)
}

/// 整文件解密：带 `dgc1` 魔数时解密；否则视为旧明文原样返回（兼容升级前文件）。
pub fn file_decrypt(key: &[u8; 32], data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() >= 4 && &data[..4] == FILE_MAGIC {
        decrypt_blob(key, &data[4..])
            .ok_or_else(|| "文件解密失败（密钥不匹配或数据损坏）".to_string())
    } else {
        Ok(data.to_vec())
    }
}

/// 字段级加密：`aes:` + Base64(nonce || 密文+tag)。空串直接返回空。
pub fn field_encrypt(plain: &str) -> Result<String, String> {
    if plain.is_empty() {
        return Ok(String::new());
    }
    let key = master_key()?;
    let blob = encrypt_blob(&key, plain.as_bytes())?;
    let b64 = base64_encode(&blob)?;
    Ok(format!("{AES_PREFIX}{b64}"))
}

/// 字段级解密：优先 `aes:`，其次旧 `dpapi:`，无前缀视为旧明文。
pub fn field_decrypt(stored: &str) -> Result<String, String> {
    if stored.is_empty() {
        return Ok(String::new());
    }
    if let Some(b64) = stored.strip_prefix(AES_PREFIX) {
        let blob = base64_decode(b64)?;
        let key = master_key()?;
        let plain = decrypt_blob(&key, &blob).ok_or("AES 字段解密失败（密钥不匹配或数据损坏）")?;
        return Ok(String::from_utf8_lossy(&plain).into_owned());
    }
    if is_encrypted(stored) {
        return dpapi_decrypt_str(stored);
    }
    Ok(stored.to_string()) // 旧明文
}

/// 字段级解密或返回占位符（存储层读取时兜底，绝不中断程序）。
pub fn field_decrypt_or_placeholder(stored: String) -> String {
    match field_decrypt(&stored) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("敏感字段解密失败，显示占位符：{e}");
            DECRYPT_FAILED_PLACEHOLDER.to_string()
        }
    }
}

/// 旧版 DPAPI 字段解密（`dpapi:` 前缀，兼容历史数据）。
fn dpapi_decrypt_str(stored: &str) -> Result<String, String> {
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

pub fn is_encrypted(s: &str) -> bool {
    s.starts_with(ENC_PREFIX)
}

fn dpapi_protect(data: &[u8]) -> Result<Vec<u8>, String> {
    let in_blob = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
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
    let out =
        unsafe { std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize) }.to_vec();
    unsafe {
        let _ = LocalFree(Some(HLOCAL(out_blob.pbData.cast::<core::ffi::c_void>())));
    }
    Ok(out)
}

fn dpapi_unprotect(blob: &[u8]) -> Result<Vec<u8>, String> {
    let in_blob = CRYPT_INTEGER_BLOB {
        cbData: blob.len() as u32,
        pbData: blob.as_ptr() as *mut u8,
    };
    let mut out_blob = CRYPT_INTEGER_BLOB::default();
    let hr = unsafe { CryptUnprotectData(&in_blob, None, None, None, None, 0, &mut out_blob) };
    if let Err(e) = hr {
        return Err(format!("DPAPI 解密失败：{e}"));
    }
    let out =
        unsafe { std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize) }.to_vec();
    unsafe {
        let _ = LocalFree(Some(HLOCAL(out_blob.pbData.cast::<core::ffi::c_void>())));
    }
    Ok(out)
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
    fn field_roundtrip() {
        let plain = "Edge - GitHub · 数迹项目讨论";
        let enc = field_encrypt(plain).unwrap();
        assert!(enc.starts_with(AES_PREFIX));
        assert_eq!(field_decrypt(&enc).unwrap(), plain);
    }

    #[test]
    fn empty_string_plain() {
        assert_eq!(field_encrypt("").unwrap(), "");
        assert_eq!(field_decrypt("").unwrap(), "");
    }

    #[test]
    fn legacy_plaintext_passthrough() {
        let legacy = "这是旧版未加密的数据";
        assert_eq!(field_decrypt(legacy).unwrap(), legacy);
    }

    #[test]
    fn blob_roundtrip_and_tamper() {
        let key = [7u8; 32];
        let blob = encrypt_blob(&key, b"hello world").unwrap();
        assert_eq!(decrypt_blob(&key, &blob).unwrap(), b"hello world");
        // 篡改一个字节必须解密失败
        let mut tampered = blob.clone();
        tampered[12] ^= 0xFF;
        assert!(decrypt_blob(&key, &tampered).is_none());
        // 错误密钥必须失败
        assert!(decrypt_blob(&[8u8; 32], &blob).is_none());
    }

    #[test]
    fn file_roundtrip_and_legacy_plain() {
        let key = [9u8; 32];
        let enc = file_encrypt(&key, b"{\"a\":1}").unwrap();
        assert_eq!(file_decrypt(&key, &enc).unwrap(), b"{\"a\":1}");
        // 旧明文（无魔数）原样返回
        assert_eq!(file_decrypt(&key, b"{\"a\":1}").unwrap(), b"{\"a\":1}");
        // 魔数损坏必须报错
        let mut bad = enc.clone();
        bad[4] ^= 0xFF; // 破坏 nonce（保留魔数）
        assert!(file_decrypt(&key, &bad).is_err());
    }

    #[test]
    fn master_key_stable_and_bound() {
        let k1 = master_key().unwrap();
        let k2 = master_key().unwrap();
        assert_eq!(k1, k2, "主密钥必须稳定（同一进程/机器）");
        assert_eq!(k1.len(), 32);
    }
}
