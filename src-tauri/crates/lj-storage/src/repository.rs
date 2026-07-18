//! `SQLite` Repository 实现 — Graph/MediaItem/Cookie CRUD。

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use crate::ids::RepoId;
use diesel::OptionalExtension;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use keyring::{Entry, Error as KeyringError};
use lj_media::{MediaGraphDelta, MediaItem, MediaResourceId};
use lj_rule_model::Error;
use lj_runtime::Graph;

use crate::library::{LibraryEntry, LibraryProjection};
use crate::models::{
    CookieRow, LibraryEntryRow, MediaGraphRow, MediaRow, NewCookieRow, NewLibraryEntryRow,
    NewMediaGraphRow, NewMediaRow, NewRuleRow, RuleRow,
};
use crate::schema::{cookies, library_entries, media, media_graph, rules, run_migrations};

const COOKIE_KEYRING_SERVICE: &str = "lanjing.cookies";
const COOKIE_KEYRING_MARKER: &str = "keyring:v1";
const EMPTY_STORAGE_FIELD: &str = "";

/// `usize` → `i64` 安全转换。
#[must_use]
fn to_i64(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

fn storage_error(error: impl std::fmt::Display) -> Error {
    Error::Storage(error.to_string())
}

fn cookie_entry(id: &str) -> Result<Entry, Error> {
    Entry::new(COOKIE_KEYRING_SERVICE, id)
        .map_err(|e| Error::Storage(format!("创建 Cookie keyring 条目失败: {e}")))
}

fn encode_cookie_map(
    id: &str,
    value: &CookieMap,
    cache: &Mutex<HashMap<String, String>>,
) -> Result<String, Error> {
    let json = serde_json::to_string(value)?;
    cookie_entry(id)?
        .set_password(&json)
        .map_err(|e| Error::Storage(format!("写入 Cookie keyring 失败: {e}")))?;
    cache
        .lock()
        .map_err(|e| Error::Storage(e.to_string()))?
        .insert(id.to_string(), json);
    Ok(COOKIE_KEYRING_MARKER.to_string())
}

fn decode_cookie_map(
    id: &str,
    stored: &str,
    cache: &Mutex<HashMap<String, String>>,
) -> Result<CookieMap, Error> {
    let json = if stored == COOKIE_KEYRING_MARKER {
        if let Some(json) = cache
            .lock()
            .map_err(|e| Error::Storage(e.to_string()))?
            .get(id)
            .cloned()
        {
            json
        } else {
            cookie_entry(id)?
                .get_password()
                .map_err(|e| Error::Storage(format!("读取 Cookie keyring 失败: {e}")))?
        }
    } else {
        stored.to_string()
    };
    Ok(serde_json::from_str(&json)?)
}

fn delete_cookie_secret(id: &str, cache: &Mutex<HashMap<String, String>>) -> Result<(), Error> {
    cache
        .lock()
        .map_err(|e| Error::Storage(e.to_string()))?
        .remove(id);
    match cookie_entry(id)?.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(e) => Err(Error::Storage(format!("删除 Cookie keyring 失败: {e}"))),
    }
}

fn establish_connection(database_url: &str) -> Result<SqliteConnection, Error> {
    let mut connection = SqliteConnection::establish(database_url)
        .map_err(|e| Error::Storage(format!("打开 SQLite 连接失败: {e}")))?;
    run_migrations(&mut connection)?;
    Ok(connection)
}

fn decode_rule_row(row: RuleRow) -> Result<(RepoId<Graph>, Graph), Error> {
    let graph = serde_json::from_str(&row.graph_json)?;
    Ok((RepoId::<Graph>::new(row.id), graph))
}

fn decode_media_row(row: MediaRow) -> Result<(RepoId<MediaItem>, MediaItem), Error> {
    let media = serde_json::from_str(&row.media_json)?;
    Ok((RepoId::<MediaItem>::new(row.id), media))
}

