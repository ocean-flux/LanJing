//! `SQLite` schema — 建表 SQL 与初始化。

/// 规则表 DDL。
///
/// 存储 Graph 序列化的 JSON 及相关元信息。
pub const CREATE_RULES_TABLE: &str = "
CREATE TABLE IF NOT EXISTS rules (
    id TEXT PRIMARY KEY,
    source_url TEXT NOT NULL,
    graph_json TEXT NOT NULL,
    import_hash TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
";

/// 媒体表 DDL。
///
/// 存储媒体数据序列化的 JSON。
pub const CREATE_MEDIA_TABLE: &str = "
CREATE TABLE IF NOT EXISTS media (
    id TEXT PRIMARY KEY,
    source_id TEXT NOT NULL,
    media_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
";

/// 媒体表 `source_id` 索引 DDL。
///
/// 加速按源筛选媒体数据的查询。
pub const CREATE_MEDIA_SOURCE_INDEX: &str =
    "CREATE INDEX IF NOT EXISTS idx_media_source ON media(source_id);";

/// Cookie 表 DDL。
///
/// 存储 Cookie 数据序列化的 JSON。
///
/// ## 安全说明
///
/// - `cookie_json` 当前为明文存储，加密推迟实现（KTD16）。
/// - 本表应排除 `WebDAV` 同步，避免明文 `Cookie` 泄露（KTD16）。
pub const CREATE_COOKIES_TABLE: &str = "
CREATE TABLE IF NOT EXISTS cookies (
    id TEXT PRIMARY KEY,
    domain TEXT NOT NULL,
    cookie_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
";

/// 初始化所有表。
///
/// # Errors
///
/// 返回 `CoreError::Storage` 当建表 SQL 执行失败。
pub fn init_db(conn: &rusqlite::Connection) -> Result<(), lj_core::error::CoreError> {
    conn.execute_batch(CREATE_RULES_TABLE)
        .map_err(|e| lj_core::error::CoreError::Storage(e.to_string()))?;
    conn.execute_batch(CREATE_MEDIA_TABLE)
        .map_err(|e| lj_core::error::CoreError::Storage(e.to_string()))?;
    conn.execute_batch(CREATE_MEDIA_SOURCE_INDEX)
        .map_err(|e| lj_core::error::CoreError::Storage(e.to_string()))?;
    conn.execute_batch(CREATE_COOKIES_TABLE)
        .map_err(|e| lj_core::error::CoreError::Storage(e.to_string()))?;
    Ok(())
}
