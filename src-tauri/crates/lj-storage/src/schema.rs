//! `SQLite` schema 与 embedded migrations 入口。

use diesel::sqlite::SqliteConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use lj_core::error::CoreError;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

diesel::table! {
    rules (id) {
        id -> Text,
        source_url -> Text,
        graph_json -> Text,
        import_hash -> Text,
        created_at -> Text,
        updated_at -> Text,
    }
}

diesel::table! {
    media (id) {
        id -> Text,
        source_id -> Text,
        media_json -> Text,
        created_at -> Text,
    }
}

diesel::table! {
    cookies (id) {
        id -> Text,
        domain -> Text,
        cookie_json -> Text,
        created_at -> Text,
    }
}

diesel::table! {
    media_graph (id) {
        id -> Integer,
        delta_json -> Text,
        updated_at -> Text,
    }
}

diesel::table! {
    library_entries (resource_id) {
        resource_id -> Text,
        favorite -> Integer,
        pinned -> Integer,
        last_opened_at -> Nullable<Text>,
        progress_json -> Nullable<Text>,
    }
}

diesel::allow_tables_to_appear_in_same_query!(rules, media, cookies, media_graph, library_entries);

/// 运行 embedded migrations。
///
/// # Errors
///
/// 返回 `CoreError::Storage` 当 migration 执行失败。
pub fn run_migrations(conn: &mut SqliteConnection) -> Result<(), CoreError> {
    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|e| CoreError::Storage(format!("执行 SQLite migration 失败: {e}")))?;
    Ok(())
}
