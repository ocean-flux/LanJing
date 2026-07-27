//! 来源文档 authoring codec 与 `EventProjectionStorage` 的唯一组合边界。
//!
//! 默认 query 只返回 storage-owned masked DTO。完整原文、manifest 与 slot plaintext 只在
//! create/save/credential mutation 的当前调用栈中短暂存在，并在进入 bounded writer 前由
//! `CredentialSlotCodec` 严格校验、重组和重新签发 revision-bound sentinel。

use std::collections::{BTreeMap, BTreeSet, HashMap};

use lj_importer::authoring::{
    CredentialCodecError, CredentialRebaseResolution, CredentialSecret, CredentialSlotCodec,
    CredentialSplit,
};
use lj_rule_model::{AuthoringDiagnostic, CredentialTargetIdentity};
use lj_storage::{
    CreateSourceDocumentInput, CredentialSlotMaterial, DeleteSourceDocumentInput,
    EditSourceDocumentCredentialInput, LoadSourceDocumentRebaseMaterialInput,
    MAX_SOURCE_DOCUMENT_BYTES, PinSourceDocumentRevisionInput, RebaseSourceDocumentInput,
    RenameSourceDocumentInput, RevealedSourceDocumentCredential, SaveSourceDocumentInput,
    SourceDocumentMaterial, SourceDocumentRebaseCommitMode, SourceDocumentRebaseInvalidReason,
    SourceDocumentRebaseMaterialOutcome, SourceDocumentRevisionInput, StorageError,
};
use uuid::Uuid;

use super::error_mapping::storage_error;
use super::{RuleSystem, now_millis};
use crate::{
    ClearSourceDocumentCredentialRequest, CreateSourceDocumentRequest, DeleteSourceDocumentRequest,
    DocumentMutationOutcome, DocumentRef, DocumentValidationIssue, GetSourceDocumentRequest,
    ListSourceDocumentsRequest, MaskedSourceDocument, PinSourceDocumentRevisionRequest,
    RebaseSourceDocumentRequest, ReleaseSourceDocumentRevisionPinRequest,
    RenameSourceDocumentRequest, ReplaceSourceDocumentCredentialRequest,
    RevealSourceDocumentCredentialRequest, RuleError, RuleErrorStage, SaveSourceDocumentRequest,
    SourceDocumentCredentialReveal, SourceDocumentFormat, SourceDocumentId,
    SourceDocumentRevisionPin, SourceDocumentRevisionPinReleaseOutcome, SourceDocumentSummary,
};

enum DocumentMaterialLoad {
    Found(SourceDocumentMaterial),
    Missing,
    Outcome(DocumentMutationOutcome),
}

enum CredentialEdit {
    Replace,
    Clear,
}

type LoadedRebaseMaterials =
    Result<(SourceDocumentMaterial, SourceDocumentMaterial), DocumentMutationOutcome>;
type RebaseOutcomeResult<T> = Result<T, Box<DocumentMutationOutcome>>;

const SOURCE_DOCUMENT_REVISION_PIN_TTL_MS: i64 = 30 * 60 * 1_000;

