//! Rule Package 文件的 ingest preflight 与已安装来源的导出。
//!
//! preflight 只读取 metadata、稳定身份与 schema，并报告 canonical Definition hash、校验诊断与
//! 引用未安装能力的节点；不 staging、不安装、不编译。未安装能力的 payload 可以展示与保存，
//! 但不能 validate/compile/execute。导出复用安装时落盘的同一份 canonical package。

use lj_compiler::validate;
use lj_rule_model::{
    FlowNodeConfig, RuleDefinition, RulePackage, definition_hash, read_rule_package,
};
use lj_storage::StorageError;

use super::super::{
    RuleSystem,
    error_mapping::{schema_read_error, storage_error},
    trace_id,
};
use crate::error::{RuleError, RuleErrorStage};
use crate::types::{RulePackageInspection, SourceId, UnavailableNodeSummary};

impl RuleSystem {
    /// 读取并校验任意 Rule Package 文件。
    ///
    /// # Errors
    ///
    /// JSON 无效、contract tag 不匹配、未知 schema、字段未知、内嵌 Definition 非法或
    /// package 身份与内嵌 Definition 不一致时返回带稳定 code 的 [`RuleError`]。
    pub fn inspect_rule_package(&self, bytes: &[u8]) -> Result<RulePackageInspection, RuleError> {
        let trace_id = trace_id();
        inspect(&read_package(bytes, &trace_id)?, &trace_id)
    }

    /// 导出已安装来源当前 revision 的 canonical Rule Package JSON。
    ///
    /// 导出的是安装时落盘的 immutable package，因此可以重新导入或分发给其他设备。
    ///
    /// # Errors
    ///
    /// 来源尚未安装、package artifact 读取失败或 package 无法序列化时返回带稳定 code 的
    /// [`RuleError`]。
    pub async fn export_rule_package(&self, source_id: SourceId) -> Result<String, RuleError> {
        let trace_id = trace_id();
        let installed = self
            .state
            .storage
            .get_installed_source(source_id.as_identity())
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Candidate, &trace_id))?
            .ok_or_else(|| {
                storage_error(
                    &StorageError::SourceMissing,
                    RuleErrorStage::Candidate,
                    &trace_id,
                )
            })?;
        serde_json::to_string(&installed.package).map_err(|_| {
            RuleError::new(
                RuleErrorStage::Internal,
                "package_export_failed",
                "Rule Package 序列化失败",
                &trace_id,
                false,
                Vec::new(),
            )
        })
    }
}

/// 读取 package 并收敛 `lj-rule-model` 的稳定合同错误。
pub(super) fn read_package(bytes: &[u8], trace_id: &str) -> Result<RulePackage, RuleError> {
    read_rule_package(bytes).map_err(|error| schema_read_error(&error, trace_id))
}

fn inspect(package: &RulePackage, trace_id: &str) -> Result<RulePackageInspection, RuleError> {
    let definition = package.definition();
    Ok(RulePackageInspection {
        source_id: SourceId::from_identity(package.source_identity().id.clone()),
        version: package.version().to_string(),
        definition_hash: definition_hash(definition).map_err(|_| {
            RuleError::new(
                RuleErrorStage::Internal,
                "definition_hash_failed",
                "Definition canonical hash 计算失败",
                trace_id,
                false,
                Vec::new(),
            )
        })?,
        diagnostics: validate(definition),
        unavailable_nodes: unavailable_nodes(definition),
    })
}

/// 列出引用未安装能力的节点；只暴露能力 kind，不暴露 payload。
fn unavailable_nodes(definition: &RuleDefinition) -> Vec<UnavailableNodeSummary> {
    definition
        .flow()
        .nodes
        .iter()
        .filter_map(|node| match &node.config {
            FlowNodeConfig::Unavailable(config) => Some(UnavailableNodeSummary {
                node_id: node.id,
                kind: config.kind.clone(),
            }),
            _ => None,
        })
        .collect()
}
