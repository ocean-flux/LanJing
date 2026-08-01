pub(crate) fn grant_covers(grant: &PolicyCapabilities, required: &PolicyCapabilities) -> bool {
    (!required.network || grant.network)
        && (!required.system.fs || grant.system.fs)
        && (!required.system.env || grant.system.env)
        && (!required.system.process || grant.system.process)
}

/// 读取唯一 current `RulePackage` artifact，并保留 schema 与损坏 JSON 的区别。
///
/// # Errors
///
/// 未知 schema 返回 [`StorageError::ContractSchemaUnsupported`]；其余 JSON/current shape
/// 错误返回 [`StorageError::Serialization`]。storage 不识别历史 `RulePackage` shape。
pub(crate) fn read_rule_package_artifact(bytes: &[u8]) -> Result<RulePackage, StorageError> {
    read_rule_package(bytes).map_err(|error| schema_read_error(&error))
}

/// 读取唯一 current `ExecutionPlan` artifact。
///
/// # Errors
///
/// 未知 schema 返回 [`StorageError::ContractSchemaUnsupported`]；其余 JSON/current shape
/// 错误返回 [`StorageError::Serialization`]。storage 不识别或迁移历史 Plan shape。
pub(crate) fn read_execution_plan_artifact(bytes: &[u8]) -> Result<ExecutionPlan, StorageError> {
    read_execution_plan(bytes).map_err(|error| schema_read_error(&error))
}

fn schema_read_error(error: &SchemaReadError) -> StorageError {
    match error {
        SchemaReadError::SchemaUnsupported { contract, version } => {
            StorageError::ContractSchemaUnsupported {
                contract: *contract,
                version: *version,
            }
        }
        SchemaReadError::Malformed(_)
        | SchemaReadError::ContractMismatch { .. }
        | SchemaReadError::InvalidData { .. } => StorageError::Serialization,
    }
}

fn candidate_contract_artifact_error(error: StorageError) -> StorageError {
    match error {
        StorageError::ContractSchemaUnsupported { .. } => error,
        _ => StorageError::CandidateTampered,
    }
}

pub(crate) fn canonical_plan_hash(plan: &ExecutionPlan) -> Result<String, StorageError> {
    execution_plan_hash(plan).map_err(|error| StorageError::InvalidInput(error.to_string()))
}

pub(crate) fn validate_candidate_package_and_plan(
    package: &RulePackage,
    plan: &ExecutionPlan,
) -> Result<(), StorageError> {
    let expected_definition_hash = definition_hash(package.definition())
        .map_err(|error| StorageError::InvalidInput(error.to_string()))?;
    if expected_definition_hash != plan.definition_hash()
        || plan.plan_hash() != canonical_plan_hash(plan)?
        || package.source_identity() != package.definition().source_identity()
    {
        return Err(StorageError::InvalidInput(
            "candidate package/Plan hash 不一致".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn candidate_stream_id(candidate_id: Uuid) -> String {
    format!("candidate/{candidate_id}")
}

pub(crate) fn source_stream_id(source_identity: &str) -> String {
    format!("source/{source_identity}")
}

pub(crate) fn source_cookie_namespace(source_identity: &str) -> String {
    format!("source/{source_identity}")
}

pub(crate) fn source_version_owner_id(source_identity: &str, source_revision: u64) -> String {
    format!("{source_identity}:{source_revision}")
}

