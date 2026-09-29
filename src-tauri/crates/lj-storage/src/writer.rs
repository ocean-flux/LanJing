//! 有界 single writer 的命令协议与 async actor。
//!
//! 只有此模块启动的 Tokio task 持有 `SeaORM` read-write pool；async 调用方通过固定容量
//! `mpsc` 排队，因此 16 路执行不会直接争抢 `SQLite` writer lock。batch 仅降低唤醒开销，**每条命令仍在
//! 自己的 transaction 中**提交 Event、expected-version、global sequence 与投影，不能跨命令
//! 合并事务或改变 receipt 的顺序边界。

use crate::database::DatabaseSession;
use lj_runtime::{ControlTraceCapture, ControlTraceReceipt, DurableCaptureReceipt, EffectCapture};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::artifact::ArtifactStore;
use crate::types::{
    AppendRequest, CandidateDraft, CandidateSummary, CheckpointReceipt, CommitReceipt,
    CreateDocumentRequest, DeleteDocumentRequest, DeltaCommit, DocumentSummary, ExecutionFinish,
    ExecutionPin, ExecutionRecord, ExecutionStartReceipt, GcReport, InstallCandidateRequest,
    LibraryUpdate, OrphanRecovery, RenameDocumentRequest, ReplayExecutionStart, RetentionPolicy,
    SaveDocumentOutcome, SaveDocumentRequest, SourceRollbackRequest, StorageError,
};

const WRITER_BATCH_LIMIT: usize = 32;

/// 只在 crate 内传递的 writer 工作项；它不会成为外部 storage API 的一部分。
pub(crate) enum WriterCommand {
    Append {
        request: AppendRequest,
        reply: oneshot::Sender<Result<CommitReceipt, StorageError>>,
    },
    StageCandidate {
        draft: Box<CandidateDraft>,
        reply: oneshot::Sender<Result<CandidateSummary, StorageError>>,
    },
    StageSourceRollback {
        request: SourceRollbackRequest,
        reply: oneshot::Sender<Result<CandidateSummary, StorageError>>,
    },
    InstallCandidate {
        request: InstallCandidateRequest,
        reply: oneshot::Sender<Result<crate::types::InstalledSource, StorageError>>,
    },
    StartExecution {
        request: crate::types::ExecutionStart,
        reply: oneshot::Sender<Result<ExecutionStartReceipt, StorageError>>,
    },
    StartReplayExecution {
        request: Box<ReplayExecutionStart>,
        reply: oneshot::Sender<Result<ExecutionRecord, StorageError>>,
    },
    CommitDelta {
        request: Box<DeltaCommit>,
        reply: oneshot::Sender<Result<CommitReceipt, StorageError>>,
    },
    FinishExecution {
        request: ExecutionFinish,
        reply: oneshot::Sender<Result<ExecutionRecord, StorageError>>,
    },
    SetExecutionPin {
        request: ExecutionPin,
        reply: oneshot::Sender<Result<ExecutionRecord, StorageError>>,
    },
    UpdateLibrary {
        request: LibraryUpdate,
        reply: oneshot::Sender<Result<CommitReceipt, StorageError>>,
    },
    PersistEffect {
        capture: EffectCapture,
        reply: oneshot::Sender<Result<DurableCaptureReceipt, StorageError>>,
    },
    PersistControlTrace {
        capture: ControlTraceCapture,
        reply: oneshot::Sender<Result<ControlTraceReceipt, StorageError>>,
    },
    CheckpointSource {
        source_identity: String,
        created_at_ms: i64,
        reply: oneshot::Sender<Result<CheckpointReceipt, StorageError>>,
    },
    CheckpointLibrary {
        created_at_ms: i64,
        reply: oneshot::Sender<Result<CheckpointReceipt, StorageError>>,
    },
    RunGc {
        policy: RetentionPolicy,
        now_ms: i64,
        reply: oneshot::Sender<Result<GcReport, StorageError>>,
    },
    ClearExecutionArchive {
        execution_id: Uuid,
        confirm_pinned: bool,
        now_ms: i64,
        reply: oneshot::Sender<Result<GcReport, StorageError>>,
    },
    CreateNativeDocument {
        request: CreateDocumentRequest,
        reply: oneshot::Sender<Result<DocumentSummary, StorageError>>,
    },
    SaveNativeDocument {
        request: SaveDocumentRequest,
        reply: oneshot::Sender<Result<SaveDocumentOutcome, StorageError>>,
    },
    RestoreNativeDocument {
        request: crate::types::RestoreDocumentRevisionRequest,
        reply: oneshot::Sender<Result<crate::types::RestoreDocumentRevisionOutcome, StorageError>>,
    },
    RenameNativeDocument {
        request: RenameDocumentRequest,
        reply: oneshot::Sender<Result<DocumentSummary, StorageError>>,
    },
    DeleteNativeDocument {
        request: DeleteDocumentRequest,
        reply: oneshot::Sender<Result<(), StorageError>>,
    },
    RecoverOrphans(oneshot::Sender<Result<OrphanRecovery, StorageError>>),
    Shutdown(oneshot::Sender<Result<(), StorageError>>),
}

