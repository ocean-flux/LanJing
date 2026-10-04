//! native rule document 的作者生命周期。
//!
//! create/save/validate/prepare/list/get/rename/delete/provenance 全部以
//! `EventProjectionStorage` 的 document 投影为唯一真相；本模块不接触 legacy 编辑链，
//! 也不为 legacy 输入创建文档。凭证值只经 secret artifact 保存，manifest 只保留 slot
//! summary；`prepare` 复用现有 `stage_prepared_candidate` 链路。

use std::collections::{BTreeMap, HashMap};

use lj_capability::{IntentExport, StandardIntent};
use lj_compiler::{CompilerError, canonicalize, validate};
use lj_rule_model::{
    CapabilityManifest, ControlledMapper, DiagnosticSeverity, ExtractRule, ExtractSpec,
    ExtractType, FlowEdge, FlowGraph, FlowNode, FlowNodeConfig, FlowPortRef, HttpMethod, HttpSpec,
    LINEAR_INPUT_HANDLE, LINEAR_OUTPUT_HANDLE, MapperOutputKind, OutputTarget, PolicyCapabilities,
    RequestHeaderDisposition, RuleDefinition, SensitiveNamePolicy, SourceIdentity,
    SystemCapabilities, definition_hash,
};
use lj_storage::{
    CreateDocumentRequest, DeleteDocumentRequest, DocumentCredentialMutation,
    DocumentCredentialMutationAction, DocumentDetail, DocumentInitial, DomainSaveOutcome,
    LayoutSaveInput, RenameDocumentRequest, RestoreDocumentRevisionRequest,
    SaveDocumentOutcome as StorageSaveOutcome, SaveDocumentRequest,
    SemanticActivation as StorageSemanticActivation, SemanticSaveInput, SemanticSnapshot,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::super::error_mapping::{compiler_error, storage_error};
use super::super::{RuleSystem, now_millis, trace_id};
use super::prepare_install::source_profile;
use crate::{
    CreateMode, CreateNativeRuleDocumentRequest, CredentialMutationAction,
    CredentialMutationRequest, DeleteNativeRuleDocumentRequest, DomainOutcome, ExpectedDataType,
    GetNativeRuleDocumentRequest, GetNativeRuleProvenanceRequest, LayoutSave,
    NativeRuleDocumentDetail, NativeRuleDocumentSummary, NativeRuleProvenanceView,
    NativeRuleRevisionSummary, ProvenanceSummaryView, RenameNativeRuleDocumentRequest,
    RestoreNativeRuleRevisionOutcome, RestoreNativeRuleRevisionRequest, RevisionConflict,
    RuleError, RuleErrorStage, SaveNativeRuleDocumentOutcome, SaveNativeRuleDocumentRequest,
    SemanticActivation, SemanticSave, ValidateNativeRuleDocumentPreview,
    ValidateNativeRuleDocumentRequest,
};

/// credential manifest 的稳定 schema 版本。
const CREDENTIAL_MANIFEST_SCHEMA_VERSION: u32 = 1;

/// 文档凭证 manifest：只保存 slot summary；值只存在于 secret artifact。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CredentialManifest {
    /// manifest schema 版本。
    schema_version: u32,
    /// 凭证槽位列表；声明顺序不承载语义。
    #[serde(default)]
    slots: Vec<CredentialSlot>,
}

/// 一个凭证槽位：指向 Definition HTTP 节点 `headers` 字段的命名 slot。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CredentialSlot {
    /// 逻辑名称（敏感名称策略归一化后唯一）。
    logical_name: String,
    /// 目标 HTTP 节点 ID。
    node_id: Uuid,
    /// 指向 `/headers/<name>` 的 JSON Pointer。
    json_pointer: String,
    /// secret artifact ID（owner=本文档槽位）。
    secret_id: String,
    /// 槽位写入时间（UTC epoch 毫秒）。
    created_at_ms: i64,
}

impl CredentialManifest {
    /// 创建空 manifest。
    #[must_use]
    fn empty() -> Self {
        Self {
            schema_version: CREDENTIAL_MANIFEST_SCHEMA_VERSION,
            slots: Vec::new(),
        }
    }

    /// 解析存储的 manifest JSON；无法解析或 schema 不匹配视为持久化损坏。
    fn parse(json: &str, trace_id: &str) -> Result<Self, RuleError> {
        let manifest: CredentialManifest = serde_json::from_str(json).map_err(|_| {
            RuleError::new(
                RuleErrorStage::Persistence,
                "document_manifest_corrupt",
                "文档凭证 manifest 无法解析",
                trace_id,
                false,
                Vec::new(),
            )
        })?;
        if manifest.schema_version != CREDENTIAL_MANIFEST_SCHEMA_VERSION {
            return Err(RuleError::new(
                RuleErrorStage::Persistence,
                "document_manifest_corrupt",
                "文档凭证 manifest schema 版本不受支持",
                trace_id,
                false,
                Vec::new(),
            ));
        }
        Ok(manifest)
    }

    /// 序列化为 manifest JSON。
    fn serialize(&self, trace_id: &str) -> Result<String, RuleError> {
        serde_json::to_string(self).map_err(|_| {
            RuleError::new(
                RuleErrorStage::Internal,
                "document_manifest_serialization_failed",
                "文档凭证 manifest 无法序列化",
                trace_id,
                false,
                Vec::new(),
            )
        })
    }

    /// 按 (`node_id`, `json_pointer`) 定位槽位；同指针视为同一槽位（replace 原位更新）。
    #[must_use]
    fn slot_by_pointer(&self, node_id: Uuid, json_pointer: &str) -> Option<&CredentialSlot> {
        self.slots
            .iter()
            .find(|slot| slot.node_id == node_id && slot.json_pointer == json_pointer)
    }

    fn slot_index_by_pointer(&self, node_id: Uuid, json_pointer: &str) -> Option<usize> {
        self.slots
            .iter()
            .position(|slot| slot.node_id == node_id && slot.json_pointer == json_pointer)
    }

    /// 是否已存在归一化等价的逻辑名称。
    #[must_use]
    fn has_equivalent_logical_name(&self, logical_name: &str) -> bool {
        self.slots
            .iter()
            .any(|slot| SensitiveNamePolicy::equivalent(&slot.logical_name, logical_name))
    }
}

