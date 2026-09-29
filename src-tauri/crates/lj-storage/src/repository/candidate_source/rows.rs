#[derive(FromQueryResult)]
struct CandidateIdRow {
    candidate_id: String,
}

#[derive(FromQueryResult)]
struct CandidateStagedEventRow {
    stream_version: i64,
    event_id: String,
    source_identity: Option<String>,
    event_type: String,
    schema_version: i32,
    payload_json: String,
    artifact_refs_json: String,
    secret_refs_json: String,
}

#[derive(FromQueryResult)]
struct CandidateRow {
    candidate_schema_version: i32,
    runtime_credential_secret_id: Option<String>,
    target_source_identity: String,
    expected_installed_revision: i64,
    package_artifact_hash: String,
    plan_artifact_hash: String,
    definition_hash: String,
    plan_hash: String,
    profile_json: String,
    required_grant_json: String,
    diagnostics_json: String,
    expires_at_ms: i64,
    status: String,
}

#[derive(FromQueryResult)]
pub(crate) struct SourceRow {
    pub(crate) source_identity: String,
    pub(crate) version: String,
    pub(crate) profile_json: String,
    pub(crate) grant_json: String,
    pub(crate) package_artifact_hash: String,
    pub(crate) plan_artifact_hash: String,
    pub(crate) revision: i64,
}

#[derive(FromQueryResult)]
struct InstalledSourceRecordRow {
    source_identity: String,
    version: String,
    profile_json: String,
    grant_json: String,
    revision: i64,
}

#[derive(FromQueryResult)]
struct InstalledSourceVersionRow {
    source_identity: String,
    source_revision: i64,
    version: String,
    profile_json: String,
    grant_json: String,
    package_artifact_hash: String,
    plan_artifact_hash: String,
}

#[derive(FromQueryResult)]
pub(crate) struct SourceRevisionRow {
    pub(crate) source_identity: String,
    pub(crate) source_revision: i64,
    pub(crate) version: String,
    pub(crate) profile_json: String,
    pub(crate) grant_json: String,
    pub(crate) base_url: String,
    pub(crate) package_artifact_hash: String,
    pub(crate) plan_artifact_hash: String,
    pub(crate) definition_hash: String,
    pub(crate) plan_hash: String,
    pub(crate) cookie_namespace: String,
    pub(crate) runtime_credential_secret_id: Option<String>,
    pub(crate) schema_version: i64,
    pub(crate) installed_at_ms: i64,
}