impl RuleSystem {
    /// 按最近更新时间读取安全来源文档摘要。
    ///
    /// # Errors
    ///
    /// storage read lane 或 projection 解码失败时返回 [`RuleError`]。
    pub async fn list_source_documents(
        &self,
        _request: ListSourceDocumentsRequest,
    ) -> Result<Vec<SourceDocumentSummary>, RuleError> {
        let trace_id = super::trace_id();
        self.state
            .storage
            .list_source_documents()
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))
    }

    /// 读取一个默认 masked 来源文档；credential plaintext 永远不进入该返回值。
    ///
    /// # Errors
    ///
    /// storage read lane、secret artifact 解密或 projection 解码失败时返回 [`RuleError`]。
    pub async fn get_source_document(
        &self,
        request: GetSourceDocumentRequest,
    ) -> Result<Option<MaskedSourceDocument>, RuleError> {
        let trace_id = super::trace_id();
        self.state
            .storage
            .get_source_document(request.document_id)
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))
    }

    /// 校验、split 并原子创建来源文档 revision `1`。
    ///
    /// codec/limit/title 输入失败返回 tagged [`DocumentMutationOutcome::Invalid`]，不会创建 secret
    /// artifact；只有 storage 基础设施失败才返回 [`RuleError`]。
    ///
    /// # Errors
    ///
    /// storage writer、SQLite、文件系统或 secure store 基础设施失败时返回 [`RuleError`]。
    pub async fn create_source_document(
        &self,
        request: CreateSourceDocumentRequest,
    ) -> Result<DocumentMutationOutcome, RuleError> {
        let trace_id = super::trace_id();
        let title = request.title.trim().to_string();
        if title.is_empty() {
            return Ok(invalid_outcome(DocumentValidationIssue::new(
                "document_title_invalid",
                "来源文档标题不能为空",
            )));
        }
        if let Some(outcome) = document_size_outcome(&request.text) {
            return Ok(outcome);
        }

        let document_id = SourceDocumentId::new();
        let target = credential_target(request.format, document_id, 1);
        let split = match CredentialSlotCodec::split(&request.text, target) {
            Ok(split) => split,
            Err(error) => return Ok(codec_invalid_outcome(error)),
        };
        let revision = revision_input(request.format, request.text, split);
        let result = self
            .state
            .storage
            .create_source_document(CreateSourceDocumentInput {
                document_id,
                title,
                revision,
                created_at_ms: now_millis(&trace_id)?,
            })
            .await;
        document_mutation_result(result, &trace_id)
    }

    /// 将当前 masked 编辑文本严格恢复后，为下一 revision 重新签发全部 sentinel 并原子保存。
    ///
    /// stale expected revision 返回 typed conflict；任意 sentinel/owner/path/limit 失败返回 typed
    /// invalid，均不会覆盖当前文档。
    ///
    /// # Errors
    ///
    /// storage read/writer、SQLite、文件系统或 secure store 基础设施失败时返回 [`RuleError`]。
    pub async fn save_source_document(
        &self,
        request: SaveSourceDocumentRequest,
    ) -> Result<DocumentMutationOutcome, RuleError> {
        let trace_id = super::trace_id();
        if request.expected_revision == 0 {
            return Ok(revision_invalid_outcome());
        }
        if let Some(outcome) = document_size_outcome(&request.masked_text) {
            return Ok(outcome);
        }
        let document_ref = DocumentRef {
            document_id: request.document_id,
            document_revision: request.expected_revision,
        };
        let material = match self.load_document_material(document_ref, &trace_id).await? {
            DocumentMaterialLoad::Found(material) => material,
            DocumentMaterialLoad::Missing => {
                return self
                    .missing_document_outcome(
                        request.document_id,
                        request.expected_revision,
                        &trace_id,
                    )
                    .await;
            }
            DocumentMaterialLoad::Outcome(outcome) => return Ok(outcome),
        };
        let (document_ref, format, _masked_text, old_raw_text, manifest, credentials) =
            material.into_parts();
        drop(old_raw_text);
        let expected_target = credential_target(
            format,
            document_ref.document_id,
            document_ref.document_revision,
        );
        let credentials = credential_secrets(credentials);
        let raw_text = match CredentialSlotCodec::reconstitute(
            &request.masked_text,
            &manifest,
            &credentials,
            &expected_target,
        ) {
            Ok(text) => text,
            Err(error) => return Ok(codec_invalid_outcome(error)),
        };
        if let Some(outcome) = document_size_outcome(&raw_text) {
            return Ok(outcome);
        }
        let Some(next_revision) = document_ref.document_revision.checked_add(1) else {
            return Ok(revision_invalid_outcome());
        };
        let split = match CredentialSlotCodec::split(
            &raw_text,
            credential_target(format, document_ref.document_id, next_revision),
        ) {
            Ok(split) => split,
            Err(error) => return Ok(codec_invalid_outcome(error)),
        };
        let revision = revision_input(format, raw_text, split);
        let result = self
            .state
            .storage
            .save_source_document(SaveSourceDocumentInput {
                document_id: document_ref.document_id,
                expected_revision: document_ref.document_revision,
                revision,
                saved_at_ms: now_millis(&trace_id)?,
            })
            .await;
        document_mutation_result(result, &trace_id)
    }

    /// 固定一个 current saved revision 的全部 secret refs，供冲突 rebase 使用。
    ///
    /// # Errors
    ///
    /// document 缺失、revision 已漂移、pin 时间溢出，或 storage owner/密文验证失败时返回安全
    /// [`RuleError`]；不会把 secret ref 或 plaintext 暴露给 wire。
    pub async fn pin_source_document_revision(
        &self,
        request: PinSourceDocumentRevisionRequest,
    ) -> Result<SourceDocumentRevisionPin, RuleError> {
        let trace_id = super::trace_id();
        if request.document_revision == 0 {
            return Err(revision_pin_rule_error(
                "revision_pin_revision_invalid",
                "来源文档 revision pin 参数无效",
                &trace_id,
            ));
        }
        let created_at_ms = now_millis(&trace_id)?;
        let expires_at_ms = created_at_ms
            .checked_add(SOURCE_DOCUMENT_REVISION_PIN_TTL_MS)
            .ok_or_else(|| {
                revision_pin_rule_error(
                    "revision_pin_expiry_invalid",
                    "来源文档 revision pin 到期时间无效",
                    &trace_id,
                )
            })?;
        self.state
            .storage
            .pin_source_document_revision(PinSourceDocumentRevisionInput {
                pin_id: uuid::Uuid::new_v4(),
                document_ref: DocumentRef {
                    document_id: request.document_id,
                    document_revision: request.document_revision,
                },
                created_at_ms,
                expires_at_ms,
            })
            .await
            .map_err(|error| pin_storage_error(&error, &trace_id))
    }

    /// 幂等释放 revision pin；未知、已释放或已过期 pin 都返回同一 released outcome。
    ///
    /// # Errors
    ///
    /// storage owner/ref-count transaction 失败时返回安全 [`RuleError`]。
    pub async fn release_source_document_revision_pin(
        &self,
        request: ReleaseSourceDocumentRevisionPinRequest,
    ) -> Result<SourceDocumentRevisionPinReleaseOutcome, RuleError> {
        let trace_id = super::trace_id();
        self.state
            .storage
            .release_source_document_revision_pin(request.pin_id)
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))?;
        Ok(SourceDocumentRevisionPinReleaseOutcome::Released {
            pin_id: request.pin_id,
        })
    }

    /// 从可信 pin/current material 执行 path-only credential resolution，并原子 merge 或 fork。
    ///
    /// pin、base/current owner、path union、重复/缺失 resolution、旧 sentinel 和 current drift 均
    /// 硬失败；replacement plaintext 只在本次调用栈和 bounded writer 中短暂存在。
    ///
    /// # Errors
    ///
    /// storage read/writer、SQLite、文件系统或 secure store 基础设施失败时返回 [`RuleError`]；预期
    /// pin、codec、resolution 与并发失败返回 tagged [`DocumentMutationOutcome`]。
    pub async fn rebase_source_document(
        &self,
        request: RebaseSourceDocumentRequest,
    ) -> Result<DocumentMutationOutcome, RuleError> {
        let trace_id = super::trace_id();
        let RebaseSourceDocumentRequest {
            pin_id,
            document_id,
            base_revision,
            current_revision,
            local_masked_text,
            mode,
            credential_resolutions,
        } = request;
        if base_revision == 0 || current_revision == 0 {
            return Ok(revision_invalid_outcome());
        }
        if let Some(outcome) = document_size_outcome(&local_masked_text) {
            return Ok(outcome);
        }
        let materials = self
            .load_rebase_materials(
                pin_id,
                DocumentRef {
                    document_id,
                    document_revision: base_revision,
                },
                current_revision,
                &trace_id,
            )
            .await?;
        let (base, current) = match materials {
            Ok(materials) => materials,
            Err(outcome) => return Ok(outcome),
        };
        let (target_document_id, target_revision, commit_mode) =
            match rebase_commit_target(mode, document_id, current_revision) {
                Ok(target) => target,
                Err(outcome) => return Ok(*outcome),
            };
        let revision = match prepare_rebased_revision(
            base,
            current,
            &local_masked_text,
            credential_resolutions,
            target_document_id,
            target_revision,
        ) {
            Ok(revision) => revision,
            Err(outcome) => return Ok(*outcome),
        };
        let result = self
            .state
            .storage
            .rebase_source_document(RebaseSourceDocumentInput {
                pin_id,
                document_id,
                base_revision,
                current_revision,
                mode: commit_mode,
                revision,
                saved_at_ms: now_millis(&trace_id)?,
            })
            .await;
        document_mutation_result(result, &trace_id)
    }

    async fn load_rebase_materials(
        &self,
        pin_id: Uuid,
        document_ref: DocumentRef,
        current_revision: u64,
        trace_id: &str,
    ) -> Result<LoadedRebaseMaterials, RuleError> {
        let result = self
            .state
            .storage
            .load_source_document_rebase_material(LoadSourceDocumentRebaseMaterialInput {
                pin_id,
                document_ref,
                current_revision,
                now_ms: now_millis(trace_id)?,
            })
            .await;
        match result {
            Ok(SourceDocumentRebaseMaterialOutcome::Ready(materials)) => {
                Ok(Ok((*materials).into_parts()))
            }
            Ok(SourceDocumentRebaseMaterialOutcome::Invalid(reason)) => {
                Ok(Err(rebase_pin_invalid_outcome(reason)))
            }
            Ok(SourceDocumentRebaseMaterialOutcome::Conflict {
                expected_revision,
                actual_revision,
                current,
            }) => Ok(Err(DocumentMutationOutcome::Conflict {
                expected_revision,
                actual_revision,
                current,
            })),
            Ok(SourceDocumentRebaseMaterialOutcome::NotFound) => {
                Ok(Err(DocumentMutationOutcome::NotFound))
            }
            Err(error) => document_security_outcome(&error).map_or_else(
                || Err(storage_error(&error, RuleErrorStage::Persistence, trace_id)),
                |outcome| Ok(Err(outcome)),
            ),
        }
    }

    /// 在 expected revision 仍为 current 时重命名来源文档。
    ///
    /// # Errors
    ///
    /// storage writer、SQLite 或 projection 编解码失败时返回 [`RuleError`]。
    pub async fn rename_source_document(
        &self,
        request: RenameSourceDocumentRequest,
    ) -> Result<DocumentMutationOutcome, RuleError> {
        let trace_id = super::trace_id();
        let title = request.title.trim().to_string();
        if title.is_empty() {
            return Ok(invalid_outcome(DocumentValidationIssue::new(
                "document_title_invalid",
                "来源文档标题不能为空",
            )));
        }
        let result = self
            .state
            .storage
            .rename_source_document(RenameSourceDocumentInput {
                document_id: request.document_id,
                expected_revision: request.expected_revision,
                title,
                renamed_at_ms: now_millis(&trace_id)?,
            })
            .await;
        document_mutation_result(result, &trace_id)
    }

    /// 在 expected revision 匹配时删除未关联 draft。
    ///
    /// # Errors
    ///
    /// storage writer、SQLite、artifact ownership 或 secure store 失败时返回 [`RuleError`]。
    pub async fn delete_source_document(
        &self,
        request: DeleteSourceDocumentRequest,
    ) -> Result<DocumentMutationOutcome, RuleError> {
        let trace_id = super::trace_id();
        let result = self
            .state
            .storage
            .delete_source_document(DeleteSourceDocumentInput {
                document_id: request.document_id,
                expected_revision: request.expected_revision,
                deleted_at_ms: now_millis(&trace_id)?,
            })
            .await;
        document_mutation_result(result, &trace_id)
    }

    /// 显式 reveal 一个严格 owner-bound slot 的短生命周期 plaintext。
    ///
    /// 默认 document query 永远不调用此方法；返回值不实现 `Debug`/`Clone`/`Deserialize`。
    ///
    /// # Errors
    ///
    /// storage ownership、secret artifact 或 secure store 验证失败时返回 [`RuleError`]。
    pub async fn reveal_source_document_credential(
        &self,
        request: RevealSourceDocumentCredentialRequest,
    ) -> Result<SourceDocumentCredentialReveal, RuleError> {
        let trace_id = super::trace_id();
        let expected_target = request.target;
        let revealed = self
            .state
            .storage
            .reveal_source_document_credential(expected_target)
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))?
            .ok_or_else(|| {
                RuleError::new(
                    RuleErrorStage::Persistence,
                    "credential_not_found",
                    "credential slot 不存在",
                    trace_id.clone(),
                    false,
                    Vec::new(),
                )
            })?;
        credential_reveal(revealed, expected_target, &trace_id)
    }

    /// 替换单个 slot，随后为下一 revision 重新生成全部随机 slot ID/sentinel 并原子提交。
    ///
    /// # Errors
    ///
    /// storage read/writer、SQLite、artifact 或 secure store 基础设施失败时返回 [`RuleError`]。
    pub async fn replace_source_document_credential(
        &self,
        request: ReplaceSourceDocumentCredentialRequest,
    ) -> Result<DocumentMutationOutcome, RuleError> {
        let trace_id = super::trace_id();
        let target = request.target;
        let document_ref = DocumentRef {
            document_id: target.document_id,
            document_revision: target.document_revision,
        };
        let material = match self.load_document_material(document_ref, &trace_id).await? {
            DocumentMaterialLoad::Found(material) => material,
            DocumentMaterialLoad::Missing => {
                return self
                    .missing_document_outcome(
                        target.document_id,
                        target.document_revision,
                        &trace_id,
                    )
                    .await;
            }
            DocumentMaterialLoad::Outcome(outcome) => return Ok(outcome),
        };
        let (format, split) = match replace_credential(material, target, &request.value) {
            Ok(value) => value,
            Err(error) => return Ok(codec_invalid_outcome(error)),
        };
        self.commit_credential_edit(target, format, split, CredentialEdit::Replace, &trace_id)
            .await
    }

    /// 清除单个 slot，随后为下一 revision 重新生成仍存在的随机 slot ID/sentinel 并原子提交。
    ///
    /// # Errors
    ///
    /// storage read/writer、SQLite、artifact 或 secure store 基础设施失败时返回 [`RuleError`]。
    pub async fn clear_source_document_credential(
        &self,
        request: ClearSourceDocumentCredentialRequest,
    ) -> Result<DocumentMutationOutcome, RuleError> {
        let trace_id = super::trace_id();
        let target = request.target;
        let document_ref = DocumentRef {
            document_id: target.document_id,
            document_revision: target.document_revision,
        };
        let material = match self.load_document_material(document_ref, &trace_id).await? {
            DocumentMaterialLoad::Found(material) => material,
            DocumentMaterialLoad::Missing => {
                return self
                    .missing_document_outcome(
                        target.document_id,
                        target.document_revision,
                        &trace_id,
                    )
                    .await;
            }
            DocumentMaterialLoad::Outcome(outcome) => return Ok(outcome),
        };
        let (format, split) = match clear_credential(material, target) {
            Ok(value) => value,
            Err(error) => return Ok(codec_invalid_outcome(error)),
        };
        self.commit_credential_edit(target, format, split, CredentialEdit::Clear, &trace_id)
            .await
    }

    async fn load_document_material(
        &self,
        document_ref: DocumentRef,
        trace_id: &str,
    ) -> Result<DocumentMaterialLoad, RuleError> {
        match self
            .state
            .storage
            .load_source_document_material(document_ref)
            .await
        {
            Ok(Some(material)) => Ok(DocumentMaterialLoad::Found(material)),
            Ok(None) => Ok(DocumentMaterialLoad::Missing),
            Err(error) => document_security_outcome(&error).map_or_else(
                || Err(storage_error(&error, RuleErrorStage::Persistence, trace_id)),
                |outcome| Ok(DocumentMaterialLoad::Outcome(outcome)),
            ),
        }
    }

    async fn missing_document_outcome(
        &self,
        document_id: SourceDocumentId,
        expected_revision: u64,
        trace_id: &str,
    ) -> Result<DocumentMutationOutcome, RuleError> {
        let current = self
            .state
            .storage
            .get_source_document(document_id)
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, trace_id))?;
        Ok(
            current.map_or(DocumentMutationOutcome::NotFound, |current| {
                DocumentMutationOutcome::Conflict {
                    expected_revision,
                    actual_revision: current.summary.revision,
                    current,
                }
            }),
        )
    }

    async fn commit_credential_edit(
        &self,
        target: crate::SourceDocumentCredentialTarget,
        format: SourceDocumentFormat,
        split: CredentialSplit,
        edit: CredentialEdit,
        trace_id: &str,
    ) -> Result<DocumentMutationOutcome, RuleError> {
        let next_target = split.manifest.target.clone();
        let raw_text = match CredentialSlotCodec::reconstitute(
            &split.masked_text,
            &split.manifest,
            &split.secrets,
            &next_target,
        ) {
            Ok(text) => text,
            Err(error) => return Ok(codec_invalid_outcome(error)),
        };
        if let Some(outcome) = document_size_outcome(&raw_text) {
            return Ok(outcome);
        }
        let next_revision = revision_input(format, raw_text, split);
        let input = EditSourceDocumentCredentialInput {
            target,
            next_revision,
            saved_at_ms: now_millis(trace_id)?,
        };
        let outcome = match edit {
            CredentialEdit::Replace => {
                self.state
                    .storage
                    .replace_source_document_credential(input)
                    .await
            }
            CredentialEdit::Clear => {
                self.state
                    .storage
                    .clear_source_document_credential(input)
                    .await
            }
        };
        document_mutation_result(outcome, trace_id)
    }
}

