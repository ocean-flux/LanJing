//! 有界 single writer 的命令协议与 blocking loop。
//!
//! 只有此模块启动的专用线程持有 Diesel 写连接；async 调用方通过固定容量 `mpsc` 排队，
//! 因此 16 路执行不会直接争抢 `SQLite` writer lock。batch 仅降低唤醒开销，**每条命令仍在
//! 自己的 transaction 中**提交 Event、expected-version、global sequence 与投影，不能跨命令
//! 合并事务或改变 receipt 的顺序边界。

use diesel::sqlite::SqliteConnection;
use lj_runtime::{DurableCaptureReceipt, EffectCapture};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::artifact::ArtifactStore;
use crate::candidate_install::{
    process_install_candidate, process_stage_candidate, process_stage_source_credentials,
};
use crate::event_store::process_append;
use crate::execution::{
    process_delta, process_finish_execution, process_pin_execution, process_start_execution,
    process_start_replay_execution,
};
use crate::projection_query::process_library_update;
use crate::retention_recovery::{
    process_checkpoint_library, process_checkpoint_source, process_clear_execution_archive,
    process_gc, recover_orphans_sync,
};
use crate::types::{
    AppendRequest, CandidateDraft, CandidateSummary, CheckpointReceipt, CommitReceipt, DeltaCommit,
    ExecutionFinish, ExecutionPin, ExecutionRecord, GcReport, InstallCandidateRequest,
    LibraryUpdate, OrphanRecovery, ReplayExecutionStart, RetentionPolicy, SourceCredentialInput,
    SourceCredentialSnapshot, StorageError,
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
    StageSourceCredentials {
        input: Box<SourceCredentialInput>,
        reply: oneshot::Sender<Result<SourceCredentialSnapshot, StorageError>>,
    },
    InstallCandidate {
        request: InstallCandidateRequest,
        reply: oneshot::Sender<Result<crate::types::InstalledSource, StorageError>>,
    },
    StartExecution {
        request: crate::types::ExecutionStart,
        reply: oneshot::Sender<Result<ExecutionRecord, StorageError>>,
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
    RecoverOrphans(oneshot::Sender<Result<OrphanRecovery, StorageError>>),
    Shutdown(oneshot::Sender<Result<(), StorageError>>),
}

/// 在专用 OS 线程运行 writer，关闭时先释放 `SQLite` 句柄再确认回复。
pub(crate) fn writer_loop(
    mut receiver: mpsc::Receiver<WriterCommand>,
    mut conn: SqliteConnection,
    artifacts: &ArtifactStore,
) {
    let shutdown_reply = 'writer: loop {
        let Some(first) = receiver.blocking_recv() else {
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
            if let Some(reply) = handle_writer_command(command, &mut conn, artifacts) {
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

fn handle_writer_command(
    command: WriterCommand,
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
) -> Option<oneshot::Sender<Result<(), StorageError>>> {
    match command {
        WriterCommand::Append { request, reply } => {
            let _ = reply.send(process_append(conn, artifacts, request));
        }
        WriterCommand::StageCandidate { draft, reply } => {
            let _ = reply.send(process_stage_candidate(conn, artifacts, *draft));
        }
        WriterCommand::StageSourceCredentials { input, reply } => {
            let _ = reply.send(process_stage_source_credentials(
                conn,
                artifacts,
                input.as_ref(),
            ));
        }
        WriterCommand::InstallCandidate { request, reply } => {
            let _ = reply.send(process_install_candidate(conn, artifacts, request));
        }
        WriterCommand::StartExecution { request, reply } => {
            let _ = reply.send(process_start_execution(conn, request));
        }
        WriterCommand::StartReplayExecution { request, reply } => {
            let _ = reply.send(process_start_replay_execution(conn, artifacts, *request));
        }
        WriterCommand::CommitDelta { request, reply } => {
            let _ = reply.send(process_delta(conn, *request));
        }
        WriterCommand::FinishExecution { request, reply } => {
            let _ = reply.send(process_finish_execution(conn, request));
        }
        WriterCommand::SetExecutionPin { request, reply } => {
            let _ = reply.send(process_pin_execution(conn, request));
        }
        WriterCommand::UpdateLibrary { request, reply } => {
            let _ = reply.send(process_library_update(conn, request));
        }
        WriterCommand::PersistEffect { capture, reply } => {
            let _ = reply.send(crate::execution_archive::persist_effect_capture(
                conn, artifacts, capture,
            ));
        }
        WriterCommand::CheckpointSource {
            source_identity,
            created_at_ms,
            reply,
        } => {
            let _ = reply.send(process_checkpoint_source(
                conn,
                artifacts,
                &source_identity,
                created_at_ms,
            ));
        }
        WriterCommand::CheckpointLibrary {
            created_at_ms,
            reply,
        } => {
            let _ = reply.send(process_checkpoint_library(conn, artifacts, created_at_ms));
        }
        WriterCommand::RunGc {
            policy,
            now_ms,
            reply,
        } => {
            let _ = reply.send(process_gc(conn, artifacts, policy, now_ms));
        }
        WriterCommand::ClearExecutionArchive {
            execution_id,
            confirm_pinned,
            now_ms,
            reply,
        } => {
            let _ = reply.send(process_clear_execution_archive(
                conn,
                artifacts,
                execution_id,
                confirm_pinned,
                now_ms,
            ));
        }
        WriterCommand::RecoverOrphans(reply) => {
            let _ = reply.send(recover_orphans_sync(conn, artifacts));
        }
        WriterCommand::Shutdown(reply) => return Some(reply),
    }
    None
}