fn decode_cookie_row(
    row: CookieRow,
    cache: &Mutex<HashMap<String, String>>,
) -> Result<(RepoId<CookieMap>, CookieMap), Error> {
    let cookies = decode_cookie_map(&row.id, &row.cookie_json, cache)?;
    Ok((RepoId::<CookieMap>::new(row.id), cookies))
}

fn decode_library_entry(row: LibraryEntryRow) -> Result<LibraryEntry, Error> {
    let progress = row
        .progress_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()?;
    Ok(LibraryEntry {
        resource_id: MediaResourceId(row.resource_id),
        favorite: row.favorite != 0,
        pinned: row.pinned != 0,
        last_opened_at: row.last_opened_at,
        progress,
    })
}

/// Cookie 容器（新类型包装 `HashMap`，满足孤儿规则）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CookieMap(pub HashMap<String, String>);

/// `SQLite` 存储管理器。
pub struct SqliteStorage {
    conn: Mutex<SqliteConnection>,
    cookie_cache: Mutex<HashMap<String, String>>,
}

impl SqliteStorage {
    /// 创建新存储实例。
    ///
    /// # Errors
    ///
    /// 返回 `Error::Storage` 当数据库打开或 migration 失败。
    pub fn new(path: impl AsRef<Path>) -> Result<Self, Error> {
        let database_url = path.as_ref().to_string_lossy().into_owned();
        Ok(Self {
            conn: Mutex::new(establish_connection(&database_url)?),
            cookie_cache: Mutex::new(HashMap::new()),
        })
    }

    /// 创建内存存储实例（测试用）。
    ///
    /// # Errors
    ///
    /// 返回 `Error::Storage` 当 migration 失败。
    pub fn in_memory() -> Result<Self, Error> {
        Ok(Self {
            conn: Mutex::new(establish_connection(":memory:")?),
            cookie_cache: Mutex::new(HashMap::new()),
        })
    }