/// 在 Tokio task 运行 async writer，关闭时先释放 `SQLite` 句柄再确认回复。
pub(crate) async fn writer_loop(
    mut receiver: mpsc::Receiver<WriterCommand>,
    mut conn: DatabaseSession,
    artifacts: ArtifactStore,
) {
    let shutdown_reply = 'writer: loop {
        let Some(first) = receiver.recv().await else {
            break None;
        };
        let mut batch = vec![first];
        while batch.len() < WRITER_BATCH_LIMIT {
            match receiver.try_recv() {
                Ok(command) => batch.push(command),
                Err(mpsc::error::TryRecvError::Empty | mpsc::error::TryRecvError::Disconnected) => {
                    break;
                }
            }
        }
        for command in batch {
            if let Some(reply) = handle_writer_command(command, &mut conn, &artifacts).await {
                break 'writer Some(reply);
            }
        }
    };
    // 先释放 SQLite 文件句柄，再确认 shutdown，避免 Windows 重开同一数据库时被锁住。
    drop(conn);
    if let Some(reply) = shutdown_reply {
        let _ = reply.send(Ok(()));
    }
}

async fn handle_writer_command(
    command: WriterCommand,
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
) -> Option<oneshot::Sender<Result<(), StorageError>>> {
    match command {
        WriterCommand::Append { request, reply } => {
            let _ = reply.send(crate::transaction::event::append(conn, artifacts, request).await);
        }
        WriterCommand::StageCandidate { draft, reply } => {
            let _ = reply.send(crate::transaction::candidate::stage(conn, artifacts, *draft).await);
        }
        WriterCommand::StageSourceRollback { request, reply } => {
            let _ = reply.send(
                crate::transaction::candidate::stage_source_rollback(conn, artifacts, request)
                    .await,
            );
        }
        WriterCommand::InstallCandidate { request, reply } => {
            let _ =
                reply.send(crate::transaction::candidate::install(conn, artifacts, request).await);
        }
        WriterCommand::StartExecution { request, reply } => {
            let _ =
                reply.send(crate::transaction::execution::start(conn, artifacts, request).await);
        }
        WriterCommand::StartReplayExecution { request, reply } => {
            let _ = reply
                .send(crate::transaction::execution::start_replay(conn, artifacts, *request).await);
        }
        WriterCommand::CommitDelta { request, reply } => {
            let _ = reply.send(crate::transaction::execution::commit_delta(conn, *request).await);
        }
        WriterCommand::FinishExecution { request, reply } => {
            let _ = reply.send(crate::transaction::execution::finish(conn, request).await);
        }
        WriterCommand::SetExecutionPin { request, reply } => {
            let _ = reply.send(crate::transaction::execution::set_pin(conn, request).await);
        }
        WriterCommand::UpdateLibrary { request, reply } => {
            let _ = reply.send(crate::transaction::projection::update_library(conn, request).await);
        }
        WriterCommand::PersistEffect { capture, reply } => {
            let _ = reply
                .send(crate::transaction::archive::persist_effect(conn, artifacts, capture).await);
        }
        WriterCommand::PersistControlTrace { capture, reply } => {
            let _ = reply.send(crate::transaction::archive::persist_control(conn, capture).await);
        }
        WriterCommand::CheckpointSource {
            source_identity,
            created_at_ms,
            reply,
        } => {
            let _ = reply.send(
                crate::transaction::maintenance::checkpoint_source(
                    conn,
                    artifacts,
                    &source_identity,
                    created_at_ms,
                )
                .await,
            );
        }
        WriterCommand::CheckpointLibrary {
            created_at_ms,
            reply,
        } => {
            let _ = reply.send(
                crate::transaction::maintenance::checkpoint_library(conn, artifacts, created_at_ms)
                    .await,
            );
        }
        WriterCommand::RunGc {
            policy,
            now_ms,
            reply,
        } => {
            let _ = reply.send(
                crate::transaction::maintenance::run_gc(conn, artifacts, policy, now_ms).await,
            );
        }
        WriterCommand::ClearExecutionArchive {
            execution_id,
            confirm_pinned,
            now_ms,
            reply,
        } => {
            let _ = reply.send(
                crate::transaction::maintenance::clear_execution_archive(
                    conn,
                    artifacts,
                    execution_id,
                    confirm_pinned,
                    now_ms,
                )
                .await,
            );
        }
        WriterCommand::RecoverOrphans(reply) => {
            let _ =
                reply.send(crate::transaction::maintenance::recover_orphans(conn, artifacts).await);
        }
        WriterCommand::CreateNativeDocument { request, reply } => {
            let _ =
                reply.send(crate::transaction::document::create(conn, artifacts, request).await);
        }
        WriterCommand::SaveNativeDocument { request, reply } => {
            let _ = reply.send(crate::transaction::document::save(conn, artifacts, request).await);
        }
        WriterCommand::RestoreNativeDocument { request, reply } => {
            let _ = reply.send(
                crate::transaction::document::restore_native_revision(conn, artifacts, request)
                    .await,
            );
        }
        WriterCommand::RenameNativeDocument { request, reply } => {
            let _ = reply.send(crate::transaction::document::rename(conn, request).await);
        }
        WriterCommand::DeleteNativeDocument { request, reply } => {
            let _ = reply.send(crate::transaction::document::delete(conn, request).await);
        }
        WriterCommand::Shutdown(reply) => return Some(reply),
    }
    None
}
