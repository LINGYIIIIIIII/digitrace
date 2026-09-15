//! 打开库时的 schema 版本迁移与旧明文敏感字段加密。
//!
//! 版本号用 SQLite 内置 `PRAGMA user_version`（0 表示从未版本化）。
//! 历史库可能已具备 V1–V3 结构但 user_version=0，因此仍用 pragma 探测
//! 做一次性补齐，再写入 [`SCHEMA_VERSION`](crate::storage::schema::SCHEMA_VERSION)。

use rusqlite::{Connection, params};
use tracing::{info, warn};

use crate::security;
use crate::storage::schema;

/// 读取 `PRAGMA user_version`。
pub fn user_version(conn: &Connection) -> i32 {
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap_or(0)
}

/// 写入 `PRAGMA user_version`。
pub fn set_user_version(conn: &Connection, version: i32) {
    // version 由本模块常量控制，非外部输入
    if let Err(e) = conn.execute_batch(&format!("PRAGMA user_version = {version}")) {
        warn!("写入 schema user_version 失败：{e}");
    }
}

/// 打开时执行：结构 DDL 之后、业务使用之前。
///
/// - 补齐历史 pragma 迁移（幂等）
/// - 敏感字段明文 → AES（幂等、分批）
/// - 将 user_version 提到 [`schema::SCHEMA_VERSION`]
pub fn run_on_open(conn: &Connection) -> Result<(), rusqlite::Error> {
    run_legacy_pragma_migrations(conn)?;
    if let Err(e) = encrypt_legacy_sensitive_fields(conn) {
        warn!("敏感字段加密迁移失败：{e}");
    }
    let target = schema::SCHEMA_VERSION;
    if user_version(conn) < target {
        set_user_version(conn, target);
        info!("schema user_version → {target}");
    }
    Ok(())
}

/// 旧库结构补齐（V1 diary 多条/日、V2 图片关联、V3 status）。
/// 探测失败则跳过对应步骤；仅真实 SQL 错误向上返回。
fn run_legacy_pragma_migrations(conn: &Connection) -> Result<(), rusqlite::Error> {
    // Migration 1: diary_entries.date 曾 UNIQUE
    let has_unique: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_index_list('diary_entries') WHERE \"unique\" = 1 AND origin = 'u'",
        [],
        |row| row.get(0),
    )?;
    if has_unique > 0 {
        for stmt in schema::MIGRATIONS {
            conn.execute_batch(stmt)?;
        }
        info!("diary_entries migrated: multi-entry per day");
    }

    // Migration 2: diary_images.entry_id
    let has_entry_col: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('diary_images') WHERE name = 'entry_id'",
        [],
        |row| row.get(0),
    )?;
    if has_entry_col == 0 {
        for stmt in schema::MIGRATIONS_V2 {
            conn.execute_batch(stmt)?;
        }
        info!("diary_images migrated: entry_id linked");
    }
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_diary_images_entry ON diary_images(entry_id)",
    )?;

    // Migration 3: diary_entries.status
    let has_status: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('diary_entries') WHERE name = 'status'",
        [],
        |row| row.get(0),
    )?;
    if has_status == 0 {
        for stmt in schema::MIGRATIONS_V3 {
            conn.execute_batch(stmt)?;
        }
        info!("diary_entries migrated: status column");
    }
    Ok(())
}

/// 把数据库里残留的旧明文敏感字段一次性加密。
/// 幂等：只处理无 `dpapi:` / `aes:` 前缀的行。
/// id 游标分页 + 500 条事务。
pub fn encrypt_legacy_sensitive_fields(conn: &Connection) -> Result<(), rusqlite::Error> {
    const BATCH: usize = 500;
    let mut updated: u64 = 0;
    for (table, col) in [
        ("usage_sessions", "app_path"),
        ("usage_sessions", "app_name"),
        ("usage_sessions", "window_title"),
        ("page_visits", "app_name"),
        ("page_visits", "window_title"),
        ("diary_entries", "content"),
    ] {
        let select_sql = format!(
            "SELECT id, {col} FROM {table}
             WHERE id > ?2 AND {col} IS NOT NULL AND {col} != ''
               AND {col} NOT LIKE 'dpapi:%' AND {col} NOT LIKE 'aes:%'
             ORDER BY id LIMIT ?1"
        );
        let update_sql = format!("UPDATE {table} SET {col} = ?1 WHERE id = ?2");
        let mut cursor: i64 = 0;
        loop {
            let mut stmt = conn.prepare(&select_sql)?;
            let rows: Vec<(i64, String)> = stmt
                .query_map(params![BATCH as i64, cursor], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })?
                .filter_map(|r| r.ok())
                .collect();
            drop(stmt);
            if rows.is_empty() {
                break;
            }
            let tx = conn.unchecked_transaction()?;
            let mut n = 0u64;
            for (id, plain) in &rows {
                if let Ok(enc) = security::field_encrypt(plain)
                    && tx.execute(&update_sql, params![enc, id]).is_ok()
                {
                    n += 1;
                }
            }
            tx.commit()?;
            updated += n;
            cursor = rows.last().map(|(id, _)| *id).unwrap_or(cursor);
            if rows.len() < BATCH || n == 0 {
                break;
            }
        }
    }
    if updated > 0 {
        info!("敏感字段加密迁移完成：共 {updated} 条");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_version_roundtrip() {
        let conn = Connection::open_in_memory().unwrap();
        assert_eq!(user_version(&conn), 0);
        set_user_version(&conn, 3);
        assert_eq!(user_version(&conn), 3);
    }

    #[test]
    fn run_on_open_sets_schema_version() {
        let conn = Connection::open_in_memory().unwrap();
        for ddl in schema::CREATE_TABLES {
            conn.execute_batch(ddl).unwrap();
        }
        run_on_open(&conn).unwrap();
        assert_eq!(user_version(&conn), schema::SCHEMA_VERSION);
    }
}
