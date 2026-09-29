//! 安全 query adapter 与资料库投影映射。
//!
//! query 只读 C2 的规范化投影或更新 library aggregate；不会返回 Definition、Plan、artifact
//! ref、secret 或 storage transaction。`catch_up_execution` 保持 C2 stream sequence 连续性，
//! 使 delivery 断开不会改变 execution 的生命周期。
//!
//! 标准媒体投影读取（item/unit/asset）只按稳定 ID 与有界分页恢复；不替代 live execution，
//! 也不下发整图 `MediaGraphDelta`。

use lj_media::{MediaItem, MediaResourceId};
use lj_storage::{
    InstalledSourceRecord as StorageInstalledSourceRecord, LibraryEntry as StorageLibraryEntry,
    LibraryProgress as StorageLibraryProgress, LibraryProjection as StorageLibraryProjection,
    LibraryProjectionEntry as StorageLibraryProjectionEntry, LibraryUpdate as StorageLibraryUpdate,
    SourceRevisionRecord as StorageSourceRevisionRecord,
};
use uuid::Uuid;

use super::error_mapping::storage_error;
use super::session_delivery::catch_up_execution;
use super::{RuleSystem, now_millis};
use crate::{
    ExecutionEvent, ExecutionId, InstalledSource, LibraryEntryUpdate, LibraryProgress,
    LibraryProjection, LibraryProjectionEntry, LibraryUpdateReceipt, MediaAssetPage, MediaUnitPage,
    RuleError, RuleErrorStage, SourceId, SourceRevisionSummary,
};

/// 产品面分页默认页大小。
const DEFAULT_MEDIA_PAGE_LIMIT: u32 = 50;
/// 产品面分页硬上限。
const MAX_MEDIA_PAGE_LIMIT: u32 = 100;
/// 批量点查 ID 数量硬上限。
const MAX_MEDIA_BATCH_IDS: usize = 64;

