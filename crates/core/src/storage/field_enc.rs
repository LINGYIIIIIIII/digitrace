//! 敏感字段加解密辅助（调用 security 层；存储只关心计数与占位）。

use tracing::warn;

use crate::security;

/// 加密失败回退明文的累计次数（可观测：隐私降级不应完全静默）。
static ENCRYPT_FALLBACKS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// 进程内敏感字段加密失败回退明文次数。
pub fn encrypt_fallback_count() -> u64 {
    ENCRYPT_FALLBACKS.load(std::sync::atomic::Ordering::Relaxed)
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
    security::field_decrypt_or_placeholder(s)
}
