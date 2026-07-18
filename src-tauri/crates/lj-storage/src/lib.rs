//! 存储层 crate。
//!
//! 基于 `Diesel + SQLite + embedded migrations` 实现本地持久化，
//! 对外继续提供 `SqliteStorage` / `AsyncStorage` 形状。

pub mod async_storage;
pub mod ids;
pub mod library;
pub mod models;
pub mod repository;
pub mod schema;

pub use async_storage::AsyncStorage;
pub use ids::RepoId;
pub use library::{LibraryEntry, LibraryProgress, LibraryProjection};
pub use repository::CookieMap;
