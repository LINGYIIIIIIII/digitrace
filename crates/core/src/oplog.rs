//! 操作日志：轻量 append-only 文件（默认 %APPDATA%/TimeTrace/op.log）。
//!
//! 记录启动/退出、后台线程关键事件与 panic，用于排查问题。
//! 事件频率低，容量上限 1MB、超出清空重写（轮转）；失败静默，不影响主功能。
//!
//! **加密**：每条记录用主密钥（AES-256-GCM）独立加密，格式：
//! `dgc1` 魔数 + u16 版本 + 逐条 [u16 BE 长度][nonce(12) + 密文+tag]。
//! 旧版明文 op.log 在首次加密写入时轮转清掉（不可读的旧数据不再保留）。

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

const MAX_LOG_BYTES: u64 = 1024 * 1024;
const MAGIC: &[u8; 4] = b"dgc1";
const HEADER_LEN: usize = 6; // 魔数 4 + 版本 u16

struct OpLog {
    path: PathBuf,
}

static OP_LOG: OnceLock<Mutex<OpLog>> = OnceLock::new();
static ENABLED: AtomicBool = AtomicBool::new(true);

fn instance() -> &'static Mutex<OpLog> {
    OP_LOG.get_or_init(|| {
        let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
        let dir = PathBuf::from(base).join("TimeTrace");
        let _ = std::fs::create_dir_all(&dir);
        Mutex::new(OpLog {
            path: dir.join("op.log"),
        })
    })
}

/// 确保文件头存在：无 `dgc1` 魔数的旧明文日志直接轮转（清空重建）。
fn ensure_header(file: &mut File) -> std::io::Result<()> {
    let mut buf = [0u8; HEADER_LEN];
    let read = file.read(&mut buf).unwrap_or(0);
    if read == HEADER_LEN && &buf[..4] == MAGIC {
        return Ok(());
    }
    // 旧明文 / 空文件：重建头部。
    let mut header = Vec::with_capacity(HEADER_LEN);
    header.extend_from_slice(MAGIC);
    header.extend_from_slice(&1u16.to_be_bytes());
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(&header)
}

fn log_path_from(path: &PathBuf) -> std::io::Result<File> {
    // 注意：不能带 append(true)——append 会覆盖 write，Windows 句柄只剩
    // FILE_APPEND_DATA，set_len(truncate) 会 ACCESS_DENIED。这里用
    // read+write 打开，写入前手动 seek 到文件末尾实现追加。
    OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)
}

/// 追加一条加密记录。
fn append_record(file: &mut File, data: &[u8]) -> std::io::Result<()> {
    ensure_header(file)?;
    file.seek(SeekFrom::End(0))?;
    let key = crate::security::master_key().map_err(std::io::Error::other)?;
    let blob = crate::security::encrypt_blob(&key, data).map_err(std::io::Error::other)?;
    let len = (blob.len() as u16).to_be_bytes();
    file.write_all(&len)?;
    file.write_all(&blob)?;
    file.flush()
}

/// 解析整个文件：返回所有解密记录（跳过头部；损坏记录跳过）。
fn parse_records(bytes: &[u8]) -> Vec<Vec<u8>> {
    if bytes.len() < HEADER_LEN || &bytes[..4] != MAGIC {
        return Vec::new();
    }
    let key = match crate::security::master_key() {
        Ok(k) => k,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    let mut pos = HEADER_LEN;
    while pos + 2 <= bytes.len() {
        let len = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]) as usize;
        pos += 2;
        if pos + len > bytes.len() {
            break; // 尾部截断
        }
        if let Some(plain) = crate::security::decrypt_blob(&key, &bytes[pos..pos + len]) {
            out.push(plain);
        }
        pos += len;
    }
    out
}

