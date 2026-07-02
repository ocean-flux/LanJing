//! `SQLite` Repository 实现 — Graph/Media/Cookie CRUD。

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

/// `usize` → `i64` 安全转换(超 `i64::MAX` 截断,分页参数不会到这量级)
fn to_i64(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

use lj_core::error::CoreError;
use lj_core::media::Media;
use lj_core::node::Graph;
use lj_core::traits::{RepoId, Repository};
use rusqlite::OptionalExtension;

use crate::schema;

/// Cookie 容器（新类型包装 `HashMap`，满足孤儿规则）。
///
/// 存储层使用本类型而非裸 `HashMap`，以便为外来类型实现 `Repository` trait。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CookieMap(pub HashMap<String, String>);

/// `SQLite` 存储管理器。
pub struct SqliteStorage {
    /// 数据库连接。
    conn: Mutex<rusqlite::Connection>,
}

impl SqliteStorage {
    /// 创建新存储实例，打开或创建指定路径的 `SQLite` 数据库。
    ///
    /// # Errors
    ///
    /// 返回 `CoreError::Storage` 当数据库打开或初始化失败。
    pub fn new(path: impl AsRef<Path>) -> Result<Self, CoreError> {
        let conn = rusqlite::Connection::open(path.as_ref())
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        schema::init_db(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// 创建内存存储实例（测试用）。
    ///
    /// # Errors
    ///
    /// 返回 `CoreError::Storage` 当数据库初始化失败。
    pub fn in_memory() -> Result<Self, CoreError> {
        let conn = rusqlite::Connection::open_in_memory()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        schema::init_db(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// 分页列出所有 Graph（规则表条目少，但分页接口可避免全量加载）。
    ///
    /// # Errors
    ///
    /// 返回 `CoreError::Storage` 当数据库查询失败。
    pub fn list_graphs_page(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<(RepoId<Graph>, Graph)>, CoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT id, graph_json FROM rules LIMIT ?1 OFFSET ?2")
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let rows = stmt
            .query_map(rusqlite::params![to_i64(limit), to_i64(offset)], |row| {
                let id: String = row.get(0)?;
                let json: String = row.get(1)?;
                Ok((id, json))
            })
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let mut result = Vec::new();
        for row in rows {
            let (id_str, json) = row.map_err(|e| CoreError::Storage(e.to_string()))?;
            let graph: Graph = serde_json::from_str(&json)?;
            result.push((RepoId::<Graph>::new(id_str), graph));
        }
        Ok(result)
    }

    /// 分页列出所有 Media。
    ///
    /// # Errors
    ///
    /// 返回 `CoreError::Storage` 当数据库查询失败。
    pub fn list_media_page(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<(RepoId<Media>, Media)>, CoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT id, media_json FROM media LIMIT ?1 OFFSET ?2")
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let rows = stmt
            .query_map(rusqlite::params![to_i64(limit), to_i64(offset)], |row| {
                let id: String = row.get(0)?;
                let json: String = row.get(1)?;
                Ok((id, json))
            })
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let mut result = Vec::new();
        for row in rows {
            let (id_str, json) = row.map_err(|e| CoreError::Storage(e.to_string()))?;
            let media: Media = serde_json::from_str(&json)?;
            result.push((RepoId::<Media>::new(id_str), media));
        }
        Ok(result)
    }

    /// 按源 ID 分页列出 Media。
    ///
    /// 使用 `idx_media_source` 索引加速查询。
    ///
    /// # Errors
    ///
    /// 返回 `CoreError::Storage` 当数据库查询失败。
    pub fn list_media_by_source(
        &self,
        source_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<(RepoId<Media>, Media)>, CoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT id, media_json FROM media WHERE source_id = ?1 LIMIT ?2 OFFSET ?3")
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let rows = stmt
            .query_map(
                rusqlite::params![source_id, to_i64(limit), to_i64(offset)],
                |row| {
                    let id: String = row.get(0)?;
                    let json: String = row.get(1)?;
                    Ok((id, json))
                },
            )
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let mut result = Vec::new();
        for row in rows {
            let (id_str, json) = row.map_err(|e| CoreError::Storage(e.to_string()))?;
            let media: Media = serde_json::from_str(&json)?;
            result.push((RepoId::<Media>::new(id_str), media));
        }
        Ok(result)
    }

    /// 分页列出所有 `CookieMap`。
    ///
    /// # Errors
    ///
    /// 返回 `CoreError::Storage` 当数据库查询失败。
    pub fn list_cookies_page(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<(RepoId<CookieMap>, CookieMap)>, CoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT id, cookie_json FROM cookies LIMIT ?1 OFFSET ?2")
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let rows = stmt
            .query_map(rusqlite::params![to_i64(limit), to_i64(offset)], |row| {
                let id: String = row.get(0)?;
                let json: String = row.get(1)?;
                Ok((id, json))
            })
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let mut result = Vec::new();
        for row in rows {
            let (id_str, json) = row.map_err(|e| CoreError::Storage(e.to_string()))?;
            let cookies: CookieMap = serde_json::from_str(&json)?;
            result.push((RepoId::<CookieMap>::new(id_str), cookies));
        }
        Ok(result)
    }
}

impl Repository<Graph> for SqliteStorage {
    fn get(&self, id: &RepoId<Graph>) -> Result<Option<Graph>, CoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT graph_json FROM rules WHERE id = ?1")
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let result: Option<String> = stmt
            .query_row(rusqlite::params![id.id], |row| row.get(0))
            .optional()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        match result {
            Some(json) => Ok(Some(serde_json::from_str(&json)?)),
            None => Ok(None),
        }
    }

    fn save(&self, id: &RepoId<Graph>, value: &Graph) -> Result<(), CoreError> {
        let json = serde_json::to_string(value)?;
        let conn = self
            .conn
            .lock()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        conn.execute(
            "INSERT OR REPLACE INTO rules (id, source_url, graph_json, import_hash) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![id.id, "", json, ""],
        )
        .map_err(|e| CoreError::Storage(e.to_string()))?;
        Ok(())
    }

    fn delete(&self, id: &RepoId<Graph>) -> Result<(), CoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        conn.execute("DELETE FROM rules WHERE id = ?1", rusqlite::params![id.id])
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        Ok(())
    }

    fn list(&self) -> Result<Vec<(RepoId<Graph>, Graph)>, CoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT id, graph_json FROM rules")
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let rows = stmt
            .query_map(rusqlite::params![], |row| {
                let id: String = row.get(0)?;
                let json: String = row.get(1)?;
                Ok((id, json))
            })
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let mut result = Vec::new();
        for row in rows {
            let (id_str, json) = row.map_err(|e| CoreError::Storage(e.to_string()))?;
            let graph: Graph = serde_json::from_str(&json)?;
            result.push((RepoId::<Graph>::new(id_str), graph));
        }
        Ok(result)
    }
}

impl Repository<Media> for SqliteStorage {
    fn get(&self, id: &RepoId<Media>) -> Result<Option<Media>, CoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT media_json FROM media WHERE id = ?1")
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let result: Option<String> = stmt
            .query_row(rusqlite::params![id.id], |row| row.get(0))
            .optional()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        match result {
            Some(json) => Ok(Some(serde_json::from_str(&json)?)),
            None => Ok(None),
        }
    }

    fn save(&self, id: &RepoId<Media>, value: &Media) -> Result<(), CoreError> {
        let json = serde_json::to_string(value)?;
        let conn = self
            .conn
            .lock()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        conn.execute(
            "INSERT OR REPLACE INTO media (id, source_id, media_json) VALUES (?1, ?2, ?3)",
            rusqlite::params![id.id, "", json],
        )
        .map_err(|e| CoreError::Storage(e.to_string()))?;
        Ok(())
    }

    fn delete(&self, id: &RepoId<Media>) -> Result<(), CoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        conn.execute("DELETE FROM media WHERE id = ?1", rusqlite::params![id.id])
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        Ok(())
    }

    fn list(&self) -> Result<Vec<(RepoId<Media>, Media)>, CoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT id, media_json FROM media")
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let rows = stmt
            .query_map(rusqlite::params![], |row| {
                let id: String = row.get(0)?;
                let json: String = row.get(1)?;
                Ok((id, json))
            })
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let mut result = Vec::new();
        for row in rows {
            let (id_str, json) = row.map_err(|e| CoreError::Storage(e.to_string()))?;
            let media: Media = serde_json::from_str(&json)?;
            result.push((RepoId::<Media>::new(id_str), media));
        }
        Ok(result)
    }
}