fn rebase_commit_target(
    mode: crate::SourceDocumentRebaseMode,
    document_id: SourceDocumentId,
    current_revision: u64,
) -> RebaseOutcomeResult<(SourceDocumentId, u64, SourceDocumentRebaseCommitMode)> {
    match mode {
        crate::SourceDocumentRebaseMode::Merge {} => current_revision
            .checked_add(1)
            .map(|target_revision| {
                (
                    document_id,
                    target_revision,
                    SourceDocumentRebaseCommitMode::Merge,
                )
            })
            .ok_or_else(|| Box::new(revision_invalid_outcome())),
        crate::SourceDocumentRebaseMode::Fork { title } => {
            let title = title.trim().to_string();
            if title.is_empty() {
                return Err(Box::new(invalid_outcome(DocumentValidationIssue::new(
                    "document_title_invalid",
                    "来源文档标题不能为空",
                ))));
            }
            let fork_document_id = SourceDocumentId::new();
            Ok((
                fork_document_id,
                1,
                SourceDocumentRebaseCommitMode::Fork {
                    document_id: fork_document_id,
                    title,
                },
            ))
        }
    }
}

struct TrustedCredentialPath<'a> {
    name: &'a str,
    value: &'a str,
}

struct TrustedRevisionMaterial {
    document_ref: DocumentRef,
    format: SourceDocumentFormat,
    manifest: lj_rule_model::CredentialSlotManifest,
    credentials: Vec<CredentialSecret>,
}