    fn lock_connection(&self) -> Result<MutexGuard<'_, SqliteConnection>, Error> {
        self.conn.lock().map_err(|e| Error::Storage(e.to_string()))
    }

    /// 分页列出所有 `Graph`。
    ///
    /// # Errors
    ///
    /// 返回 `Error::Storage` 当数据库查询失败。
    pub fn list_graphs_page(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<(RepoId<Graph>, Graph)>, Error> {
        let mut conn = self.lock_connection()?;
        let rows = rules::table
            .select(RuleRow::as_select())
            .order(rules::id.asc())
            .limit(to_i64(limit))
            .offset(to_i64(offset))
            .load::<RuleRow>(&mut *conn)
            .map_err(storage_error)?;
        rows.into_iter().map(decode_rule_row).collect()
    }

    /// 分页列出所有 Media。
    ///
    /// # Errors
    ///
    /// 返回 `Error::Storage` 当数据库查询失败。
    pub fn list_media_page(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<(RepoId<MediaItem>, MediaItem)>, Error> {
        let mut conn = self.lock_connection()?;
        let rows = media::table
            .select(MediaRow::as_select())
            .order(media::id.asc())
            .limit(to_i64(limit))
            .offset(to_i64(offset))
            .load::<MediaRow>(&mut *conn)
            .map_err(storage_error)?;
        rows.into_iter().map(decode_media_row).collect()
    }

    /// 按源 ID 分页列出 Media。
    ///
    /// # Errors
    ///
    /// 返回 `Error::Storage` 当数据库查询失败。
    pub fn list_media_by_source(
        &self,
        source_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<(RepoId<MediaItem>, MediaItem)>, Error> {
        let mut conn = self.lock_connection()?;
        let rows = media::table
            .filter(media::source_id.eq(source_id))
            .select(MediaRow::as_select())
            .order(media::id.asc())
            .limit(to_i64(limit))
            .offset(to_i64(offset))
            .load::<MediaRow>(&mut *conn)
            .map_err(storage_error)?;
        rows.into_iter().map(decode_media_row).collect()
    }

    /// 分页列出所有 `CookieMap`。
    ///
    /// # Errors
    ///
    /// 返回 `Error::Storage` 当数据库查询失败。
    pub fn list_cookies_page(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<(RepoId<CookieMap>, CookieMap)>, Error> {
        let mut conn = self.lock_connection()?;
        let rows = cookies::table
            .select(CookieRow::as_select())
            .order(cookies::id.asc())
            .limit(to_i64(limit))
            .offset(to_i64(offset))
            .load::<CookieRow>(&mut *conn)
            .map_err(storage_error)?;
        rows.into_iter()
            .map(|row| decode_cookie_row(row, &self.cookie_cache))
            .collect()
    }

    /// 读取当前权威标准媒体资源图。
    ///
    /// # Errors
    ///
    /// 返回 `Error::Storage` 当图读取或反序列化失败。
    pub fn media_graph(&self) -> Result<MediaGraphDelta, Error> {
        let mut conn = self.lock_connection()?;
        let row = media_graph::table
            .find(1)
            .select(MediaGraphRow::as_select())
            .first::<MediaGraphRow>(&mut *conn)
            .optional()
            .map_err(storage_error)?;
        row.map_or_else(
            || Ok(MediaGraphDelta::default()),
            |row| {
                let _ = (row.id, row.updated_at);
                serde_json::from_str(&row.delta_json).map_err(Into::into)
            },
        )
    }

    /// 合并标准媒体资源图增量并持久化。
    ///
    /// # Errors
    ///
    /// 返回 `Error::Storage` 当图读取、合并或写入失败。
    pub fn merge_media_graph_delta(
        &self,
        delta: MediaGraphDelta,
    ) -> Result<MediaGraphDelta, Error> {
        let merged = self.media_graph()?.merge(delta);
        let json = serde_json::to_string(&merged)?;
        let mut conn = self.lock_connection()?;
        let row = NewMediaGraphRow {
            id: 1,
            delta_json: &json,
        };
        diesel::insert_into(media_graph::table)
            .values(&row)
            .on_conflict(media_graph::id)
            .do_update()
            .set(media_graph::delta_json.eq(json.as_str()))
            .execute(&mut *conn)
            .map_err(storage_error)?;
        Ok(merged)
    }

    /// 读取资料库状态与标准资源图组成的共享投影。
    ///
    /// # Errors
    ///
    /// 返回 `Error::Storage` 当图或状态读取失败。
    pub fn library_projection(&self) -> Result<LibraryProjection, Error> {
        let graph = self.media_graph()?;
        let mut conn = self.lock_connection()?;
        let rows = library_entries::table
            .select(LibraryEntryRow::as_select())
            .order(library_entries::resource_id.asc())
            .load::<LibraryEntryRow>(&mut *conn)
            .map_err(storage_error)?;
        let entries = rows
            .into_iter()
            .map(decode_library_entry)
            .collect::<Result<_, _>>()?;
        Ok(LibraryProjection { graph, entries })
    }

    /// 更新资料库唯一拥有的用户状态。
    ///
    /// # Errors
    ///
    /// 当资源不在权威资源图中或状态写入失败时返回 `Error::Storage`。
    pub fn set_library_entry(&self, entry: &LibraryEntry) -> Result<(), Error> {
        let graph = self.media_graph()?;
        if !graph.items.iter().any(|item| item.id == entry.resource_id) {
            return Err(Error::Storage(format!(
                "标准媒体资源不存在: {}",
                entry.resource_id.0
            )));
        }
        let progress_json = entry
            .progress
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let row = NewLibraryEntryRow {
            resource_id: &entry.resource_id.0,
            favorite: i32::from(entry.favorite),
            pinned: i32::from(entry.pinned),
            last_opened_at: entry.last_opened_at.as_deref(),
            progress_json: progress_json.as_deref(),
        };
        let mut conn = self.lock_connection()?;
        diesel::insert_into(library_entries::table)
            .values(&row)
            .on_conflict(library_entries::resource_id)
            .do_update()
            .set((
                library_entries::favorite.eq(row.favorite),
                library_entries::pinned.eq(row.pinned),
                library_entries::last_opened_at.eq(row.last_opened_at),
                library_entries::progress_json.eq(row.progress_json),
            ))
            .execute(&mut *conn)
            .map_err(storage_error)?;
        Ok(())
    }
}

impl SqliteStorage {
    /// 按 ID 读取 `Graph`。
    ///
    /// # Errors
    ///
    /// 数据库查询或反序列化失败时返回错误。
    pub fn get_graph(&self, id: &RepoId<Graph>) -> Result<Option<Graph>, Error> {
        let mut conn = self.lock_connection()?;
        let row = rules::table
            .find(&id.id)
            .select(RuleRow::as_select())
            .first::<RuleRow>(&mut *conn)
            .optional()
            .map_err(storage_error)?;
        row.map(|row| serde_json::from_str(&row.graph_json).map_err(Into::into))
            .transpose()
    }

    /// 保存 `Graph`。
    ///
    /// # Errors
    ///
    /// 数据库写入失败时返回错误。
    pub fn save_graph(&self, id: &RepoId<Graph>, value: &Graph) -> Result<(), Error> {
        let json = serde_json::to_string(value)?;
        let row = NewRuleRow {
            id: &id.id,
            source_url: EMPTY_STORAGE_FIELD,
            graph_json: &json,
            import_hash: EMPTY_STORAGE_FIELD,
        };
        let mut conn = self.lock_connection()?;
        diesel::insert_into(rules::table)
            .values(&row)
            .on_conflict(rules::id)
            .do_update()
            .set((
                rules::source_url.eq(EMPTY_STORAGE_FIELD),
                rules::graph_json.eq(json.as_str()),
                rules::import_hash.eq(EMPTY_STORAGE_FIELD),
            ))
            .execute(&mut *conn)
            .map_err(storage_error)?;
        Ok(())
    }

    /// 删除 `Graph`。
    ///
    /// # Errors
    ///
    /// 数据库删除失败时返回错误。
    pub fn delete_graph(&self, id: &RepoId<Graph>) -> Result<(), Error> {
        let mut conn = self.lock_connection()?;
        diesel::delete(rules::table.find(&id.id))
            .execute(&mut *conn)
            .map_err(storage_error)?;
        Ok(())
    }

    /// 列出全部 `Graph`。
    ///
    /// # Errors
    ///
    /// 数据库查询失败时返回错误。
    pub fn list_graphs(&self) -> Result<Vec<(RepoId<Graph>, Graph)>, Error> {
        let mut conn = self.lock_connection()?;
        let rows = rules::table
            .select(RuleRow::as_select())
            .order(rules::id.asc())
            .load::<RuleRow>(&mut *conn)
            .map_err(storage_error)?;
        rows.into_iter().map(decode_rule_row).collect()
    }
}

impl SqliteStorage {
    /// 按 ID 读取 `MediaItem`。
    ///
    /// # Errors
    ///
    /// 数据库查询失败时返回错误。
    pub fn get_media(&self, id: &RepoId<MediaItem>) -> Result<Option<MediaItem>, Error> {
        let mut conn = self.lock_connection()?;
        let row = media::table
            .find(&id.id)
            .select(MediaRow::as_select())
            .first::<MediaRow>(&mut *conn)
            .optional()
            .map_err(storage_error)?;
        row.map(|row| serde_json::from_str(&row.media_json).map_err(Into::into))
            .transpose()
    }

    /// 保存 `MediaItem`。
    ///
    /// # Errors
    ///
    /// 数据库写入失败时返回错误。
    pub fn save_media(&self, id: &RepoId<MediaItem>, value: &MediaItem) -> Result<(), Error> {
        let json = serde_json::to_string(value)?;
        let row = NewMediaRow {
            id: &id.id,
            source_id: &value.source_id.0,
            media_json: &json,
        };
        let mut conn = self.lock_connection()?;
        diesel::insert_into(media::table)
            .values(&row)
            .on_conflict(media::id)
            .do_update()
            .set((
                media::source_id.eq(value.source_id.0.as_str()),
                media::media_json.eq(json.as_str()),
            ))
            .execute(&mut *conn)
            .map_err(storage_error)?;
        Ok(())
    }

    /// 删除 `MediaItem`。
    ///
    /// # Errors
    ///
    /// 数据库删除失败时返回错误。
    pub fn delete_media(&self, id: &RepoId<MediaItem>) -> Result<(), Error> {
        let mut conn = self.lock_connection()?;
        diesel::delete(media::table.find(&id.id))
            .execute(&mut *conn)
            .map_err(storage_error)?;
        Ok(())
    }

    /// 列出全部 `MediaItem`。
    ///
    /// # Errors
    ///
    /// 数据库查询失败时返回错误。
    pub fn list_media(&self) -> Result<Vec<(RepoId<MediaItem>, MediaItem)>, Error> {
        let mut conn = self.lock_connection()?;
        let rows = media::table
            .select(MediaRow::as_select())
            .order(media::id.asc())
            .load::<MediaRow>(&mut *conn)
            .map_err(storage_error)?;
        rows.into_iter().map(decode_media_row).collect()
    }
}

impl SqliteStorage {
    /// 按 ID 读取 `CookieMap`。
    ///
    /// # Errors
    ///
    /// 数据库查询失败时返回错误。
    pub fn get_cookie(&self, id: &RepoId<CookieMap>) -> Result<Option<CookieMap>, Error> {
        let mut conn = self.lock_connection()?;
        let row = cookies::table
            .find(&id.id)
            .select(CookieRow::as_select())
            .first::<CookieRow>(&mut *conn)
            .optional()
            .map_err(storage_error)?;
        row.map(|row| decode_cookie_map(&row.id, &row.cookie_json, &self.cookie_cache))
            .transpose()
    }

    /// 保存 `CookieMap`。
    ///
    /// # Errors
    ///
    /// 数据库写入失败时返回错误。
    pub fn save_cookie(&self, id: &RepoId<CookieMap>, value: &CookieMap) -> Result<(), Error> {
        let stored = encode_cookie_map(&id.id, value, &self.cookie_cache)?;
        let row = NewCookieRow {
            id: &id.id,
            domain: EMPTY_STORAGE_FIELD,
            cookie_json: &stored,
        };
        let mut conn = self.lock_connection()?;
        if let Err(error) = diesel::insert_into(cookies::table)
            .values(&row)
            .on_conflict(cookies::id)
            .do_update()
            .set((
                cookies::domain.eq(EMPTY_STORAGE_FIELD),
                cookies::cookie_json.eq(stored.as_str()),
            ))
            .execute(&mut *conn)
        {
            let _ = delete_cookie_secret(&id.id, &self.cookie_cache);
            return Err(storage_error(error));
        }
        Ok(())
    }

    /// 删除 `CookieMap`。
    ///
    /// # Errors
    ///
    /// 数据库删除失败时返回错误。
    pub fn delete_cookie(&self, id: &RepoId<CookieMap>) -> Result<(), Error> {
        let mut conn = self.lock_connection()?;
        diesel::delete(cookies::table.find(&id.id))
            .execute(&mut *conn)
            .map_err(storage_error)?;
        delete_cookie_secret(&id.id, &self.cookie_cache)
    }

    /// 列出全部 `CookieMap`。
    ///
    /// # Errors
    ///
    /// 数据库查询失败时返回错误。
    pub fn list_cookies(&self) -> Result<Vec<(RepoId<CookieMap>, CookieMap)>, Error> {
        let mut conn = self.lock_connection()?;
        let rows = cookies::table
            .select(CookieRow::as_select())
            .order(cookies::id.asc())
            .load::<CookieRow>(&mut *conn)
            .map_err(storage_error)?;
        rows.into_iter()
            .map(|row| decode_cookie_row(row, &self.cookie_cache))
            .collect()
    }
}