impl RuleSystem {
    /// 创建 blank、template 或 imported 形态的 native rule document。
    ///
    /// blank：空 `FlowGraph` + 空 intent export，语义可保存但不可编译（有缺意图/节点诊断）。
    /// template：最小 Http→Extract→Mapper 骨架 + 单个标准意图导出，可保存且可编译。
    /// import：导入 current Definition，但把来源身份重置为新文档身份，避免跨文档冲突。
    ///
    /// # Errors
    ///
    /// 模板参数无效、Definition 序列化/hash 失败或存储创建失败时返回 `RuleError`。
    pub async fn create_native_rule_document(
        &self,
        request: CreateNativeRuleDocumentRequest,
    ) -> Result<NativeRuleDocumentSummary, RuleError> {
        let trace_id = trace_id();
        let occurred_at_ms = now_millis(&trace_id)?;
        let document_uuid = Uuid::new_v4();
        let document_id = document_uuid.to_string();
        let generated_source_identity = SourceIdentity {
            id: format!("native:{document_uuid}"),
        };
        let (definition, title) = match request.mode {
            CreateMode::Blank => (
                RuleDefinition::new(
                    generated_source_identity,
                    "",
                    BTreeMap::new(),
                    FlowGraph {
                        nodes: Vec::new(),
                        edges: Vec::new(),
                    },
                    CapabilityManifest::default(),
                    Vec::new(),
                ),
                String::new(),
            ),
            CreateMode::Template {
                title,
                intent,
                data_type,
                base_url,
            } => (
                template_definition(
                    &generated_source_identity,
                    &base_url,
                    intent,
                    data_type,
                    &trace_id,
                )?,
                title,
            ),
            CreateMode::Import {
                title,
                mut definition,
            } => {
                *definition.source_identity_mut() = generated_source_identity.clone();
                (definition, title)
            }
        };
        let definition = canonicalize(&definition);
        let diagnostics = validate(&definition);
        let definition_json = serde_json::to_string(&definition).map_err(|_| {
            RuleError::new(
                RuleErrorStage::Internal,
                "definition_serialization_failed",
                "Definition 无法序列化为 canonical JSON",
                &trace_id,
                false,
                Vec::new(),
            )
        })?;
        let definition_hash = definition_hash(&definition).map_err(|_| {
            RuleError::new(
                RuleErrorStage::Internal,
                "definition_hash_failed",
                "Definition canonical hash 计算失败",
                &trace_id,
                false,
                Vec::new(),
            )
        })?;
        let manifest_json = CredentialManifest::empty().serialize(&trace_id)?;
        let semantic_snapshot = SemanticSnapshot {
            revision: 1,
            definition_json,
            definition_hash,
            manifest_json,
        };
        let effective_semantic = (!diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error))
        .then(|| semantic_snapshot.clone());
        let summary = self
            .state
            .storage
            .create_native_rule_document(CreateDocumentRequest {
                document_id,
                format: "native_rule".to_string(),
                title,
                source_identity: definition.source_identity().id.clone(),
                initial: DocumentInitial {
                    semantic: Some(semantic_snapshot),
                    effective_semantic,
                    layout: None,
                },
                provenance: None,
                trace_id: trace_id.clone(),
                occurred_at_ms,
            })
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))?;
        Ok(document_summary_from_storage(summary))
    }

    /// 分域保存 native rule document。
    ///
    /// 语义域：canonicalize → validate（诊断不阻断保存）→ 校验一次性凭证变更 →
    /// storage.save。在同一个 writer transaction 内写 secret、更新 manifest、Draft Rule
    /// Revision 与 Effective Rule Revision；revision 冲突按域返回 `conflict`，不阻断另一域。
    ///
    /// # Errors
    ///
    /// 凭证变更校验失败（节点/指针/策略/重复/形状）、Definition 序列化或存储失败时
    /// 返回 `RuleError`。
    pub async fn save_native_rule_document(
        &self,
        request: SaveNativeRuleDocumentRequest,
    ) -> Result<SaveNativeRuleDocumentOutcome, RuleError> {
        let trace_id = trace_id();
        let occurred_at_ms = now_millis(&trace_id)?;

        let semantic = match request.semantic {
            Some(SemanticSave {
                expected_revision,
                definition,
                credential_mutations: requested_credential_mutations,
            }) => {
                let existing_detail = self
                    .state
                    .storage
                    .get_native_rule_document(&request.document_id)
                    .await
                    .map_err(|error| {
                        storage_error(&error, RuleErrorStage::Persistence, &trace_id)
                    })?;
                let current_semantic_revision = existing_detail
                    .as_ref()
                    .and_then(|detail| detail.semantic.as_ref())
                    .map(|snapshot| snapshot.revision);
                let mut manifest = existing_detail
                    .and_then(|detail| detail.semantic)
                    .map(|snapshot| CredentialManifest::parse(&snapshot.manifest_json, &trace_id))
                    .transpose()?
                    .unwrap_or_else(CredentialManifest::empty);
                let definition = canonicalize(&definition);
                let diagnostics = validate(&definition);
                // stale semantic 请求不进入凭证校验/写入；writer 也会再次校验 revision。
                let credential_mutations = if current_semantic_revision == Some(expected_revision) {
                    Self::prepare_credential_mutations(
                        &definition,
                        &mut manifest,
                        requested_credential_mutations,
                        &trace_id,
                    )?
                } else {
                    Vec::new()
                };
                let definition_json = serde_json::to_string(&definition).map_err(|_| {
                    RuleError::new(
                        RuleErrorStage::Internal,
                        "definition_serialization_failed",
                        "Definition 无法序列化为 canonical JSON",
                        &trace_id,
                        false,
                        Vec::new(),
                    )
                })?;
                let definition_hash = definition_hash(&definition).map_err(|_| {
                    RuleError::new(
                        RuleErrorStage::Internal,
                        "definition_hash_failed",
                        "Definition canonical hash 计算失败",
                        &trace_id,
                        false,
                        Vec::new(),
                    )
                })?;
                let activation = if diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
                {
                    StorageSemanticActivation::Draft
                } else {
                    StorageSemanticActivation::Effective
                };
                Some(SemanticSaveInput {
                    expected_revision,
                    definition_json,
                    definition_hash,
                    credential_mutations,
                    activation,
                })
            }
            None => None,
        };
        let layout = request.layout.map(
            |LayoutSave {
                 expected_revision,
                 layout_json,
             }| LayoutSaveInput {
                expected_revision,
                layout_json,
            },
        );
        let outcome = self
            .state
            .storage
            .save_native_rule_document(SaveDocumentRequest {
                document_id: request.document_id,
                semantic,
                layout,
                trace_id: trace_id.clone(),
                occurred_at_ms,
            })
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))?;
        Ok(save_outcome_from_storage(outcome))
    }

    /// 校验已保存的语义快照并返回安全摘要（无 Plan/Graph JSON）。
    ///
    /// # Errors
    ///
    /// 文档不存在、revision 过期、快照损坏或存储读取失败时返回 `RuleError`；Definition
    /// 语义无效时返回 `valid=false` 的安全预览。
    pub async fn validate_native_rule_document(
        &self,
        request: ValidateNativeRuleDocumentRequest,
    ) -> Result<ValidateNativeRuleDocumentPreview, RuleError> {
        let trace_id = trace_id();
        let detail = self
            .read_semantic_snapshot(&request.document_id, request.revision, &trace_id)
            .await?;
        let semantic = detail.semantic.as_ref().ok_or_else(|| {
            RuleError::new(
                RuleErrorStage::Persistence,
                "document_semantic_missing",
                "文档没有语义快照",
                trace_id.clone(),
                false,
                Vec::new(),
            )
        })?;
        let definition = read_snapshot_definition(semantic, &trace_id)?;
        let diagnostics = validate(&definition);
        let capability = definition.capability_manifest().required.clone();
        let valid = !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error);
        if !valid {
            return Ok(ValidateNativeRuleDocumentPreview {
                revision: semantic.revision,
                definition_hash: semantic.definition_hash.clone(),
                valid: false,
                plan_hash: None,
                diagnostics,
                profile: None,
                capability,
            });
        }

        let plan = match self.state.compiler.compile(&definition) {
            Ok(plan) => plan,
            Err(CompilerError::Validation { diagnostics }) => {
                return Ok(ValidateNativeRuleDocumentPreview {
                    revision: semantic.revision,
                    definition_hash: semantic.definition_hash.clone(),
                    valid: false,
                    plan_hash: None,
                    diagnostics,
                    profile: None,
                    capability,
                });
            }
            Err(error) => return Err(compiler_error(&error, &trace_id)),
        };
        let profile = source_profile(
            &definition,
            plan.definition_hash(),
            Some(detail.summary.title.clone()),
            None,
        );
        Ok(ValidateNativeRuleDocumentPreview {
            revision: semantic.revision,
            definition_hash: semantic.definition_hash.clone(),
            valid: true,
            plan_hash: Some(plan.plan_hash().to_string()),
            diagnostics,
            profile: Some(profile),
            capability,
        })
    }

    /// 列出全部 native rule documents（安全摘要）。
    ///
    /// # Errors
    ///
    /// 存储读取失败时返回 `RuleError`。
    pub async fn list_native_rule_documents(
        &self,
    ) -> Result<Vec<NativeRuleDocumentSummary>, RuleError> {
        let trace_id = trace_id();
        let summaries = self
            .state
            .storage
            .list_native_rule_documents()
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))?;
        Ok(summaries
            .into_iter()
            .map(document_summary_from_storage)
            .collect())
    }

    /// 列出文档的 Effective Rule Revision 历史安全摘要。
    ///
    /// # Errors
    ///
    /// 存储读取失败时返回 `RuleError`。
    pub async fn list_native_rule_revision_history(
        &self,
        request: GetNativeRuleDocumentRequest,
    ) -> Result<Vec<NativeRuleRevisionSummary>, RuleError> {
        let trace_id = trace_id();
        let history = self
            .state
            .storage
            .list_native_rule_revision_history(request.document_id)
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))?;
        Ok(history
            .into_iter()
            .map(native_rule_revision_summary_from_storage)
            .collect())
    }

    /// 从 Effective 历史创建新的 Draft Rule Revision，不直接替换 Effective。
    ///
    /// # Errors
    ///
    /// 文档或历史 revision 不存在、系统时钟不合法或存储写入失败时返回 `RuleError`；
    /// Draft revision 冲突以 `conflict` 字段返回，不是错误。
    pub async fn restore_native_rule_revision(
        &self,
        request: RestoreNativeRuleRevisionRequest,
    ) -> Result<RestoreNativeRuleRevisionOutcome, RuleError> {
        let trace_id = trace_id();
        let occurred_at_ms = now_millis(&trace_id)?;
        let outcome = self
            .state
            .storage
            .restore_native_rule_revision(RestoreDocumentRevisionRequest {
                document_id: request.document_id,
                revision: request.revision,
                expected_revision: request.expected_revision,
                trace_id: trace_id.clone(),
                occurred_at_ms,
            })
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))?;
        Ok(RestoreNativeRuleRevisionOutcome {
            document_id: outcome.document_id,
            revision: outcome.revision,
            conflict: outcome.conflict.map(|conflict| RevisionConflict {
                expected: conflict.expected,
                current: conflict.current,
            }),
        })
    }

    /// 获取单个 native rule document 详情。
    ///
    /// # Errors
    ///
    /// 存储读取失败时返回 `RuleError`；文档不存在返回 `Ok(None)`。
    pub async fn get_native_rule_document(
        &self,
        request: GetNativeRuleDocumentRequest,
    ) -> Result<Option<NativeRuleDocumentDetail>, RuleError> {
        let trace_id = trace_id();
        let detail = self
            .state
            .storage
            .get_native_rule_document(&request.document_id)
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))?;
        let Some(detail) = detail else {
            return Ok(None);
        };
        Ok(Some(document_detail_from_storage(detail, &trace_id)?))
    }

    /// 重命名 native rule document 的展示标题。
    ///
    /// 标题不进入 Definition/hash，也不推进语义 revision。
    ///
    /// # Errors
    ///
    /// 文档不存在或 revision 冲突时返回 `RuleError`。
    pub async fn rename_native_rule_document(
        &self,
        request: RenameNativeRuleDocumentRequest,
    ) -> Result<NativeRuleDocumentSummary, RuleError> {
        let trace_id = trace_id();
        let occurred_at_ms = now_millis(&trace_id)?;
        let summary = self
            .state
            .storage
            .rename_native_rule_document(RenameDocumentRequest {
                document_id: request.document_id,
                title: request.title,
                expected_revision: request.expected_revision,
                trace_id: trace_id.clone(),
                occurred_at_ms,
            })
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))?;
        Ok(document_summary_from_storage(summary))
    }

    /// 删除 native rule document。
    ///
    /// 文档为 `linked` 状态且未显式确认时返回 `document_linked_delete_guard`；
    /// 存储层删除时同时清理 secret owner ref-count。
    ///
    /// # Errors
    ///
    /// linked 删除未确认或存储删除失败时返回 `RuleError`。
    pub async fn delete_native_rule_document(
        &self,
        request: DeleteNativeRuleDocumentRequest,
    ) -> Result<(), RuleError> {
        let trace_id = trace_id();
        if !request.confirm_linked {
            let linked = self
                .state
                .storage
                .get_native_rule_document(&request.document_id)
                .await
                .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))?
                .is_some_and(|detail| detail.summary.state == "linked");
            if linked {
                return Err(RuleError::new(
                    RuleErrorStage::Persistence,
                    "document_linked_delete_guard",
                    "已链接来源的文档需要显式确认删除",
                    &trace_id,
                    false,
                    Vec::new(),
                ));
            }
        }
        let occurred_at_ms = now_millis(&trace_id)?;
        self.state
            .storage
            .delete_native_rule_document(DeleteDocumentRequest {
                document_id: request.document_id,
                confirm_linked: request.confirm_linked,
                trace_id: trace_id.clone(),
                occurred_at_ms,
            })
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))?;
        Ok(())
    }

    /// 获取 native rule document 的 provenance 只读视图（原文已脱敏）。
    ///
    /// # Errors
    ///
    /// 存储读取失败时返回 `RuleError`；无 provenance 或文档不存在返回 `Ok(None)`。
    pub async fn get_native_rule_provenance(
        &self,
        request: GetNativeRuleProvenanceRequest,
    ) -> Result<Option<NativeRuleProvenanceView>, RuleError> {
        let trace_id = trace_id();
        let Some(detail) = self
            .state
            .storage
            .get_native_rule_document(&request.document_id)
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))?
        else {
            return Ok(None);
        };
        let Some(provenance) = detail.provenance else {
            return Ok(None);
        };
        let Some(source_text) = self
            .state
            .storage
            .get_native_rule_provenance_text(&request.document_id)
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))?
        else {
            return Ok(None);
        };
        Ok(Some(NativeRuleProvenanceView {
            format: provenance.format,
            adapter_version: provenance.adapter_version,
            input_hash: provenance.input_hash,
            diagnostics: serde_json::from_str(&provenance.diagnostics_json).unwrap_or_default(),
            imported_at_ms: provenance.imported_at_ms,
            masked_text: mask_provenance_text(&source_text),
        }))
    }

    /// 读取并校验语义快照：文档必须存在、必须已有语义、revision 必须匹配。
    async fn read_semantic_snapshot(
        &self,
        document_id: &str,
        expected_revision: i64,
        trace_id: &str,
    ) -> Result<DocumentDetail, RuleError> {
        let detail = self
            .state
            .storage
            .get_native_rule_document(document_id)
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, trace_id))?;
        let detail = detail.ok_or_else(|| {
            RuleError::new(
                RuleErrorStage::Persistence,
                "document_not_found",
                "文档不存在",
                trace_id,
                false,
                Vec::new(),
            )
        })?;
        if detail.semantic.is_none() {
            return Err(RuleError::new(
                RuleErrorStage::Persistence,
                "document_not_found",
                "文档还没有已保存的语义快照",
                trace_id,
                false,
                Vec::new(),
            ));
        }
        let semantic = detail.semantic.as_ref().expect("上一分支保证语义快照存在");
        if semantic.revision != expected_revision {
            return Err(RuleError::new(
                RuleErrorStage::Persistence,
                "document_revision_conflict",
                "文档语义 revision 已过期，需要重新读取",
                trace_id,
                false,
                Vec::new(),
            ));
        }
        Ok(detail)
    }

    /// 校验一次性凭证变更，并生成只能进入 writer transaction 的 transport。
    ///
    /// 每个变更按序：节点存在 → Http 节点 → pointer 指向已声明 header → 敏感名称策略 →
    /// 批内/已有槽位重复 → 生成 mutation transport。明文值只在此和 writer transaction
    /// 短暂存在，绝不进入 Definition、manifest、日志或错误信息。
    fn prepare_credential_mutations(
        definition: &RuleDefinition,
        manifest: &mut CredentialManifest,
        mutations: Vec<CredentialMutationRequest>,
        trace_id: &str,
    ) -> Result<Vec<DocumentCredentialMutation>, RuleError> {
        for (index, mutation) in mutations.iter().enumerate() {
            let duplicated = mutations[..index].iter().any(|earlier| {
                SensitiveNamePolicy::equivalent(&earlier.logical_name, &mutation.logical_name)
            });
            if duplicated {
                return Err(RuleError::new(
                    RuleErrorStage::Validation,
                    "credential_duplicate_sensitive_name",
                    "同一批凭证变更包含重复的敏感名称",
                    trace_id,
                    false,
                    Vec::new(),
                ));
            }
        }
        let mut prepared = Vec::with_capacity(mutations.len());
        for mutation in mutations {
            let CredentialMutationRequest {
                node_id,
                json_pointer,
                logical_name,
                action,
                value,
            } = mutation;
            let header = resolve_credential_header(definition, node_id, &json_pointer, trace_id)?;
            enforce_credential_policy(&header, trace_id)?;
            match action {
                CredentialMutationAction::Replace => {
                    let value = value.ok_or_else(|| {
                        RuleError::new(
                            RuleErrorStage::Validation,
                            "credential_mutation_invalid",
                            "replace 变更必须携带一次性值",
                            trace_id,
                            false,
                            Vec::new(),
                        )
                    })?;
                    if manifest.slot_by_pointer(node_id, &json_pointer).is_none()
                        && manifest.has_equivalent_logical_name(&logical_name)
                    {
                        return Err(RuleError::new(
                            RuleErrorStage::Validation,
                            "credential_duplicate_sensitive_name",
                            "凭证敏感名称与已有槽位重复",
                            trace_id,
                            false,
                            Vec::new(),
                        ));
                    }
                    let slot = CredentialSlot {
                        logical_name: logical_name.clone(),
                        node_id,
                        json_pointer: json_pointer.clone(),
                        secret_id: String::new(),
                        created_at_ms: 0,
                    };
                    match manifest.slot_index_by_pointer(slot.node_id, &slot.json_pointer) {
                        Some(index) => manifest.slots[index] = slot,
                        None => manifest.slots.push(slot),
                    }
                    prepared.push(DocumentCredentialMutation {
                        node_id: node_id.to_string(),
                        json_pointer,
                        logical_name,
                        action: DocumentCredentialMutationAction::Replace,
                        value: Some(value),
                    });
                }
                CredentialMutationAction::Clear => {
                    if value.is_some() {
                        return Err(RuleError::new(
                            RuleErrorStage::Validation,
                            "credential_mutation_invalid",
                            "clear 变更不能携带值",
                            trace_id,
                            false,
                            Vec::new(),
                        ));
                    }
                    if let Some(index) = manifest.slot_index_by_pointer(node_id, &json_pointer) {
                        manifest.slots.remove(index);
                    }
                    prepared.push(DocumentCredentialMutation {
                        node_id: node_id.to_string(),
                        json_pointer,
                        logical_name,
                        action: DocumentCredentialMutationAction::Clear,
                        value: None,
                    });
                }
            }
        }
        Ok(prepared)
    }
}