fn trusted_revision_material(
    material: SourceDocumentMaterial,
) -> RebaseOutcomeResult<TrustedRevisionMaterial> {
    let (document_ref, format, masked_text, raw_text, manifest, credentials) =
        material.into_parts();
    let credentials = credential_secrets(credentials);
    let target = credential_target(
        format,
        document_ref.document_id,
        document_ref.document_revision,
    );
    let reconstructed =
        CredentialSlotCodec::reconstitute(&masked_text, &manifest, &credentials, &target)
            .map_err(codec_invalid_outcome)?;
    if reconstructed != raw_text {
        return Err(Box::new(rebase_resolution_invalid(
            "credential_material_mismatch",
            "",
        )));
    }
    Ok(TrustedRevisionMaterial {
        document_ref,
        format,
        manifest,
        credentials,
    })
}

fn prepare_rebased_revision(
    base: SourceDocumentMaterial,
    current: SourceDocumentMaterial,
    local_masked_text: &str,
    resolutions: Vec<crate::SourceDocumentCredentialResolution>,
    target_document_id: SourceDocumentId,
    target_revision: u64,
) -> RebaseOutcomeResult<SourceDocumentRevisionInput> {
    let base = trusted_revision_material(base)?;
    let current = trusted_revision_material(current)?;
    if base.document_ref.document_id != current.document_ref.document_id
        || base.format != current.format
    {
        return Err(Box::new(rebase_resolution_invalid(
            "rebase_document_owner_mismatch",
            "",
        )));
    }
    let base_target = credential_target(
        base.format,
        base.document_ref.document_id,
        base.document_ref.document_revision,
    );
    let local_raw_text = CredentialSlotCodec::reconstitute(
        local_masked_text,
        &base.manifest,
        &base.credentials,
        &base_target,
    )
    .map_err(codec_invalid_outcome)?;
    resolve_rebased_credentials(
        &local_raw_text,
        &base,
        &current,
        resolutions,
        target_document_id,
        target_revision,
    )
}