/// 写一条事件日志（线程安全；任何失败都静默，不影响主流程）。
pub fn log_event(category: &str, message: &str) {
    if !ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!("[{ts}] {category}: {message}\n");
    let Ok(op) = instance().lock() else {
        return;
    };

    // 容量控制：超出上限轮转（清空重建，不保留旧内容——加密记录无法就地裁剪）。
    let too_large = std::fs::metadata(&op.path)
        .map(|m| m.len() + line.len() as u64 > MAX_LOG_BYTES)
        .unwrap_or(false);
    if too_large {
        let _ = std::fs::remove_file(&op.path);
    }

    if let Ok(mut f) = log_path_from(&op.path) {
        let _ = append_record(&mut f, line.as_bytes());
    }
}

/// 启用/停用操作日志（立即生效）。
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

/// 当前是否启用。
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// 日志文件绝对路径。
pub fn log_path() -> PathBuf {
    instance()
        .lock()
        .map(|op| op.path.clone())
        .unwrap_or_default()
}

/// 日志文件当前大小（字节，含密文开销）。
pub fn size_bytes() -> u64 {
    std::fs::metadata(log_path()).map(|m| m.len()).unwrap_or(0)
}

/// 读取日志末尾最多 `n` 行（供界面预览；加密文件自动解密，失败返回空）。
pub fn tail_lines(n: usize) -> String {
    let Ok(content) = std::fs::read(log_path()) else {
        return String::new();
    };
    let records = parse_records(&content);
    let mut lines: Vec<String> = Vec::new();
    for r in records {
        lines.push(
            String::from_utf8_lossy(&r)
                .trim_end_matches('\n')
                .to_string(),
        );
    }
    if lines.is_empty() {
        return String::new();
    }
    let start = lines.len().saturating_sub(n);
    lines[start..].join("\n")
}

/// 清空日志文件（重建头部）。
pub fn clear() {
    let Ok(mut f) = log_path_from(&log_path()) else {
        return;
    };
    let _ = f.set_len(0);
    let _ = f.seek(SeekFrom::Start(0));
    let mut header = Vec::with_capacity(HEADER_LEN);
    header.extend_from_slice(MAGIC);
    header.extend_from_slice(&1u16.to_be_bytes());
    let _ = f.write_all(&header);
    let _ = f.sync_all();
}

/// 安装全局 panic 钩子：任何线程 panic 都先写入操作日志，再转发给默认钩子。
pub fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic payload".to_string()
        };
        log_event("PANIC", &format!("{payload} @ {location}"));
        default(info);
    }));
}

/// 加密日志写入器（供 tracing_subscriber 等把每个 write 块加密追加到文件）。
/// 与 op.log 相同的记录格式，但独立文件、独立头部。
pub struct EncryptedLogWriter {
    path: PathBuf,
}

impl EncryptedLogWriter {
    pub fn new(path: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(path.parent().unwrap_or(std::path::Path::new(".")));
        Self { path }
    }
}

impl Write for EncryptedLogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut f = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&self.path)?;
        append_record(&mut f, buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_tail_and_clear_roundtrip() {
        let old_path = instance().lock().unwrap().path.clone();
        let tmp = std::env::temp_dir().join("tt_oplog_enc_test.log");
        let _ = std::fs::remove_file(&tmp);
        *instance().lock().unwrap() = OpLog { path: tmp.clone() };
        log_event("TEST", "hello");
        log_event("TEST", "world");
        let tail = tail_lines(10);
        assert!(tail.contains("TEST: hello"), "tail={tail}");
        assert!(tail.contains("TEST: world"), "tail={tail}");
        // 文件必须是加密的（不是明文）
        let raw = std::fs::read(&tmp).unwrap();
        assert_eq!(&raw[..4], MAGIC);
        assert!(!String::from_utf8_lossy(&raw).contains("TEST"));
        // 只读尾 1 行
        let last = tail_lines(1);
        assert!(!last.contains("hello"));
        assert!(last.contains("world"));
        clear();
        assert!(tail_lines(10).is_empty());
        let _ = std::fs::remove_file(&tmp);
        *instance().lock().unwrap() = OpLog { path: old_path };
    }
}
