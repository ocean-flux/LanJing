//! 按 aggregate 组织的数据库读写 owner。

pub(crate) mod archive;
pub(crate) mod candidate_source;
pub(crate) mod event;
pub(crate) mod execution;
pub(crate) mod maintenance;
pub(crate) mod projection;
pub(crate) mod secret;