fn resolve_rebased_credentials(
    local_raw_text: &str,
    base: &TrustedRevisionMaterial,
    current: &TrustedRevisionMaterial,
    resolutions: Vec<crate::SourceDocumentCredentialResolution>,
    target_document_id: SourceDocumentId,
    target_revision: u64,
) -> RebaseOutcomeResult<SourceDocumentRevisionInput> {
    let base_paths = trusted_credential_paths(&base.manifest, &base.credentials)?;
    let current_paths = trusted_credential_paths(&current.manifest, &current.credentials)?;
    let union = base_paths
        .keys()
        .chain(current_paths.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    for path in &union {
        if let (Some(base), Some(current)) = (base_paths.get(*path), current_paths.get(*path))
            && base.name != current.name
        {
            return Err(Box::new(rebase_resolution_invalid(
                "credential_path_drift",
                path,
            )));
        }
    }

    let mut resolution_by_path = BTreeMap::new();
    for resolution in resolutions {
        if resolution.path.is_empty() || resolution_by_path.contains_key(&resolution.path) {
            return Err(Box::new(rebase_resolution_invalid(
                "credential_resolution_duplicate",
                &resolution.path,
            )));
        }
        if !union.contains(resolution.path.as_str()) {
            return Err(Box::new(rebase_resolution_invalid(
                "credential_resolution_path_unknown",
                &resolution.path,
            )));
        }
        resolution_by_path.insert(resolution.path, resolution.action);
    }
    for path in &union {
        if !resolution_by_path.contains_key(*path) {
            return Err(Box::new(rebase_resolution_invalid(
                "credential_resolution_missing",
                path,
            )));
        }
    }

    let mut resolved = Vec::with_capacity(union.len());
    for path in union {
        let action = resolution_by_path
            .get(path)
            .ok_or_else(|| rebase_resolution_invalid("credential_resolution_missing", path))?;
        match action {
            crate::SourceDocumentCredentialAction::KeepCurrent => {
                let credential = current_paths.get(path).ok_or_else(|| {
                    rebase_resolution_invalid("credential_resolution_current_missing", path)
                })?;
                resolved.push(CredentialRebaseResolution::replace(path, credential.value));
            }
            crate::SourceDocumentCredentialAction::KeepLocal => {
                let credential = base_paths.get(path).ok_or_else(|| {
                    rebase_resolution_invalid("credential_resolution_local_missing", path)
                })?;
                resolved.push(CredentialRebaseResolution::replace(path, credential.value));
            }
            crate::SourceDocumentCredentialAction::Replace { value } => {
                resolved.push(CredentialRebaseResolution::replace(path, value));
            }
            crate::SourceDocumentCredentialAction::Clear => {
                resolved.push(CredentialRebaseResolution::clear(path));
            }
        }
    }
    let rebased = CredentialSlotCodec::rebase_resolved_credentials(
        local_raw_text,
        base.format,
        &resolved,
        credential_target(base.format, target_document_id, target_revision),
    )
    .map_err(codec_invalid_outcome)?;
    let (raw_text, split) = rebased.into_parts();
    if let Some(outcome) = document_size_outcome(&raw_text) {
        return Err(Box::new(outcome));
    }
    Ok(revision_input(base.format, raw_text, split))
}

fn trusted_credential_paths<'a>(
    manifest: &'a lj_rule_model::CredentialSlotManifest,
    credentials: &'a [CredentialSecret],
) -> RebaseOutcomeResult<BTreeMap<&'a str, TrustedCredentialPath<'a>>> {
    let by_slot = credentials
        .iter()
        .map(|credential| (credential.slot_id(), credential))
        .collect::<HashMap<_, _>>();
    if by_slot.len() != credentials.len() || by_slot.len() != manifest.slots.len() {
        return Err(Box::new(rebase_resolution_invalid(
            "credential_slot_set_mismatch",
            "",
        )));
    }
    let mut by_path = BTreeMap::new();
    for slot in &manifest.slots {
        let credential = *by_slot
            .get(&slot.slot_id)
            .ok_or_else(|| rebase_resolution_invalid("credential_slot_set_mismatch", &slot.path))?;
        if by_path
            .insert(
                slot.path.as_str(),
                TrustedCredentialPath {
                    name: &slot.name,
                    value: credential.expose_value(),
                },
            )
            .is_some()
        {
            return Err(Box::new(rebase_resolution_invalid(
                "credential_resolution_duplicate",
                &slot.path,
            )));
        }
    }
    Ok(by_path)
}

