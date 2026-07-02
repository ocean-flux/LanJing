//! 存储层 crate。
//!
//! 基于 SQLite（rusqlite）实现数据持久化，提供仓库（Repository）模式
//! 的数据访问接口，以及数据库 schema 管理与迁移。

pub mod async_storage;
pub mod repository;
pub mod schema;

pub use async_storage::AsyncStorage;
pub use repository::CookieMap;