/// 校验并解析一个凭证变更的 header 目标，返回 header 名。
fn resolve_credential_header(
    definition: &RuleDefinition,
    node_id: Uuid,
    json_pointer: &str,
    trace_id: &str,
) -> Result<String, RuleError> {
    let node = definition
        .flow()
        .nodes
        .iter()
        .find(|node| node.id == node_id)
        .ok_or_else(|| {
            RuleError::new(
                RuleErrorStage::Validation,
                "credential_node_not_found",
                "凭证变更指向不存在的节点",
                trace_id,
                false,
                Vec::new(),
            )
        })?;
    let FlowNodeConfig::Http(spec) = &node.config else {
        return Err(RuleError::new(
            RuleErrorStage::Validation,
            "credential_pointer_invalid",
            "凭证变更只支持 HTTP 节点",
            trace_id,
            false,
            Vec::new(),
        ));
    };
    let header = json_pointer
        .strip_prefix("/headers/")
        .filter(|name| !name.is_empty() && !name.contains('/'))
        .ok_or_else(|| {
            RuleError::new(
                RuleErrorStage::Validation,
                "credential_pointer_invalid",
                "凭证指针必须指向 /headers/<name>",
                trace_id,
                false,
                Vec::new(),
            )
        })?;
    if !spec.headers.contains_key(header) {
        return Err(RuleError::new(
            RuleErrorStage::Validation,
            "credential_pointer_invalid",
            "凭证指针指向的 header 未在 HTTP 节点声明",
            trace_id,
            false,
            Vec::new(),
        ));
    }
    Ok(header.to_string())
}

/// 校验 header 名满足敏感名称策略：只允许 Credential 处置（Public/Blocked 一律拒绝）。
fn enforce_credential_policy(header: &str, trace_id: &str) -> Result<(), RuleError> {
    match SensitiveNamePolicy::request_header_disposition(header) {
        RequestHeaderDisposition::Credential => Ok(()),
        RequestHeaderDisposition::Public | RequestHeaderDisposition::Blocked => {
            Err(RuleError::new(
                RuleErrorStage::Validation,
                "credential_policy_denied",
                "凭证目标字段不满足敏感名称策略",
                trace_id,
                false,
                Vec::new(),
            ))
        }
    }
}

