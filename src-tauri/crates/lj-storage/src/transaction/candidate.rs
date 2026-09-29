//! Candidate staging/install transaction service。

use crate::artifact::ArtifactStore;
use crate::database::DatabaseSession;
use crate::types::{
    CandidateDraft, CandidateSummary, InstallCandidateRequest, InstalledSource,
    SourceRollbackRequest, StorageError,
};

pub(crate) async fn stage(
    connection: &DatabaseSession,
    artifacts: &ArtifactStore,
    draft: CandidateDraft,
) -> Result<CandidateSummary, StorageError> {
    let artifacts = artifacts.clone();
    super::run(connection, move |transaction| {
        Box::pin(async move {
            crate::repository::candidate_source::process_stage_candidate(
                transaction,
                &artifacts,
                draft,
            )
            .await
        })
    })
    .await
}

pub(crate) async fn stage_source_rollback(
    connection: &DatabaseSession,
    artifacts: &ArtifactStore,
    request: SourceRollbackRequest,
) -> Result<CandidateSummary, StorageError> {
    let artifacts = artifacts.clone();
    super::run(connection, move |transaction| {
        Box::pin(async move {
            crate::repository::candidate_source::process_stage_source_rollback(
                transaction,
                &artifacts,
                request,
            )
            .await
        })
    })
    .await
}

pub(crate) async fn install(
    connection: &DatabaseSession,
    artifacts: &ArtifactStore,
    request: InstallCandidateRequest,
) -> Result<InstalledSource, StorageError> {
    let artifacts = artifacts.clone();
    super::run(connection, move |transaction| {
        Box::pin(async move {
            crate::repository::candidate_source::process_install_candidate(
                transaction,
                &artifacts,
                request,
            )
            .await
        })
    })
    .await
}

pub(crate) async fn recover(
    connection: &DatabaseSession,
    now_ms: i64,
) -> Result<usize, StorageError> {
    super::run(connection, move |transaction| {
        Box::pin(async move {
            crate::repository::candidate_source::recover_candidates_sync(transaction, now_ms).await
        })
    })
    .await
}
