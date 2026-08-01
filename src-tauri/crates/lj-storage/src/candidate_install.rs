//! Candidate/source 的 crate-private 查询入口。
//!
//! SQL 与 row mapping 由 `repository::candidate_source` 拥有；写操作统一经
//! `transaction::candidate`。

pub(crate) use crate::repository::candidate_source::{
    get_candidate_summary, get_installed_source_sync, list_installed_sources_sync,
};