/// 从快照 JSON 读取 Definition，并校验存储 hash 与重算 hash 一致。
fn read_snapshot_definition(
    snapshot: &SemanticSnapshot,
    trace_id: &str,
) -> Result<RuleDefinition, RuleError> {
    let definition = lj_rule_model::read_rule_definition(snapshot.definition_json.as_bytes())
        .map_err(|_| {
            RuleError::new(
                RuleErrorStage::Persistence,
                "document_semantic_corrupt",
                "文档语义快照 Definition 无法解析",
                trace_id,
                false,
                Vec::new(),
            )
        })?;
    let computed = definition_hash(&definition).map_err(|_| {
        RuleError::new(
            RuleErrorStage::Internal,
            "definition_hash_failed",
            "Definition canonical hash 计算失败",
            trace_id,
            false,
            Vec::new(),
        )
    })?;
    if computed != snapshot.definition_hash {
        return Err(RuleError::new(
            RuleErrorStage::Persistence,
            "document_semantic_corrupt",
            "文档语义快照 hash 与内容不一致",
            trace_id,
            false,
            Vec::new(),
        ));
    }
    Ok(definition)
}

/// 构造最小 Http→Extract→Mapper 骨架 Definition。
///
/// 节点 ID 由来源身份与角色派生，保证同一模板的 compiler 产物可复现（与 importer 一致）；
/// `source_id_rules` 复用意图对应 identity 字段，满足 compiler 的稳定 ID 规则要求。
fn template_definition(
    source_identity: &SourceIdentity,
    base_url: &str,
    intent: StandardIntent,
    data_type: ExpectedDataType,
    trace_id: &str,
) -> Result<RuleDefinition, RuleError> {
    let base_url = base_url.trim();
    if base_url.is_empty() {
        return Err(RuleError::new(
            RuleErrorStage::Validation,
            "document_template_invalid",
            "模板需要非空 base_url",
            trace_id,
            false,
            Vec::new(),
        ));
    }
    let expected_type = model_expected_type(data_type);
    let (mapper_output, identity_fields) = template_mapper_contract(intent);
    let http_id = template_node_id(source_identity, "http");
    let extract_id = template_node_id(source_identity, "extract");
    let mapper_id = template_node_id(source_identity, "mapper");
    let nodes = vec![
        FlowNode::new(
            http_id,
            FlowNodeConfig::Http(HttpSpec {
                method: HttpMethod::Get,
                url: base_url.to_string(),
                headers: HashMap::new(),
                body: None,
                charset: None,
                expected_type,
            }),
        ),
        FlowNode::new(
            extract_id,
            FlowNodeConfig::Extract(ExtractSpec {
                rules: vec![template_extract_rule(expected_type)],
                field_rules: HashMap::new(),
                expected_type,
                output_target: OutputTarget::Media,
            }),
        ),
        FlowNode::new(
            mapper_id,
            FlowNodeConfig::Mapper(ControlledMapper {
                output: mapper_output,
                identity_fields: identity_fields
                    .iter()
                    .map(|field| (*field).to_string())
                    .collect(),
            }),
        ),
    ];
    let edges = vec![
        FlowEdge::new(
            FlowPortRef::new(http_id, LINEAR_OUTPUT_HANDLE),
            FlowPortRef::new(extract_id, LINEAR_INPUT_HANDLE),
        ),
        FlowEdge::new(
            FlowPortRef::new(extract_id, LINEAR_OUTPUT_HANDLE),
            FlowPortRef::new(mapper_id, LINEAR_INPUT_HANDLE),
        ),
    ];
    let intent_exports = BTreeMap::from([(intent, IntentExport::new(http_id, mapper_id))]);
    Ok(RuleDefinition::new(
        source_identity.clone(),
        base_url,
        intent_exports,
        FlowGraph { nodes, edges },
        CapabilityManifest {
            required: PolicyCapabilities {
                network: true,
                system: SystemCapabilities::default(),
            },
        },
        identity_fields
            .iter()
            .map(|field| (*field).to_string())
            .collect(),
    ))
}

/// 标准意图 → 模板 Mapper 输出类型与 identity 字段（复用 `mapper_vocab` 常量）。
fn template_mapper_contract(intent: StandardIntent) -> (MapperOutputKind, &'static [&'static str]) {
    use lj_rule_model::mapper_vocab::{
        ASSET_IDENTITY_FIELDS, DISCOVERY_ACTION_IDENTITY_FIELDS, DISCOVERY_MEDIA_IDENTITY_FIELDS,
        ITEM_IDENTITY_FIELDS, UNIT_IDENTITY_FIELDS,
    };
    match intent {
        StandardIntent::Search | StandardIntent::ResolveItem => {
            (MapperOutputKind::Items, ITEM_IDENTITY_FIELDS)
        }
        StandardIntent::Discover => (MapperOutputKind::Discovery, DISCOVERY_MEDIA_IDENTITY_FIELDS),
        StandardIntent::ListUnits => (MapperOutputKind::Units, UNIT_IDENTITY_FIELDS),
        StandardIntent::ResolveAsset => (MapperOutputKind::Assets, ASSET_IDENTITY_FIELDS),
        StandardIntent::ContinueAction => (
            MapperOutputKind::Discovery,
            DISCOVERY_ACTION_IDENTITY_FIELDS,
        ),
    }
}

