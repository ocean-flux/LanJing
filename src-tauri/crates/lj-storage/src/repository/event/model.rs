//! Event repository rows 与 `SQLite` value 转换。

use sea_orm::FromQueryResult;
use serde::Serialize;

use crate::types::StorageError;

pub(crate) fn ensure_blake3_hash(value: &str, name: &str) -> Result<(), StorageError> {
    if value.len() == 64 && value.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        Ok(())
    } else {
        Err(StorageError::InvalidInput(format!(
            "{name} 必须是 BLAKE3 hex"
        )))
    }
}

pub(crate) fn serialize<T: Serialize>(value: &T) -> Result<String, StorageError> {
    serde_json::to_string(value).map_err(|_| StorageError::Serialization)
}

pub(crate) fn deserialize<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, StorageError> {
    serde_json::from_slice(bytes).map_err(|_| StorageError::Serialization)
}

pub(crate) fn database_error(error: impl std::fmt::Display) -> StorageError {
    StorageError::Database(error.to_string())
}

impl From<sea_orm::DbErr> for StorageError {
    fn from(error: sea_orm::DbErr) -> Self {
        database_error(error)
    }
}

pub(crate) fn to_i64(value: u64) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::InvalidInput("整数超出 SQLite 范围".to_string()))
}

pub(crate) fn from_i64(value: i64, field: &str) -> Result<u64, StorageError> {
    u64::try_from(value).map_err(|_| StorageError::InvalidInput(format!("{field} 为负数")))
}

pub(crate) fn now_millis() -> i64 {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

#[derive(FromQueryResult)]
pub(crate) struct I64Value {
    pub(crate) value: i64,
}

#[derive(FromQueryResult)]
pub(crate) struct TextValue {
    pub(crate) value: String,
}

#[derive(FromQueryResult)]
pub(crate) struct EventRow {
    pub(crate) global_seq: i64,
    pub(crate) stream_id: String,
    pub(crate) stream_version: i64,
    pub(crate) event_id: String,
    pub(crate) source_identity: Option<String>,
    pub(crate) event_type: String,
    pub(crate) schema_version: i32,
    pub(crate) correlation_id: Option<String>,
    pub(crate) causation_id: Option<String>,
    pub(crate) trace_id: String,
    pub(crate) occurred_at_ms: i64,
    pub(crate) payload_json: String,
    pub(crate) artifact_refs_json: String,
    pub(crate) secret_refs_json: String,
}

#[derive(FromQueryResult)]
pub(crate) struct ArtifactPathRow {
    pub(crate) relative_path: String,
}

#[derive(FromQueryResult)]
pub(crate) struct ArtifactRow {
    pub(crate) hash: String,
    pub(crate) codec: String,
    pub(crate) relative_path: String,
}

#[derive(FromQueryResult)]
pub(crate) struct ArtifactReferenceRow {
    pub(crate) hash: String,
    pub(crate) artifact_kind: String,
}
