//! 敏感字段加解密辅助（调用 security 层；存储只关心计数与占位）。

use tracing::warn;

use crate::security;

/// 加密失败回退明文的累计次数（可观测：隐私降级不应完全静默）。
static ENCRYPT_FALLBACKS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// 解密失败改用占位符的累计次数。
static DECRYPT_PLACEHOLDERS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// 进程内敏感字段加密失败回退明文次数。
pub fn encrypt_fallback_count() -> u64 {
    ENCRYPT_FALLBACKS.load(std::sync::atomic::Ordering::Relaxed)
}

/// 进程内敏感字段解密失败占位符次数。
pub fn decrypt_placeholder_count() -> u64 {
    DECRYPT_PLACEHOLDERS.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn enc_str(s: &str) -> String {
    security::field_encrypt(s).unwrap_or_else(|e| {
        ENCRYPT_FALLBACKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        warn!("敏感字段加密失败，回退明文存储：{e}");
        s.to_string()
    })
}

pub fn enc_opt(s: Option<&str>) -> Option<String> {
    s.map(enc_str)
}

pub fn dec_str(s: String) -> String {
    match security::field_decrypt(&s) {
        Ok(v) => v,
        Err(e) => {
            DECRYPT_PLACEHOLDERS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            warn!("敏感字段解密失败，显示占位符：{e}");
            security::DECRYPT_FAILED_PLACEHOLDER.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decrypt_placeholder_increments_on_garbage() {
        let before = decrypt_placeholder_count();
        let out = dec_str("aes:!!!!not-base64".to_string());
        assert_eq!(out, security::DECRYPT_FAILED_PLACEHOLDER);
        assert_eq!(decrypt_placeholder_count(), before + 1);
    }

    #[test]
    fn encrypt_fallback_and_decrypt_counters_exported() {
        let _ = encrypt_fallback_count();
        let _ = decrypt_placeholder_count();
    }
}