/// 数据类型的占位提取规则（可解析的最小规则，保证骨架可编译）。
fn template_extract_rule(expected_type: lj_rule_model::ExpectedDataType) -> ExtractRule {
    match expected_type {
        lj_rule_model::ExpectedDataType::Html => ExtractRule::CssSelector {
            selector: "body".to_string(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        },
        lj_rule_model::ExpectedDataType::Xml => ExtractRule::XPath {
            expression: "/".to_string(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        },
        lj_rule_model::ExpectedDataType::Json => ExtractRule::JsonPath {
            path: "$".to_string(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        },
    }
}

/// 派生可复现的模板节点 ID（与 importer 的稳定节点派生方式一致）。
fn template_node_id(source_identity: &SourceIdentity, role: &str) -> Uuid {
    let digest = blake3::hash(format!("native-template:{}:{role}", source_identity.id).as_bytes());
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest.as_bytes()[..16]);
    Uuid::from_bytes(bytes)
}

/// DTO `ExpectedDataType` → model `ExpectedDataType`。
fn model_expected_type(data_type: ExpectedDataType) -> lj_rule_model::ExpectedDataType {
    match data_type {
        ExpectedDataType::Html => lj_rule_model::ExpectedDataType::Html,
        ExpectedDataType::Xml => lj_rule_model::ExpectedDataType::Xml,
        ExpectedDataType::Json => lj_rule_model::ExpectedDataType::Json,
    }
}

/// storage `DocumentSummary` → façade 摘要。
fn document_summary_from_storage(
    summary: lj_storage::DocumentSummary,
) -> NativeRuleDocumentSummary {
    NativeRuleDocumentSummary {
        document_id: summary.document_id,
        format: summary.format,
        title: summary.title,
        source_identity: summary.source_identity,
        state: summary.state,
        semantic_revision: summary.semantic_revision,
        layout_revision: summary.layout_revision,
        link_revision: summary.link_revision,
        created_at_ms: summary.created_at_ms,
        updated_at_ms: summary.updated_at_ms,
    }
}

/// storage `DocumentDetail` → façade 详情。
fn document_detail_from_storage(
    detail: DocumentDetail,
    trace_id: &str,
) -> Result<NativeRuleDocumentDetail, RuleError> {
    let semantic_revision = detail
        .semantic
        .as_ref()
        .map_or(detail.summary.semantic_revision, |snapshot| {
            snapshot.revision
        });
    let effective_semantic_revision = detail
        .effective_semantic
        .as_ref()
        .map(|snapshot| snapshot.revision);
    let layout_revision = detail
        .layout
        .as_ref()
        .map_or(detail.summary.layout_revision, |snapshot| snapshot.revision);
    let definition = detail
        .semantic
        .as_ref()
        .map(|snapshot| {
            serde_json::from_str(&snapshot.definition_json).map_err(|_| {
                RuleError::new(
                    RuleErrorStage::Persistence,
                    "document_definition_invalid",
                    "文档语义 Definition 无法读取",
                    trace_id,
                    false,
                    Vec::new(),
                )
            })
        })
        .transpose()?;
    let layout_json = detail.layout.map(|snapshot| snapshot.layout_json);
    Ok(NativeRuleDocumentDetail {
        summary: document_summary_from_storage(detail.summary),
        semantic_revision,
        effective_semantic_revision,
        effective_summary: detail
            .effective_summary
            .map(native_rule_revision_summary_from_storage),
        layout_revision,
        definition,
        layout_json,
        provenance: detail.provenance.map(provenance_view_from_storage),
    })
}

fn native_rule_revision_summary_from_storage(
    summary: lj_storage::RuleRevisionHistoryRecord,
) -> NativeRuleRevisionSummary {
    NativeRuleRevisionSummary {
        revision: summary.revision,
        definition_hash: summary.definition_hash,
        effective_at_ms: summary.effective_at_ms,
    }
}

/// storage `ProvenanceSummary` → façade 摘要视图。
fn provenance_view_from_storage(
    provenance: lj_storage::ProvenanceSummary,
) -> ProvenanceSummaryView {
    ProvenanceSummaryView {
        format: provenance.format,
        adapter_version: provenance.adapter_version,
        input_hash: provenance.input_hash,
        diagnostics: serde_json::from_str(&provenance.diagnostics_json).unwrap_or_default(),
        imported_at_ms: provenance.imported_at_ms,
    }
}

/// storage `SaveDocumentOutcome` → façade 结果。
fn save_outcome_from_storage(outcome: StorageSaveOutcome) -> SaveNativeRuleDocumentOutcome {
    SaveNativeRuleDocumentOutcome {
        document_id: outcome.document_id,
        semantic: outcome.semantic.map(domain_outcome_from_storage),
        layout: outcome.layout.map(domain_outcome_from_storage),
    }
}

/// storage `DomainSaveOutcome` → façade 单域结果。
fn domain_outcome_from_storage(outcome: DomainSaveOutcome) -> DomainOutcome {
    DomainOutcome {
        revision: outcome.revision,
        conflict: outcome.conflict.map(|conflict| RevisionConflict {
            expected: conflict.expected,
            current: conflict.current,
        }),
        activation: outcome.activation.map(|activation| match activation {
            StorageSemanticActivation::Draft => SemanticActivation::Draft,
            StorageSemanticActivation::Effective => SemanticActivation::Effective,
        }),
    }
}

/// 对 provenance 原文做只读脱敏：header 风格行与 URL query 的敏感参数值替换为固定掩码。
///
/// 启发式处理：按行识别 `Name: value`，按 `?`/`&`/`=` 识别 query 参数名；不保证任意
/// 格式完备，只保证输出不携带敏感值。
fn mask_provenance_text(text: &str) -> String {
    const MASK: &str = "***";
    text.lines()
        .map(|line| {
            let masked = mask_header_line(line, MASK);
            mask_url_query(&masked, MASK)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 把 `Name: value` 行中敏感名称的值替换为掩码；非敏感行原样返回。
fn mask_header_line(line: &str, mask: &str) -> String {
    let Some((name, _)) = line.split_once(':') else {
        return line.to_string();
    };
    let name = name.trim().trim_matches('"').trim();
    if !SensitiveNamePolicy::is_sensitive(name) {
        return line.to_string();
    }
    format!("{name}: {mask}")
}

/// 把 URL query 中敏感参数名的值替换为掩码。
fn mask_url_query(line: &str, mask: &str) -> String {
    let Some(question) = line.find('?') else {
        return line.to_string();
    };
    let (prefix, query_and_fragment) = line.split_at(question + 1);
    let (query, fragment) = query_and_fragment
        .split_once('#')
        .map_or((query_and_fragment, ""), |(query, fragment)| {
            (query, fragment)
        });
    let masked = query
        .split('&')
        .map(|part| {
            let Some((name, _)) = part.split_once('=') else {
                return part.to_string();
            };
            let name = percent_decode_ascii(name);
            if SensitiveNamePolicy::is_sensitive(&name) {
                format!("{name}={mask}")
            } else {
                part.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("&");
    if fragment.is_empty() {
        format!("{prefix}{masked}")
    } else {
        format!("{prefix}{masked}#{fragment}")
    }
}

/// ASCII percent-decode（仅用于参数名判断）。
fn percent_decode_ascii(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = String::with_capacity(value.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
        {
            let byte = high * 16 + low;
            if byte.is_ascii() {
                decoded.push(char::from(byte));
                index += 3;
                continue;
            }
        }
        let character = value[index..]
            .chars()
            .next()
            .expect("index 停留在 UTF-8 边界");
        decoded.push(character);
        index += character.len_utf8();
    }
    decoded
}

const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keyring_core::{mock, set_default_store};
    use lj_compiler::Compiler;
    use lj_rule_model::ExpectedDataType as ModelExpectedDataType;

    /// 模板骨架必须能被 compiler 编译（无 Error 级诊断）。
    #[test]
    fn template_definitions_compile_for_all_intents_and_data_types() {
        let source = SourceIdentity {
            id: "native:template-compile-test".to_string(),
        };
        for intent in [
            StandardIntent::Search,
            StandardIntent::Discover,
            StandardIntent::ResolveItem,
            StandardIntent::ListUnits,
            StandardIntent::ResolveAsset,
            StandardIntent::ContinueAction,
        ] {
            for data_type in [
                ExpectedDataType::Html,
                ExpectedDataType::Xml,
                ExpectedDataType::Json,
            ] {
                let definition = template_definition(
                    &source,
                    "https://example.test",
                    intent,
                    data_type,
                    "trace-template-compile",
                )
                .expect("模板构造成功");
                let compiler = Compiler::default();
                let plan = compiler.compile(&definition).unwrap_or_else(|error| {
                    panic!("{intent:?}/{data_type:?} 模板编译失败: {error:?}")
                });
                assert!(!plan.plan_hash().is_empty());
            }
        }
    }

    /// 空 `base_url` 模板被拒绝。
    #[test]
    fn template_rejects_empty_base_url() {
        let source = SourceIdentity {
            id: "native:template-empty-base-url".to_string(),
        };
        let error = template_definition(
            &source,
            "   ",
            StandardIntent::Search,
            ExpectedDataType::Json,
            "trace-template-empty",
        )
        .expect_err("空 base_url 必须失败");
        assert_eq!(error.code, "document_template_invalid");
    }

    /// 凭证 pointer 解析：仅接受指向已声明 header 的 `/headers/<name>`。
    #[test]
    fn credential_header_resolution_rules() {
        let node_id = Uuid::from_u128(9_001);
        let definition = RuleDefinition::new(
            SourceIdentity {
                id: "native:credential-test".to_string(),
            },
            "https://example.test",
            BTreeMap::from([(StandardIntent::Search, IntentExport::new(node_id, node_id))]),
            FlowGraph {
                nodes: vec![FlowNode::new(
                    node_id,
                    FlowNodeConfig::Http(HttpSpec {
                        method: HttpMethod::Get,
                        url: "https://example.test/search?q={{key}}".to_string(),
                        headers: HashMap::from([("Authorization".to_string(), String::new())]),
                        body: None,
                        charset: None,
                        expected_type: ModelExpectedDataType::Json,
                    }),
                )],
                edges: Vec::new(),
            },
            CapabilityManifest::default(),
            vec!["url".to_string()],
        );
        let trace = "trace-credential-resolution";

        assert_eq!(
            resolve_credential_header(&definition, node_id, "/headers/Authorization", trace)
                .expect("已声明敏感 header 必须解析成功"),
            "Authorization"
        );
        let missing_node = resolve_credential_header(
            &definition,
            Uuid::from_u128(9_999),
            "/headers/Authorization",
            trace,
        )
        .expect_err("不存在的节点必须失败");
        assert_eq!(missing_node.code, "credential_node_not_found");
        let missing_header =
            resolve_credential_header(&definition, node_id, "/headers/X-API-Key", trace)
                .expect_err("未声明的 header 必须失败");
        assert_eq!(missing_header.code, "credential_pointer_invalid");
        let not_pointer = resolve_credential_header(&definition, node_id, "/url", trace)
            .expect_err("非 headers 指针必须失败");
        assert_eq!(not_pointer.code, "credential_pointer_invalid");
    }

    /// 敏感名称策略：只有 Credential 处置的 header 可作为凭证目标。
    #[test]
    fn credential_policy_gate() {
        let trace = "trace-credential-policy";
        assert!(enforce_credential_policy("Authorization", trace).is_ok());
        assert!(enforce_credential_policy("X_API_KEY", trace).is_ok());
        let public = enforce_credential_policy("User-Agent", trace).expect_err("Public 必须拒绝");
        assert_eq!(public.code, "credential_policy_denied");
        let blocked =
            enforce_credential_policy("Proxy-Authorization", trace).expect_err("Blocked 必须拒绝");
        assert_eq!(blocked.code, "credential_policy_denied");
    }

    /// provenance 原文脱敏：敏感 header 行与 URL query 参数值被掩码。
    #[test]
    fn provenance_masking_covers_headers_and_query_params() {
        let text = "GET /api\nAuthorization: Bearer abc123\nX-Api-Key: k\nhttps://example.test/list?page=2&api_key=secret&q=ok";
        let masked = mask_provenance_text(text);
        assert!(!masked.contains("Bearer abc123"));
        assert!(masked.contains("Authorization: ***"));
        assert!(!masked.contains("api_key=secret"));
        assert!(masked.contains("api_key=***"));
        assert!(masked.contains("page=2"));
        assert!(masked.contains("q=ok"));
    }

    /// manifest 合并：同 (`node_id`, `json_pointer`) 视为同一槽位，replace 原位更新。
    #[test]
    fn manifest_slot_identity_is_node_and_pointer() {
        let mut manifest = CredentialManifest::empty();
        let node = Uuid::from_u128(9_100);
        manifest.slots.push(CredentialSlot {
            logical_name: "Authorization".to_string(),
            node_id: node,
            json_pointer: "/headers/Authorization".to_string(),
            secret_id: "secret-1".to_string(),
            created_at_ms: 1,
        });
        assert!(manifest.has_equivalent_logical_name("authorization"));
        assert!(manifest.has_equivalent_logical_name("Authorization"));
        assert!(
            manifest
                .slot_by_pointer(node, "/headers/Authorization")
                .is_some()
        );
        assert!(
            manifest
                .slot_by_pointer(node, "/headers/X-API-Key")
                .is_none()
        );
    }

    // ---- 全链集成测试 ----
    //
    // 依赖并行 storage 切片的 document API（EventProjectionStorage document 投影、
    // write/clear_document_credential_secret、StorageError::DocumentMissing）；
    // 随 storage 落地后由主代理统一运行。

    fn init_mock_keyring() {
        static INIT: std::sync::Once = std::sync::Once::new();
        INIT.call_once(|| {
            set_default_store(mock::Store::new().expect("keyring-core mock store"));
        });
    }

    async fn open_system() -> RuleSystem {
        init_mock_keyring();
        let root = std::env::temp_dir().join(format!("lj-rule-system-document-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create fixture root");
        RuleSystem::open(
            crate::RuleSystemConfig::desktop(root.join("event-store.db"), root.join("artifacts"))
                .with_keyring_service(format!(
                    "lanjing.rule-system.document.test.{}",
                    Uuid::new_v4()
                )),
        )
        .await
        .expect("open RuleSystem")
    }

    /// 无效 Definition 返回安全 preview，且只能形成 Rule Draft Revision。
    #[tokio::test]
    async fn invalid_definition_preview_keeps_draft_only() {
        let system = open_system().await;
        let created = system
            .create_native_rule_document(CreateNativeRuleDocumentRequest {
                mode: CreateMode::Blank,
            })
            .await
            .expect("blank create");
        let invalid = RuleDefinition::new(
            SourceIdentity {
                id: created.source_identity.clone(),
            },
            "",
            BTreeMap::new(),
            FlowGraph {
                nodes: Vec::new(),
                edges: Vec::new(),
            },
            CapabilityManifest::default(),
            Vec::new(),
        );
        let saved = system
            .save_native_rule_document(SaveNativeRuleDocumentRequest {
                document_id: created.document_id.clone(),
                semantic: Some(SemanticSave {
                    expected_revision: 1,
                    definition: invalid,
                    credential_mutations: Vec::new(),
                }),
                layout: None,
            })
            .await
            .expect("invalid draft save is allowed");
        assert_eq!(saved.semantic.expect("semantic outcome").revision, 2);

        let preview = system
            .validate_native_rule_document(ValidateNativeRuleDocumentRequest {
                document_id: created.document_id.clone(),
                revision: 2,
            })
            .await
            .expect("invalid Definition returns preview");
        assert!(!preview.valid);
        assert!(preview.plan_hash.is_none());
        assert!(preview.profile.is_none());
        assert!(!preview.diagnostics.is_empty());
    }

    #[tokio::test]
    async fn invalid_save_preserves_effective_rule_and_records_draft() {
        let system = open_system().await;
        let created = system
            .create_native_rule_document(CreateNativeRuleDocumentRequest {
                mode: CreateMode::Template {
                    title: "生效模板".to_string(),
                    intent: StandardIntent::Search,
                    data_type: ExpectedDataType::Json,
                    base_url: "https://example.test".to_string(),
                },
            })
            .await
            .expect("创建有效模板");

        let before = system
            .get_native_rule_document(GetNativeRuleDocumentRequest {
                document_id: created.document_id.clone(),
            })
            .await
            .expect("读取模板")
            .expect("模板存在");
        assert_eq!(before.effective_semantic_revision, Some(1));

        let invalid = RuleDefinition::new(
            SourceIdentity {
                id: created.source_identity.clone(),
            },
            "",
            BTreeMap::new(),
            FlowGraph {
                nodes: Vec::new(),
                edges: Vec::new(),
            },
            CapabilityManifest::default(),
            Vec::new(),
        );
        let saved = system
            .save_native_rule_document(SaveNativeRuleDocumentRequest {
                document_id: created.document_id.clone(),
                semantic: Some(SemanticSave {
                    expected_revision: 1,
                    definition: invalid,
                    credential_mutations: Vec::new(),
                }),
                layout: None,
            })
            .await
            .expect("无效草稿可以保存");
        assert_eq!(
            saved.semantic.expect("语义域结果").activation,
            Some(SemanticActivation::Draft)
        );

        let detail = system
            .get_native_rule_document(GetNativeRuleDocumentRequest {
                document_id: created.document_id,
            })
            .await
            .expect("读取草稿")
            .expect("草稿存在");
        assert_eq!(detail.semantic_revision, 2);
        assert_eq!(detail.effective_semantic_revision, Some(1));
        drop(system);
    }

    /// 保存一版语义 revision, 返回它落到了 Draft 还是 Effective。
    async fn save_semantic(
        system: &RuleSystem,
        document_id: &str,
        expected_revision: i64,
        definition: RuleDefinition,
    ) -> SemanticActivation {
        system
            .save_native_rule_document(SaveNativeRuleDocumentRequest {
                document_id: document_id.to_string(),
                semantic: Some(SemanticSave {
                    expected_revision,
                    definition,
                    credential_mutations: Vec::new(),
                }),
                layout: None,
            })
            .await
            .expect("保存文档")
            .semantic
            .expect("语义域结果")
            .activation
            .expect("保存必须给出激活结果")
    }

    async fn read_document(system: &RuleSystem, document_id: &str) -> NativeRuleDocumentDetail {
        system
            .get_native_rule_document(GetNativeRuleDocumentRequest {
                document_id: document_id.to_string(),
            })
            .await
            .expect("读取文档")
            .expect("文档必须存在")
    }

    async fn preview_document(
        system: &RuleSystem,
        document_id: &str,
        revision: i64,
    ) -> ValidateNativeRuleDocumentPreview {
        system
            .validate_native_rule_document(ValidateNativeRuleDocumentRequest {
                document_id: document_id.to_string(),
                revision,
            })
            .await
            .expect("校验预览")
    }

    /// 校验失败的 Recovery Draft 仍可继续编辑, 只有改到通过校验才能替换 Effective Rule Revision。
    ///
    /// 相邻用例已证明两端: 无效保存只成为 Draft、合法保存替换 Effective。未证明的是中间那段 ——
    /// 失败草稿自身能不能继续编辑: 失败那一版必须能原样读回, 在它上面再改一版仍然无效时
    /// 仍不得发布, 改到通过校验后才能按草稿 revision 重新发布。否则用户一旦保存失败就会
    /// 丢掉可恢复的编辑起点, 或者一个尚未通过的草稿被静默发布。
    #[tokio::test]
    async fn recovery_draft_stays_editable_and_only_publishes_once_valid() {
        let system = open_system().await;
        let created = system
            .create_native_rule_document(CreateNativeRuleDocumentRequest {
                mode: CreateMode::Template {
                    title: "恢复草稿".to_string(),
                    intent: StandardIntent::Search,
                    data_type: ExpectedDataType::Json,
                    base_url: "https://example.test".to_string(),
                },
            })
            .await
            .expect("创建有效模板");
        let document_id = created.document_id.clone();

        // revision 1 = 通过校验的模板, 成为 Effective Rule Revision。
        let published = read_document(&system, &document_id).await;
        assert_eq!(published.semantic_revision, 1);
        assert_eq!(published.effective_semantic_revision, Some(1));
        let published_definition = published.definition.clone().expect("模板定义存在");
        let published_hash = published
            .effective_summary
            .as_ref()
            .map(|summary| summary.definition_hash.clone());

        // revision 2 = 校验失败的草稿 (缺意图导出)。
        let mut broken = published_definition.clone();
        broken.intent_exports_mut().clear();
        assert_eq!(
            save_semantic(&system, &document_id, 1, broken.clone()).await,
            SemanticActivation::Draft,
            "校验失败只能成为 Draft"
        );

        // 失败那一版必须能原样读回, 否则根本没有可编辑的恢复起点。
        let draft = read_document(&system, &document_id).await;
        assert_eq!(draft.semantic_revision, 2);
        assert_eq!(draft.effective_semantic_revision, Some(1));
        assert_eq!(
            draft
                .effective_summary
                .as_ref()
                .map(|summary| summary.definition_hash.clone()),
            published_hash,
            "校验失败不得动 Effective Rule Revision"
        );
        assert_eq!(draft.definition.as_ref(), Some(&broken));
        assert!(!preview_document(&system, &document_id, 2).await.valid);

        // 在恢复草稿上继续编辑, 但仍然无效: 不得发布, Effective 仍是 revision 1。
        let mut still_broken = draft.definition.clone().expect("恢复草稿定义");
        *still_broken.base_url_mut() = "https://still-not-valid.example.test".to_string();
        assert_eq!(
            save_semantic(&system, &document_id, 2, still_broken.clone()).await,
            SemanticActivation::Draft
        );
        let draft = read_document(&system, &document_id).await;
        assert_eq!(draft.semantic_revision, 3);
        assert_eq!(draft.effective_semantic_revision, Some(1));
        assert_eq!(
            draft.definition.as_ref(),
            Some(&still_broken),
            "继续编辑必须真的落到恢复草稿上"
        );
        assert!(!preview_document(&system, &document_id, 3).await.valid);

        // 把恢复草稿改到通过校验, 再按草稿 revision 重新发布。
        let mut recovered = draft.definition.clone().expect("恢复草稿定义");
        *recovered.intent_exports_mut() = published_definition.intent_exports().clone();
        assert_eq!(
            save_semantic(&system, &document_id, 3, recovered).await,
            SemanticActivation::Effective,
            "修复后的恢复草稿必须能重新发布"
        );
        let republished = read_document(&system, &document_id).await;
        assert_eq!(republished.semantic_revision, 4);
        assert_eq!(republished.effective_semantic_revision, Some(4));
        let recovery_preview = preview_document(&system, &document_id, 4).await;
        assert!(recovery_preview.valid);
        assert!(
            recovery_preview
                .plan_hash
                .as_deref()
                .is_some_and(|hash| !hash.is_empty()),
            "重新发布的 revision 必须能产出 immutable Plan"
        );
        drop(system);
    }

    /// 合法保存会替换当前 Effective Rule Revision。
    #[tokio::test]
    async fn valid_save_replaces_effective_rule() {
        let system = open_system().await;
        let created = system
            .create_native_rule_document(CreateNativeRuleDocumentRequest {
                mode: CreateMode::Template {
                    title: "初始规则".to_string(),
                    intent: StandardIntent::Search,
                    data_type: ExpectedDataType::Json,
                    base_url: "https://example.test".to_string(),
                },
            })
            .await
            .expect("创建模板");
        let mut definition = system
            .get_native_rule_document(GetNativeRuleDocumentRequest {
                document_id: created.document_id.clone(),
            })
            .await
            .expect("读取模板")
            .expect("模板存在")
            .definition
            .expect("模板定义存在");
        *definition.base_url_mut() = "https://next.example.test".to_string();

        let saved = system
            .save_native_rule_document(SaveNativeRuleDocumentRequest {
                document_id: created.document_id.clone(),
                semantic: Some(SemanticSave {
                    expected_revision: 1,
                    definition,
                    credential_mutations: Vec::new(),
                }),
                layout: None,
            })
            .await
            .expect("合法规则可以保存");
        assert_eq!(
            saved.semantic.expect("语义域结果").activation,
            Some(SemanticActivation::Effective)
        );

        let detail = system
            .get_native_rule_document(GetNativeRuleDocumentRequest {
                document_id: created.document_id,
            })
            .await
            .expect("读取生效规则")
            .expect("规则存在");
        assert_eq!(detail.semantic_revision, 2);
        assert_eq!(detail.effective_semantic_revision, Some(2));
        drop(system);
    }

    /// template create 后 validate 即可编译（骨架满足 compiler 全部 Error 级合同）。
    #[tokio::test]
    async fn template_document_compiles_on_validate() {
        let system = open_system().await;
        let created = system
            .create_native_rule_document(CreateNativeRuleDocumentRequest {
                mode: CreateMode::Template {
                    title: "测试模板".to_string(),
                    intent: StandardIntent::Search,
                    data_type: ExpectedDataType::Json,
                    base_url: "https://example.test".to_string(),
                },
            })
            .await
            .expect("template create");
        assert_eq!(created.title, "测试模板");
        let preview = system
            .validate_native_rule_document(ValidateNativeRuleDocumentRequest {
                document_id: created.document_id,
                revision: 1,
            })
            .await
            .expect("模板 validate 必须可编译");
        assert!(preview.valid);
        assert!(
            preview
                .plan_hash
                .as_deref()
                .is_some_and(|hash| !hash.is_empty())
        );
        assert_eq!(
            preview
                .profile
                .as_ref()
                .expect("valid preview profile")
                .supported_intents,
            vec![StandardIntent::Search]
        );
        drop(system);
    }

    /// import 重置来源身份，避免把外部 Definition 的 identity 带入 native 文档。
    #[tokio::test]
    async fn imported_document_uses_new_native_identity() {
        let system = open_system().await;
        let imported = RuleDefinition::new(
            SourceIdentity {
                id: "source:imported".to_string(),
            },
            "",
            BTreeMap::new(),
            FlowGraph {
                nodes: Vec::new(),
                edges: Vec::new(),
            },
            CapabilityManifest::default(),
            Vec::new(),
        );
        let created = system
            .create_native_rule_document(CreateNativeRuleDocumentRequest {
                mode: CreateMode::Import {
                    title: "导入规则".to_string(),
                    definition: imported,
                },
            })
            .await
            .expect("import create");

        assert!(created.source_identity.starts_with("native:"));
        assert_ne!(created.source_identity, "source:imported");
        let detail = system
            .get_native_rule_document(GetNativeRuleDocumentRequest {
                document_id: created.document_id,
            })
            .await
            .expect("get imported document")
            .expect("imported document exists");
        assert_eq!(
            detail
                .definition
                .expect("saved definition")
                .source_identity()
                .id,
            created.source_identity
        );
        drop(system);
    }

    /// 凭证 replace → 原位更新 → clear 全流程成功，revision 单调推进。
    #[tokio::test]
    async fn credential_replace_update_and_clear_roundtrip() {
        let system = open_system().await;
        let created = system
            .create_native_rule_document(CreateNativeRuleDocumentRequest {
                mode: CreateMode::Blank,
            })
            .await
            .expect("blank create");
        let document_id = created.document_id;
        let replace = |value: &str| CredentialMutationRequest {
            node_id: Uuid::from_u128(9_200),
            json_pointer: "/headers/Authorization".to_string(),
            logical_name: "Authorization".to_string(),
            action: CredentialMutationAction::Replace,
            value: Some(value.to_string()),
        };
        let clear = CredentialMutationRequest {
            node_id: Uuid::from_u128(9_200),
            json_pointer: "/headers/Authorization".to_string(),
            logical_name: "Authorization".to_string(),
            action: CredentialMutationAction::Clear,
            value: None,
        };

        let first = save_with_credentials(&system, &document_id, 1, vec![replace("Bearer secret")])
            .await
            .expect("replace 保存成功");
        assert_eq!(first.semantic.as_ref().expect("语义域").revision, 2);
        let second =
            save_with_credentials(&system, &document_id, 2, vec![replace("Bearer rotated")])
                .await
                .expect("原位更新保存成功");
        assert_eq!(second.semantic.as_ref().expect("语义域").revision, 3);

        // 过期 semantic save 不应先解析/执行凭证 mutation；否则会在 conflict 前
        // 清除或写入 secret。用无效目标锁住调用顺序：结果必须仍是分域 conflict。
        let stale = save_with_credentials(
            &system,
            &document_id,
            2,
            vec![CredentialMutationRequest {
                node_id: Uuid::from_u128(9_999),
                json_pointer: "/headers/Authorization".to_string(),
                logical_name: "Authorization".to_string(),
                action: CredentialMutationAction::Clear,
                value: None,
            }],
        )
        .await
        .expect("过期凭证保存必须返回 conflict outcome");
        assert!(stale.semantic.as_ref().expect("语义域").conflict.is_some());

        let third = save_with_credentials(&system, &document_id, 3, vec![clear])
            .await
            .expect("clear 保存成功");
        assert_eq!(third.semantic.as_ref().expect("语义域").revision, 4);
        drop(system);
    }

    /// 凭证变更按合同逐项拒绝：节点缺失/指针无效/策略拒绝/缺值/批内重复/已有槽位重复。
    #[tokio::test]
    async fn credential_mutations_are_rejected_by_rules() {
        let system = open_system().await;
        let created = system
            .create_native_rule_document(CreateNativeRuleDocumentRequest {
                mode: CreateMode::Blank,
            })
            .await
            .expect("blank create");
        let document_id = created.document_id;

        let error = save_with_credentials(
            &system,
            &document_id,
            1,
            vec![CredentialMutationRequest {
                node_id: Uuid::from_u128(9_999),
                json_pointer: "/headers/Authorization".to_string(),
                logical_name: "Authorization".to_string(),
                action: CredentialMutationAction::Replace,
                value: Some("x".to_string()),
            }],
        )
        .await
        .expect_err("不存在的节点必须拒绝");
        assert_eq!(error.code, "credential_node_not_found");

        let error = save_with_credentials(
            &system,
            &document_id,
            1,
            vec![CredentialMutationRequest {
                node_id: Uuid::from_u128(9_200),
                json_pointer: "/headers/User-Agent".to_string(),
                logical_name: "User-Agent".to_string(),
                action: CredentialMutationAction::Replace,
                value: Some("x".to_string()),
            }],
        )
        .await
        .expect_err("Public header 必须被策略拒绝");
        assert_eq!(error.code, "credential_policy_denied");

        let error = save_with_credentials(
            &system,
            &document_id,
            1,
            vec![CredentialMutationRequest {
                node_id: Uuid::from_u128(9_200),
                json_pointer: "/headers/Authorization".to_string(),
                logical_name: "Authorization".to_string(),
                action: CredentialMutationAction::Replace,
                value: None,
            }],
        )
        .await
        .expect_err("replace 缺值必须拒绝");
        assert_eq!(error.code, "credential_mutation_invalid");

        let error = save_with_credentials(
            &system,
            &document_id,
            1,
            vec![
                CredentialMutationRequest {
                    node_id: Uuid::from_u128(9_200),
                    json_pointer: "/headers/Authorization".to_string(),
                    logical_name: "Authorization".to_string(),
                    action: CredentialMutationAction::Replace,
                    value: Some("a".to_string()),
                },
                CredentialMutationRequest {
                    node_id: Uuid::from_u128(9_200),
                    json_pointer: "/headers/X-API-Key".to_string(),
                    logical_name: "authorization".to_string(),
                    action: CredentialMutationAction::Replace,
                    value: Some("b".to_string()),
                },
            ],
        )
        .await
        .expect_err("批内重复敏感名称必须拒绝");
        assert_eq!(error.code, "credential_duplicate_sensitive_name");

        // 与已有槽位（前一次保存写入）的归一化同名重复也必须拒绝。
        save_with_credentials(
            &system,
            &document_id,
            1,
            vec![CredentialMutationRequest {
                node_id: Uuid::from_u128(9_200),
                json_pointer: "/headers/Authorization".to_string(),
                logical_name: "Authorization".to_string(),
                action: CredentialMutationAction::Replace,
                value: Some("first".to_string()),
            }],
        )
        .await
        .expect("首次写入成功");
        let error = save_with_credentials(
            &system,
            &document_id,
            2,
            vec![CredentialMutationRequest {
                node_id: Uuid::from_u128(9_200),
                json_pointer: "/headers/X-API-Key".to_string(),
                logical_name: "Authorization".to_string(),
                action: CredentialMutationAction::Replace,
                value: Some("other".to_string()),
            }],
        )
        .await
        .expect_err("与已有槽位重复必须拒绝");
        assert_eq!(error.code, "credential_duplicate_sensitive_name");
        drop(system);
    }

    /// 过期语义 revision：save 返回分域 conflict，validate 返回 typed 错误。
    #[tokio::test]
    async fn stale_semantic_revision_flows() {
        let system = open_system().await;
        let created = system
            .create_native_rule_document(CreateNativeRuleDocumentRequest {
                mode: CreateMode::Template {
                    title: "冲突模板".to_string(),
                    intent: StandardIntent::Search,
                    data_type: ExpectedDataType::Json,
                    base_url: "https://example.test".to_string(),
                },
            })
            .await
            .expect("template create");
        let document_id = created.document_id.clone();
        let definition = template_definition(
            &SourceIdentity {
                id: created.source_identity,
            },
            "https://example.test/search",
            StandardIntent::Search,
            ExpectedDataType::Json,
            "trace-conflict-save",
        )
        .expect("模板构造成功");
        let outcome = system
            .save_native_rule_document(SaveNativeRuleDocumentRequest {
                document_id: document_id.clone(),
                semantic: Some(SemanticSave {
                    expected_revision: 99,
                    definition,
                    credential_mutations: Vec::new(),
                }),
                layout: None,
            })
            .await
            .expect("过期 revision 保存返回 outcome 而非错误");
        let semantic = outcome.semantic.expect("语义域结果");
        assert!(semantic.conflict.is_some(), "冲突域必须报告 conflict");

        let error = system
            .validate_native_rule_document(ValidateNativeRuleDocumentRequest {
                document_id,
                revision: 42,
            })
            .await
            .expect_err("过期 revision 必须 typed error");
        assert_eq!(error.code, "document_revision_conflict");
        drop(system);
    }

    /// 缺失文档：save/delete 都映射为 `document_not_found`。
    #[tokio::test]
    async fn missing_document_is_not_found() {
        let system = open_system().await;
        let definition = template_definition(
            &SourceIdentity {
                id: "native:missing-save".to_string(),
            },
            "https://example.test",
            StandardIntent::Search,
            ExpectedDataType::Json,
            "trace-missing-save",
        )
        .expect("模板构造成功");
        let error = system
            .save_native_rule_document(SaveNativeRuleDocumentRequest {
                document_id: "missing-document".to_string(),
                semantic: Some(SemanticSave {
                    expected_revision: 0,
                    definition,
                    credential_mutations: Vec::new(),
                }),
                layout: None,
            })
            .await
            .expect_err("缺失文档保存必须失败");
        assert_eq!(error.code, "document_not_found");

        let error = system
            .delete_native_rule_document(DeleteNativeRuleDocumentRequest {
                document_id: "missing-document".to_string(),
                confirm_linked: false,
            })
            .await
            .expect_err("缺失文档删除必须失败");
        assert_eq!(error.code, "document_not_found");
        drop(system);
    }

    /// 重命名与删除：标题不进 Definition；draft 文档直接删除。
    #[tokio::test]
    async fn rename_and_delete_flow() {
        let system = open_system().await;
        let created = system
            .create_native_rule_document(CreateNativeRuleDocumentRequest {
                mode: CreateMode::Template {
                    title: "旧标题".to_string(),
                    intent: StandardIntent::Search,
                    data_type: ExpectedDataType::Json,
                    base_url: "https://example.test".to_string(),
                },
            })
            .await
            .expect("template create");
        let renamed = system
            .rename_native_rule_document(RenameNativeRuleDocumentRequest {
                document_id: created.document_id.clone(),
                title: "新标题".to_string(),
                expected_revision: created.semantic_revision,
            })
            .await
            .expect("rename");
        assert_eq!(renamed.title, "新标题");
        assert_eq!(renamed.semantic_revision, 1, "重命名不推进语义 revision");

        system
            .delete_native_rule_document(DeleteNativeRuleDocumentRequest {
                document_id: created.document_id.clone(),
                confirm_linked: false,
            })
            .await
            .expect("draft 文档直接删除");
        let error = system
            .delete_native_rule_document(DeleteNativeRuleDocumentRequest {
                document_id: created.document_id,
                confirm_linked: false,
            })
            .await
            .expect_err("重复删除必须失败");
        assert_eq!(error.code, "document_not_found");
        drop(system);
    }

    /// 无导入记录时 provenance 为 None。
    #[tokio::test]
    async fn provenance_is_none_without_import() {
        let system = open_system().await;
        let created = system
            .create_native_rule_document(CreateNativeRuleDocumentRequest {
                mode: CreateMode::Blank,
            })
            .await
            .expect("blank create");
        let provenance = system
            .get_native_rule_provenance(GetNativeRuleProvenanceRequest {
                document_id: created.document_id,
            })
            .await
            .expect("读取 provenance");
        assert!(provenance.is_none());
        drop(system);
    }

    /// masked wire：provenance 视图 serde roundtrip + template 请求 wire 形状。
    #[test]
    fn masked_provenance_view_roundtrips_on_wire() {
        let view = NativeRuleProvenanceView {
            format: "legado".to_string(),
            adapter_version: "1.0.0".to_string(),
            input_hash: "deadbeef".to_string(),
            diagnostics: Vec::new(),
            imported_at_ms: 1234,
            masked_text: "Authorization: ***".to_string(),
        };
        let json = serde_json::to_string(&view).expect("序列化");
        assert!(json.contains("\"adapter_version\""));
        assert!(json.contains("\"masked_text\""));
        let back: NativeRuleProvenanceView = serde_json::from_str(&json).expect("反序列化");
        assert_eq!(back, view);
    }

    #[test]
    fn create_template_request_serializes_snake_case_wire() {
        let request = CreateNativeRuleDocumentRequest {
            mode: CreateMode::Template {
                title: "模板".to_string(),
                intent: StandardIntent::Search,
                data_type: ExpectedDataType::Json,
                base_url: "https://example.test".to_string(),
            },
        };
        let json = serde_json::to_value(&request).expect("序列化");
        assert_eq!(json["mode"]["kind"], "template");
        assert_eq!(json["mode"]["data_type"], "json");
        assert_eq!(json["mode"]["intent"], "Search");
        assert_eq!(json["mode"]["base_url"], "https://example.test");
        let blank = serde_json::to_value(CreateNativeRuleDocumentRequest {
            mode: CreateMode::Blank,
        })
        .expect("序列化");
        assert_eq!(blank["mode"]["kind"], "blank");
    }

    async fn save_with_credentials(
        system: &RuleSystem,
        document_id: &str,
        expected_revision: i64,
        mutations: Vec<CredentialMutationRequest>,
    ) -> Result<crate::SaveNativeRuleDocumentOutcome, crate::RuleError> {
        system
            .save_native_rule_document(SaveNativeRuleDocumentRequest {
                document_id: document_id.to_string(),
                semantic: Some(SemanticSave {
                    expected_revision,
                    definition: credential_definition(),
                    credential_mutations: mutations,
                }),
                layout: None,
            })
            .await
    }

    /// 带已声明敏感 header（Authorization/X-API-Key/User-Agent）的可编译 Definition。
    fn credential_definition() -> RuleDefinition {
        let http = Uuid::from_u128(9_200);
        let extract = Uuid::from_u128(9_201);
        let mapper = Uuid::from_u128(9_202);
        RuleDefinition::new(
            SourceIdentity {
                id: "native:credential-chain-test".to_string(),
            },
            "https://example.test",
            BTreeMap::from([(StandardIntent::Search, IntentExport::new(http, mapper))]),
            FlowGraph {
                nodes: vec![
                    FlowNode::new(
                        http,
                        FlowNodeConfig::Http(HttpSpec {
                            method: HttpMethod::Get,
                            url: "https://example.test/search?q={{key}}".to_string(),
                            headers: HashMap::from([
                                ("Authorization".to_string(), String::new()),
                                ("X-API-Key".to_string(), String::new()),
                                ("User-Agent".to_string(), String::new()),
                            ]),
                            body: None,
                            charset: None,
                            expected_type: ModelExpectedDataType::Json,
                        }),
                    ),
                    FlowNode::new(
                        extract,
                        FlowNodeConfig::Extract(ExtractSpec {
                            rules: vec![ExtractRule::JsonPath {
                                path: "$".to_string(),
                                extract_type: ExtractType::Text,
                                regex_clean: None,
                            }],
                            field_rules: HashMap::new(),
                            expected_type: ModelExpectedDataType::Json,
                            output_target: OutputTarget::Media,
                        }),
                    ),
                    FlowNode::new(
                        mapper,
                        FlowNodeConfig::Mapper(ControlledMapper {
                            output: MapperOutputKind::Items,
                            identity_fields: vec!["url".to_string()],
                        }),
                    ),
                ],
                edges: vec![
                    FlowEdge::new(
                        FlowPortRef::new(http, LINEAR_OUTPUT_HANDLE),
                        FlowPortRef::new(extract, LINEAR_INPUT_HANDLE),
                    ),
                    FlowEdge::new(
                        FlowPortRef::new(extract, LINEAR_OUTPUT_HANDLE),
                        FlowPortRef::new(mapper, LINEAR_INPUT_HANDLE),
                    ),
                ],
            },
            CapabilityManifest {
                required: PolicyCapabilities {
                    network: true,
                    system: SystemCapabilities::default(),
                },
            },
            vec!["url".to_string()],
        )
    }
}
