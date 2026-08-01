/// 按 stream revision 有序读取 durable events。
pub(crate) async fn events_after_stream_sync(
    conn: &mut DatabaseSession,
    stream_id: &str,
    after_version: u64,
) -> Result<Vec<StoredEvent>, StorageError> {
    let rows = statement(
        "SELECT global_seq, stream_id, stream_version, event_id, source_identity, event_type, schema_version, correlation_id, causation_id, trace_id, occurred_at_ms, payload_json, artifact_refs_json, secret_refs_json FROM events WHERE stream_id = ? AND stream_version > ? ORDER BY stream_version ASC",
    )
    .bind(stream_id)
    .bind(to_i64(after_version)?)
    .load::<crate::repository::event::EventRow>(conn).await
    .map_err(database_error)?;
    rows.into_iter().map(stored_event_from_row).collect()
}

/// 按 source/global sequence 有序读取 events。
pub(crate) async fn events_after_source_sync(
    conn: &mut DatabaseSession,
    source_identity: &str,
    after_global_seq: u64,
) -> Result<Vec<StoredEvent>, StorageError> {
    let rows = statement(
        "SELECT global_seq, stream_id, stream_version, event_id, source_identity, event_type, schema_version, correlation_id, causation_id, trace_id, occurred_at_ms, payload_json, artifact_refs_json, secret_refs_json FROM events WHERE source_identity = ? AND global_seq > ? ORDER BY global_seq ASC",
    )
    .bind(source_identity)
    .bind(to_i64(after_global_seq)?)
    .load::<crate::repository::event::EventRow>(conn).await
    .map_err(database_error)?;
    rows.into_iter().map(stored_event_from_row).collect()
}

#[derive(FromQueryResult)]
struct JsonRow {
    payload_json: String,
}

#[derive(FromQueryResult)]
struct SourceVersionRow {
    source_identity: String,
    source_revision: i64,
    version: String,
    profile_json: String,
    grant_json: String,
    base_url: String,
    package_artifact_hash: String,
    plan_artifact_hash: String,
    definition_hash: String,
    plan_hash: String,
    cookie_namespace: String,
    runtime_credential_secret_id: Option<String>,
}

#[derive(FromQueryResult)]
struct SourceVersionCredentialRow {
    cookie_namespace: String,
    runtime_credential_secret_id: Option<String>,
}

#[derive(FromQueryResult)]
pub(crate) struct ExecutionRow {
    pub(crate) execution_id: String,
    pub(crate) source_identity: String,
    pub(crate) source_revision: i64,
    pub(crate) plan_hash: String,
    pub(crate) status: String,
    pub(crate) pinned: i32,
    pub(crate) archive_available: i32,
    pub(crate) gc_state: String,
    pub(crate) started_at_ms: i64,
    pub(crate) finished_at_ms: Option<i64>,
    pub(crate) revision: i64,
}

#[derive(FromQueryResult)]
struct ExecutionReplayPinRow {
    execution_id: String,
    source_identity: String,
    source_version: String,
    source_revision: i64,
    plan_hash: String,
    plan_artifact_hash: String,
    archive_available: i32,
    gc_state: String,
}

#[derive(FromQueryResult)]
struct ExecutionSourceRevisionRow {
    source_identity: String,
    source_revision: i64,
}

/// execution aggregate 稳定 Event stream ID。
pub(crate) fn execution_stream_id(execution_id: Uuid) -> String {
    format!("execution/{execution_id}")
}