fn rebase_resolution_invalid(code: &str, path: &str) -> DocumentMutationOutcome {
    DocumentMutationOutcome::Invalid {
        issues: vec![DocumentValidationIssue {
            code: code.to_string(),
            message: "来源文档 credential resolution 未通过安全校验".to_string(),
            path: (!path.is_empty()).then(|| path.to_string()),
            byte_offset: None,
            byte_length: None,
        }],
    }
}

fn rebase_pin_invalid_outcome(
    reason: SourceDocumentRebaseInvalidReason,
) -> DocumentMutationOutcome {
    let code = match reason {
        SourceDocumentRebaseInvalidReason::PinNotFound => "revision_pin_not_found",
        SourceDocumentRebaseInvalidReason::PinExpired => "revision_pin_expired",
        SourceDocumentRebaseInvalidReason::PinOwnerMismatch => "revision_pin_owner_mismatch",
        SourceDocumentRebaseInvalidReason::PinRevisionMismatch => "revision_pin_revision_mismatch",
    };
    invalid_outcome(DocumentValidationIssue::new(
        code,
        "来源文档 revision pin 无效或已失效",
    ))
}

fn pin_storage_error(error: &StorageError, trace_id: &str) -> RuleError {
    match error {
        StorageError::DocumentMissing => revision_pin_rule_error(
            "revision_pin_document_not_found",
            "来源文档不存在",
            trace_id,
        ),
        StorageError::VersionConflict { .. } => revision_pin_rule_error(
            "revision_pin_revision_mismatch",
            "来源文档 revision 已变化",
            trace_id,
        ),
        StorageError::InvalidInput(_) => revision_pin_rule_error(
            "revision_pin_invalid",
            "来源文档 revision pin 参数无效",
            trace_id,
        ),
        _ => storage_error(error, RuleErrorStage::Persistence, trace_id),
    }
}