impl Repository<CookieMap> for SqliteStorage {
    fn get(&self, id: &RepoId<CookieMap>) -> Result<Option<CookieMap>, CoreError> {
        // TODO: 解密 cookie 值（keyring crate），KTD16 要求
        let conn = self
            .conn
            .lock()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT cookie_json FROM cookies WHERE id = ?1")
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let result: Option<String> = stmt
            .query_row(rusqlite::params![id.id], |row| row.get(0))
            .optional()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        match result {
            Some(json) => Ok(Some(serde_json::from_str(&json)?)),
            None => Ok(None),
        }
    }

    fn save(&self, id: &RepoId<CookieMap>, value: &CookieMap) -> Result<(), CoreError> {
        // TODO: 加密 cookie 值（keyring crate），KTD16 要求
        let json = serde_json::to_string(value)?;
        let conn = self
            .conn
            .lock()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        conn.execute(
            "INSERT OR REPLACE INTO cookies (id, domain, cookie_json) VALUES (?1, ?2, ?3)",
            rusqlite::params![id.id, "", json],
        )
        .map_err(|e| CoreError::Storage(e.to_string()))?;
        Ok(())
    }

    fn delete(&self, id: &RepoId<CookieMap>) -> Result<(), CoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        conn.execute(
            "DELETE FROM cookies WHERE id = ?1",
            rusqlite::params![id.id],
        )
        .map_err(|e| CoreError::Storage(e.to_string()))?;
        Ok(())
    }

    fn list(&self) -> Result<Vec<(RepoId<CookieMap>, CookieMap)>, CoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT id, cookie_json FROM cookies")
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let rows = stmt
            .query_map(rusqlite::params![], |row| {
                let id: String = row.get(0)?;
                let json: String = row.get(1)?;
                Ok((id, json))
            })
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let mut result = Vec::new();
        for row in rows {
            let (id_str, json) = row.map_err(|e| CoreError::Storage(e.to_string()))?;
            let cookies: CookieMap = serde_json::from_str(&json)?;
            result.push((RepoId::<CookieMap>::new(id_str), cookies));
        }
        Ok(result)
    }
}