impl RuleSystem {
    /// 按稳定 source identity 升序列出所有已安装来源的安全摘要。
    ///
    /// # Errors
    ///
    /// C2 来源投影读取失败时返回 [`RuleError`]；不会返回 Definition、Plan、artifact ref 或 secret。
    pub async fn list_installed_sources(&self) -> Result<Vec<InstalledSource>, RuleError> {
        let trace_id = super::trace_id();
        let sources = self
            .state
            .storage
            .list_installed_sources()
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))?;
        Ok(sources
            .into_iter()
            .map(installed_source_from_record)
            .collect())
    }

    /// 按新到旧读取来源的不可变 revision 安全摘要。
    ///
    /// # Errors
    ///
    /// 来源 ID 为空、历史来源不存在或 C2 读取失败时返回 [`RuleError`]。
    pub async fn list_source_revisions(
        &self,
        source_id: SourceId,
    ) -> Result<Vec<SourceRevisionSummary>, RuleError> {
        let trace_id = super::trace_id();
        let source_identity = source_id.as_identity().to_string();
        if source_identity.trim().is_empty() {
            return Err(RuleError::new(
                RuleErrorStage::Validation,
                "source_id_invalid",
                "来源 ID 不能为空",
                trace_id,
                false,
                Vec::new(),
            ));
        }
        let revisions = self
            .state
            .storage
            .list_source_revisions(source_identity)
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))?;
        Ok(revisions
            .into_iter()
            .map(source_revision_from_storage)
            .collect())
    }

    /// 读取完整、安全的资料库投影快照。
    ///
    /// # Errors
    ///
    /// C2 资料库投影读取失败时返回 [`RuleError`]；不会返回媒体 Graph、规则 Plan 或 secret。
    pub async fn get_library_projection(&self) -> Result<LibraryProjection, RuleError> {
        let trace_id = super::trace_id();
        let projection = self
            .state
            .storage
            .get_library_projection()
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))?;
        Ok(library_projection_from_storage(projection))
    }

    /// 原子更新一个资料库条目，并返回新的全局序号与资源 revision。
    ///
    /// C2 在同一 transaction 中写入 library event 与投影；调用方不能观察到只写其一的中间态。
    ///
    /// # Errors
    ///
    /// 资源 ID 为空、optimistic revision 冲突或 C2 持久化失败时返回 [`RuleError`]。
    pub async fn update_library_entry(
        &self,
        request: LibraryEntryUpdate,
    ) -> Result<LibraryUpdateReceipt, RuleError> {
        let trace_id = super::trace_id();
        let LibraryEntryUpdate {
            resource_id,
            favorite,
            pinned,
            last_opened_at,
            progress,
            expected_version,
        } = request;
        if resource_id.trim().is_empty() {
            return Err(RuleError::new(
                RuleErrorStage::Validation,
                "library_resource_id_invalid",
                "资料库资源 ID 不能为空",
                trace_id,
                false,
                Vec::new(),
            ));
        }
        let receipt = self
            .state
            .storage
            .update_library(StorageLibraryUpdate {
                entry: StorageLibraryEntry {
                    resource_id: MediaResourceId(resource_id),
                    favorite,
                    pinned,
                    last_opened_at,
                    progress: progress.map(library_progress_to_storage),
                },
                expected_version,
                event_id: Uuid::new_v4(),
                occurred_at_ms: now_millis(&trace_id)?,
                trace_id: trace_id.clone(),
            })
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))?;
        Ok(LibraryUpdateReceipt {
            global_seq: receipt.global_seq,
            revision: receipt.stream_version,
        })
    }

    /// 从指定 execution stream sequence 之后补读所有持久事件。
    ///
    /// sequence 必须无洞：若 C2 返回的 stream version 不是严格递增的下一个值，本 façade
    /// 会返回错误而不是向 delivery 伪造连续事件。
    ///
    /// # Errors
    ///
    /// execution 不存在、事件 payload 损坏或 C2 read lane 失败时返回 [`RuleError`]。
    pub async fn catch_up_execution(
        &self,
        execution_id: ExecutionId,
        after_sequence: u64,
    ) -> Result<Vec<ExecutionEvent>, RuleError> {
        catch_up_execution(&self.state.storage, execution_id, after_sequence).await
    }

    /// 按稳定 item ID 取标准媒体摘要；缺失或 tombstone 返回 `None`。
    ///
    /// # Errors
    ///
    /// 资源 ID 为空，或 C2 投影读取失败时返回 [`RuleError`]；存储失败不会变成空成功。
    pub async fn get_media_item(
        &self,
        resource_id: String,
    ) -> Result<Option<MediaItem>, RuleError> {
        let trace_id = super::trace_id();
        let resource_id = require_non_empty_id(&resource_id, "resource_id", &trace_id)?;
        self.state
            .storage
            .get_item(MediaResourceId(resource_id))
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))
    }

    /// 批量按稳定 item ID 取标准媒体摘要；跳过缺失 ID，命中结果按 `id` 升序。
    ///
    /// # Errors
    ///
    /// ID 列表为空、超过 64、含空 ID，或 C2 投影读取失败时返回 [`RuleError`]。
    pub async fn get_media_items(
        &self,
        resource_ids: Vec<String>,
    ) -> Result<Vec<MediaItem>, RuleError> {
        let trace_id = super::trace_id();
        if resource_ids.is_empty() {
            return Err(validation_error(
                "media_resource_ids_empty",
                "批量资源 ID 不能为空",
                &trace_id,
            ));
        }
        if resource_ids.len() > MAX_MEDIA_BATCH_IDS {
            return Err(validation_error(
                "media_resource_ids_too_many",
                format!("批量资源 ID 最多 {MAX_MEDIA_BATCH_IDS} 个"),
                &trace_id,
            ));
        }
        let mut ids = Vec::with_capacity(resource_ids.len());
        for resource_id in resource_ids {
            let resource_id = require_non_empty_id(&resource_id, "resource_id", &trace_id)?;
            ids.push(MediaResourceId(resource_id));
        }
        self.state
            .storage
            .get_items(ids)
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))
    }

    /// 有界列出某 item 下的消费单元（目录页）。
    ///
    /// 父 item 缺失时返回空页且 `parent_found=false`（不 404）。
    /// `limit` 默认 50、硬上限 100；`0` 或 `>100` 为校验错误。
    ///
    /// # Errors
    ///
    /// 参数非法或 C2 投影读取失败时返回 [`RuleError`]。
    pub async fn list_media_units(
        &self,
        item_id: String,
        offset: u32,
        limit: Option<u32>,
    ) -> Result<MediaUnitPage, RuleError> {
        let trace_id = super::trace_id();
        let item_id = require_non_empty_id(&item_id, "item_id", &trace_id)?;
        let limit = resolve_page_limit(limit, &trace_id)?;
        let parent_found = self
            .state
            .storage
            .get_item(MediaResourceId(item_id.clone()))
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))?
            .is_some();
        if !parent_found {
            return Ok(MediaUnitPage {
                items: Vec::new(),
                offset,
                limit,
                has_more: false,
                parent_found: false,
            });
        }
        // limit+1 探测 has_more，避免全量 count。
        let fetch_limit = limit.saturating_add(1);
        let rows = self
            .state
            .storage
            .list_units_for_item(MediaResourceId(item_id), offset, fetch_limit)
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))?;
        let (items, has_more) = take_page(rows, limit);
        Ok(MediaUnitPage {
            items,
            offset,
            limit,
            has_more,
            parent_found: true,
        })
    }

    /// 有界列出某 unit 下的资产（正文/封面/流）。
    ///
    /// 父 unit 缺失时返回空页且 `parent_found=false`（不 404）。
    ///
    /// # Errors
    ///
    /// 参数非法或 C2 投影读取失败时返回 [`RuleError`]。
    pub async fn list_media_assets(
        &self,
        unit_id: String,
        offset: u32,
        limit: Option<u32>,
    ) -> Result<MediaAssetPage, RuleError> {
        let trace_id = super::trace_id();
        let unit_id = require_non_empty_id(&unit_id, "unit_id", &trace_id)?;
        let limit = resolve_page_limit(limit, &trace_id)?;
        let parent_found = self
            .state
            .storage
            .get_unit(MediaResourceId(unit_id.clone()))
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))?
            .is_some();
        if !parent_found {
            return Ok(MediaAssetPage {
                items: Vec::new(),
                offset,
                limit,
                has_more: false,
                parent_found: false,
            });
        }
        let fetch_limit = limit.saturating_add(1);
        let rows = self
            .state
            .storage
            .list_assets_for_unit(MediaResourceId(unit_id), offset, fetch_limit)
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))?;
        let (items, has_more) = take_page(rows, limit);
        Ok(MediaAssetPage {
            items,
            offset,
            limit,
            has_more,
            parent_found: true,
        })
    }
}

