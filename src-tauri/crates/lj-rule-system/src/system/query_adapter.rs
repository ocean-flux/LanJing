//! 安全 query adapter 与资料库投影映射。
//!
//! query 只读 C2 的规范化投影或更新 library aggregate；不会返回 Definition、Plan、artifact
//! ref、secret 或 storage transaction。`catch_up_execution` 保持 C2 stream sequence 连续性，
//! 使 delivery 断开不会改变 execution 的生命周期。

use lj_media::MediaResourceId;
use lj_storage::{
    InstalledSourceRecord as StorageInstalledSourceRecord, LibraryEntry as StorageLibraryEntry,
    LibraryProgress as StorageLibraryProgress, LibraryProjection as StorageLibraryProjection,
    LibraryProjectionEntry as StorageLibraryProjectionEntry, LibraryUpdate as StorageLibraryUpdate,
};
use uuid::Uuid;

use super::error_mapping::storage_error;
use super::session_delivery::catch_up_execution;
use super::{RuleSystem, now_millis};
use crate::{
    ExecutionEvent, ExecutionId, InstalledSource, LibraryEntryUpdate, LibraryProgress,
    LibraryProjection, LibraryProjectionEntry, LibraryUpdateReceipt, RuleError, RuleErrorStage,
};

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
}

fn installed_source_from_record(source: StorageInstalledSourceRecord) -> InstalledSource {
    InstalledSource {
        source_id: crate::SourceId::from_identity(source.source_identity),
        version: source.version,
        profile: source.profile,
        revision: source.revision,
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