fn revision_pin_rule_error(code: &str, message: &str, trace_id: &str) -> RuleError {
    RuleError::new(
        RuleErrorStage::Validation,
        code,
        message,
        trace_id.to_string(),
        false,
        Vec::new(),
    )
}

fn replace_credential(
    material: SourceDocumentMaterial,
    target: crate::SourceDocumentCredentialTarget,
    replacement: &str,
) -> Result<(SourceDocumentFormat, CredentialSplit), CredentialCodecError> {
    let (document_ref, format, masked_text, raw_text, manifest, credentials) =
        material.into_parts();
    drop(raw_text);
    let expected_target = credential_target(
        format,
        document_ref.document_id,
        document_ref.document_revision,
    );
    let next_target = credential_target(
        format,
        document_ref.document_id,
        document_ref.document_revision.saturating_add(1),
    );
    let credentials = credential_secrets(credentials);
    CredentialSlotCodec::replace_credential(
        &masked_text,
        &manifest,
        &credentials,
        &expected_target,
        target.slot_id,
        replacement,
        next_target,
    )
    .map(|split| (format, split))
}

fn clear_credential(
    material: SourceDocumentMaterial,
    target: crate::SourceDocumentCredentialTarget,
) -> Result<(SourceDocumentFormat, CredentialSplit), CredentialCodecError> {
    let (document_ref, format, masked_text, raw_text, manifest, credentials) =
        material.into_parts();
    drop(raw_text);
    let expected_target = credential_target(
        format,
        document_ref.document_id,
        document_ref.document_revision,
    );
    let next_target = credential_target(
        format,
        document_ref.document_id,
        document_ref.document_revision.saturating_add(1),
    );
    let credentials = credential_secrets(credentials);
    CredentialSlotCodec::clear_credential(
        &masked_text,
        &manifest,
        &credentials,
        &expected_target,
        target.slot_id,
        next_target,
    )
    .map(|split| (format, split))
}