fn require_non_empty_id(value: &str, field: &str, trace_id: &str) -> Result<String, RuleError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(validation_error(
            format!("{field}_invalid"),
            format!("{field} 不能为空"),
            trace_id,
        ));
    }
    Ok(trimmed.to_string())
}

fn resolve_page_limit(limit: Option<u32>, trace_id: &str) -> Result<u32, RuleError> {
    let limit = limit.unwrap_or(DEFAULT_MEDIA_PAGE_LIMIT);
    if limit == 0 || limit > MAX_MEDIA_PAGE_LIMIT {
        return Err(validation_error(
            "media_page_limit_invalid",
            format!("limit 必须在 1..={MAX_MEDIA_PAGE_LIMIT} 之间"),
            trace_id,
        ));
    }
    Ok(limit)
}

/// 将 `limit+1` 探测结果裁成产品页：超出则 `has_more=true` 并截断至 `limit`。
fn take_page<T>(mut rows: Vec<T>, limit: u32) -> (Vec<T>, bool) {
    let limit_usize = usize::try_from(limit).unwrap_or(usize::MAX);
    let has_more = rows.len() > limit_usize;
    if has_more {
        rows.truncate(limit_usize);
    }
    (rows, has_more)
}

fn validation_error(
    code: impl Into<String>,
    message: impl Into<String>,
    trace_id: &str,
) -> RuleError {
    RuleError::new(
        RuleErrorStage::Validation,
        code,
        message,
        trace_id.to_string(),
        false,
        Vec::new(),
    )
}

