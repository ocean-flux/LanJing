//! Event row 到安全 envelope 的映射。

use lj_rule_model::EventEnvelope;
use uuid::Uuid;

use crate::repository::event::{EventRow, from_i64};
use crate::types::{StorageError, StoredEvent};

pub(crate) fn stored_event_from_row(row: EventRow) -> Result<StoredEvent, StorageError> {
    let event_type =
        serde_json::from_str(&row.event_type).map_err(|_| StorageError::Serialization)?;
    let payload =
        serde_json::from_str(&row.payload_json).map_err(|_| StorageError::Serialization)?;
    let artifact_refs =
        serde_json::from_str(&row.artifact_refs_json).map_err(|_| StorageError::Serialization)?;
    let secret_refs =
        serde_json::from_str(&row.secret_refs_json).map_err(|_| StorageError::Serialization)?;
    Ok(StoredEvent {
        source_identity: row.source_identity,
        envelope: EventEnvelope {
            global_seq: from_i64(row.global_seq, "global sequence")?,
            stream_id: row.stream_id,
            stream_version: from_i64(row.stream_version, "stream version")?,
            event_id: parse_uuid(&row.event_id, "event ID")?,
            event_type,
            schema_version: u32::try_from(row.schema_version)
                .map_err(|_| StorageError::InvalidInput("损坏的 schema version".to_string()))?,
            correlation_id: parse_optional_uuid(row.correlation_id)?,
            causation_id: parse_optional_uuid(row.causation_id)?,
            trace_id: row.trace_id,
            occurred_at: row.occurred_at_ms.to_string(),
            payload,
            artifact_refs,
            secret_refs,
        },
    })
}

fn parse_uuid(value: &str, field: &str) -> Result<Uuid, StorageError> {
    Uuid::parse_str(value).map_err(|_| StorageError::InvalidInput(format!("损坏的 {field}")))
}

fn parse_optional_uuid(value: Option<String>) -> Result<Option<Uuid>, StorageError> {
    value.map(|value| parse_uuid(&value, "UUID")).transpose()
}
