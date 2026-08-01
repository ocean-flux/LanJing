//! candidate staging 与 installed source façade。

use uuid::Uuid;

use super::EventProjectionStorage;
use crate::candidate_install::{
    get_candidate_summary, get_installed_source_sync, list_installed_sources_sync,
};
use crate::types::{
    CandidateDraft, CandidateSummary, InstallCandidateRequest, InstalledSource,
    InstalledSourceRecord, StorageError,
};
use crate::writer::WriterCommand;

impl EventProjectionStorage {
    /// durable staging opaque candidate、作者包与 immutable Plan。
    ///
    /// # Errors
    ///
    /// 输入不一致、artifact/SQLite 写入或 writer 失败时返回 `StorageError`。
    pub async fn stage_candidate(
        &self,
        draft: CandidateDraft,
    ) -> Result<CandidateSummary, StorageError> {
        self.dispatch(|reply| WriterCommand::StageCandidate {
            draft: Box::new(draft),
            reply,
        })
        .await
    }

    /// 读取 candidate 安全预览，不读取作者包、Definition 或 Plan。
    ///
    /// # Errors
    ///
    /// `SQLite` 或 preview JSON 损坏时返回 `StorageError`。
    pub async fn get_candidate_summary(
        &self,
        candidate_id: Uuid,
    ) -> Result<Option<CandidateSummary>, StorageError> {
        self.read(move |conn, _| {
            Box::pin(async move { get_candidate_summary(conn, candidate_id).await })
        })
        .await
    }

    /// 原子消费 candidate 并安装 source version、package、Plan、grant 与 profile。
    ///
    /// # Errors
    ///
    /// candidate 状态、version conflict 或持久化失败时返回 `StorageError`。
    pub async fn install_candidate(
        &self,
        request: InstallCandidateRequest,
    ) -> Result<InstalledSource, StorageError> {
        self.dispatch(|reply| WriterCommand::InstallCandidate { request, reply })
            .await
    }

    /// 读取 installed source 与 immutable package/Plan。
    ///
    /// # Errors
    ///
    /// SQLite、artifact 或 JSON 读取失败时返回 `StorageError`。
    pub async fn get_installed_source(
        &self,
        source_identity: impl Into<String>,
    ) -> Result<Option<InstalledSource>, StorageError> {
        let source_identity = source_identity.into();
        self.read(move |conn, artifacts| {
            Box::pin(
                async move { get_installed_source_sync(conn, artifacts, &source_identity).await },
            )
        })
        .await
    }

    /// 按 source identity 升序读取 installed source 安全摘要。
    ///
    /// # Errors
    ///
    /// SQLite、投影 JSON 或 ownership 损坏时返回 `StorageError`。
    pub async fn list_installed_sources(&self) -> Result<Vec<InstalledSourceRecord>, StorageError> {
        self.read(move |conn, _| Box::pin(async move { list_installed_sources_sync(conn).await }))
            .await
    }
}
