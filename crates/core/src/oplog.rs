//! 操作日志：轻量 append-only 文件（默认 %APPDATA%/TimeTrace/op.log）。
//!
//! 记录启动/退出、后台线程关键事件、余额/代理事件与 panic，用于排查问题。
//! 事件频率低，容量上限 1MB、超出保留末尾 256KB，失败静默，不影响主功能。

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

const MAX_LOG_BYTES: u64 = 1024 * 1024;
const KEEP_TAIL_BYTES: u64 = 256 * 1024;

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

    // 容量控制：超出上限时保留末尾 KEEP_TAIL_BYTES（从换行处截断）。
    let too_large = std::fs::metadata(&op.path)
        .map(|m| m.len() + line.len() as u64 > MAX_LOG_BYTES)
        .unwrap_or(false);
    if too_large && let Ok(content) = std::fs::read(&op.path) {
        let mut start = content.len().saturating_sub(KEEP_TAIL_BYTES as usize);
        start = content[start..]
            .iter()
            .position(|b| *b == b'\n')
            .map(|i| start + i + 1)
            .unwrap_or(start);
        if let Ok(f) = File::create(&op.path) {
            let _ = (&f).write_all(&content[start..]);
        }
    }

    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&op.path) {
        let _ = f.write_all(line.as_bytes());
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

/// 日志文件当前大小（字节）。
pub fn size_bytes() -> u64 {
    std::fs::metadata(log_path()).map(|m| m.len()).unwrap_or(0)
}

/// 读取日志末尾最多 `n` 行（供界面预览；文件为空返回空字符串）。
pub fn tail_lines(n: usize) -> String {
    let Ok(content) = std::fs::read(log_path()) else {
        return String::new();
    };
    let text = String::from_utf8_lossy(&content);
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return String::new();
    }
    let start = lines.len().saturating_sub(n);
    lines[start..].join("\n")
}

/// 清空日志文件（失败静默）。
pub fn clear() {
    if let Ok(f) = File::create(log_path()) {
        let _ = f.sync_all();
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_and_clear_roundtrip() {
        let old_path = instance().lock().unwrap().path.clone();
        // 用临时路径测试，不污染真实日志。
        let tmp = std::env::temp_dir().join("tt_oplog_test.log");
        let _ = std::fs::remove_file(&tmp);
        *instance().lock().unwrap() = OpLog { path: tmp.clone() };
        log_event("TEST", "hello");
        let tail = tail_lines(10);
        assert!(tail.contains("TEST: hello"));
        clear();
        assert!(tail_lines(10).is_empty());
        let _ = std::fs::remove_file(&tmp);
        *instance().lock().unwrap() = OpLog { path: old_path };
    }
}