fn installed_source_from_record(source: StorageInstalledSourceRecord) -> InstalledSource {
    InstalledSource {
        source_id: crate::SourceId::from_identity(source.source_identity),
        version: source.version,
        profile: source.profile,
        grant: crate::CapabilityGrant::from_policy(source.grant),
        revision: source.source_revision,
    }
}

fn source_revision_from_storage(record: StorageSourceRevisionRecord) -> SourceRevisionSummary {
    SourceRevisionSummary {
        source_id: SourceId::from_identity(record.source_identity),
        revision: record.source_revision,
        version: record.version,
        profile: record.profile,
        grant: crate::CapabilityGrant::from_policy(record.grant),
        definition_hash: record.definition_hash,
        plan_hash: record.plan_hash,
        installed_at_ms: record.installed_at_ms,
    }
}

fn library_projection_from_storage(projection: StorageLibraryProjection) -> LibraryProjection {
    LibraryProjection {
        global_seq: projection.global_seq,
        entries: projection
            .entries
            .into_iter()
            .map(library_projection_entry_from_storage)
            .collect(),
    }
}

fn library_projection_entry_from_storage(
    entry: StorageLibraryProjectionEntry,
) -> LibraryProjectionEntry {
    LibraryProjectionEntry {
        resource_id: entry.resource_id.0,
        favorite: entry.favorite,
        pinned: entry.pinned,
        last_opened_at: entry.last_opened_at,
        progress: entry.progress.map(library_progress_from_storage),
        revision: entry.revision,
        updated_global_seq: entry.updated_global_seq,
    }
}

fn library_progress_from_storage(progress: StorageLibraryProgress) -> LibraryProgress {
    LibraryProgress {
        unit_id: progress.unit_id.map(|unit_id| unit_id.0),
        position: progress.position,
        total: progress.total,
    }
}

fn library_progress_to_storage(progress: LibraryProgress) -> StorageLibraryProgress {
    StorageLibraryProgress {
        unit_id: progress.unit_id.map(MediaResourceId),
        position: progress.position,
        total: progress.total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn take_page_detects_has_more_and_truncates() {
        let (page, has_more) = take_page(vec!["a", "b", "c"], 2);
        assert!(has_more);
        assert_eq!(page, vec!["a", "b"]);
    }

    #[test]
    fn take_page_exact_limit_has_no_more() {
        let (page, has_more) = take_page(vec!["a", "b"], 2);
        assert!(!has_more);
        assert_eq!(page, vec!["a", "b"]);
    }

    #[test]
    fn resolve_page_limit_defaults_and_rejects_bounds() {
        let trace = "trace-limit";
        assert_eq!(resolve_page_limit(None, trace).expect("default"), 50);
        assert_eq!(resolve_page_limit(Some(1), trace).expect("min"), 1);
        assert_eq!(resolve_page_limit(Some(100), trace).expect("max"), 100);
        assert_eq!(
            resolve_page_limit(Some(0), trace).expect_err("zero").code,
            "media_page_limit_invalid"
        );
        assert_eq!(
            resolve_page_limit(Some(101), trace).expect_err("over").code,
            "media_page_limit_invalid"
        );
    }

    #[test]
    fn require_non_empty_id_trims_and_rejects_blank() {
        let trace = "trace-id";
        assert_eq!(
            require_non_empty_id("  item:1  ", "resource_id", trace).expect("trim"),
            "item:1"
        );
        assert_eq!(
            require_non_empty_id("   ", "resource_id", trace)
                .expect_err("blank")
                .code,
            "resource_id_invalid"
        );
    }
}