fn revision_input(
    format: SourceDocumentFormat,
    raw_text: String,
    split: CredentialSplit,
) -> SourceDocumentRevisionInput {
    let credentials = split
        .secrets
        .into_iter()
        .map(|secret| {
            let slot_id = secret.slot_id();
            CredentialSlotMaterial::new(slot_id, secret.into_value())
        })
        .collect();
    SourceDocumentRevisionInput {
        format,
        masked_text: split.masked_text,
        raw_text,
        manifest: split.manifest,
        credentials,
        issues: Vec::new(),
    }
}

fn credential_secrets(credentials: Vec<CredentialSlotMaterial>) -> Vec<CredentialSecret> {
    credentials
        .into_iter()
        .map(|credential| {
            let slot_id = credential.slot_id();
            CredentialSecret::new(slot_id, credential.into_value())
        })
        .collect()
}

fn credential_target(
    format: SourceDocumentFormat,
    document_id: SourceDocumentId,
    revision: u64,
) -> CredentialTargetIdentity {
    CredentialTargetIdentity {
        format,
        document_id: document_id.to_string(),
        revision,
    }
}

fn credential_reveal(
    revealed: RevealedSourceDocumentCredential,
    expected_target: crate::SourceDocumentCredentialTarget,
    trace_id: &str,
) -> Result<SourceDocumentCredentialReveal, RuleError> {
    if revealed.target() != expected_target {
        return Err(RuleError::new(
            RuleErrorStage::Persistence,
            "credential_owner_mismatch",
            "credential slot owner 验证失败",
            trace_id.to_string(),
            false,
            Vec::new(),
        ));
    }
    Ok(SourceDocumentCredentialReveal {
        target: expected_target,
        value: revealed.into_value(),
    })
}

fn document_security_outcome(error: &StorageError) -> Option<DocumentMutationOutcome> {
    match error {
        StorageError::KeyringLocked => Some(DocumentMutationOutcome::Locked),
        StorageError::KeyringUnavailable => Some(DocumentMutationOutcome::KeyUnavailable),
        StorageError::KeyLost | StorageError::MasterKeyUnavailable => {
            Some(DocumentMutationOutcome::KeyLost)
        }
        StorageError::ArtifactCorrupt
        | StorageError::ArtifactUnavailable(_)
        | StorageError::SecretUnavailable => Some(DocumentMutationOutcome::Corrupt),
        _ => None,
    }
}

fn document_mutation_result(
    result: Result<DocumentMutationOutcome, StorageError>,
    trace_id: &str,
) -> Result<DocumentMutationOutcome, RuleError> {
    match result {
        Ok(outcome) => Ok(outcome),
        Err(error) => document_security_outcome(&error).map_or_else(
            || Err(storage_error(&error, RuleErrorStage::Persistence, trace_id)),
            Ok,
        ),
    }
}

fn document_size_outcome(text: &str) -> Option<DocumentMutationOutcome> {
    (text.len() > MAX_SOURCE_DOCUMENT_BYTES).then(|| {
        invalid_outcome(DocumentValidationIssue::new(
            "document_too_large",
            "来源文档超过 2 MiB UTF-8 上限",
        ))
    })
}

fn revision_invalid_outcome() -> DocumentMutationOutcome {
    invalid_outcome(DocumentValidationIssue::new(
        "document_revision_invalid",
        "来源文档 revision 无效",
    ))
}

fn codec_invalid_outcome(error: CredentialCodecError) -> DocumentMutationOutcome {
    invalid_outcome(authoring_issue(error.diagnostic))
}

fn authoring_issue(diagnostic: AuthoringDiagnostic) -> DocumentValidationIssue {
    DocumentValidationIssue {
        code: diagnostic.code,
        message: "来源文档未通过安全校验".to_string(),
        path: (!diagnostic.path.is_empty()).then_some(diagnostic.path),
        byte_offset: u64::try_from(diagnostic.byte_offset).ok(),
        byte_length: u64::try_from(diagnostic.byte_length).ok(),
    }
}

fn invalid_outcome(issue: DocumentValidationIssue) -> DocumentMutationOutcome {
    DocumentMutationOutcome::Invalid {
        issues: vec![issue],
    }
}
