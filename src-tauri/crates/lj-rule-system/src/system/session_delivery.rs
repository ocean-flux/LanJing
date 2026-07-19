//! execution session 的持久化、delivery 与 catch-up。
//!
//! 这里是 `RuleSystem` 的唯一 session runner。它保持以下不变量：
//!
//! - 每个 execution 只有一个持久 terminal；runtime stream 异常结束也会尝试写入 `Failed`；
//! - `MediaGraphDelta` 必须先由 C2 在 Event + projection transaction 中提交，再向 delivery 发送；
//! - 取消只阻止后续 effect；已提交 Delta 保持先于 `Cancelled` 的可观察顺序；
//! - delivery stream 被丢弃不能取消 execution；bounded sender 只影响 delivery 背压；
//! - catch-up 只接受严格连续的 C2 stream sequence，绝不补造缺失事件。

use std::collections::BTreeMap;
use std::sync::Arc;

use futures::StreamExt;
use lj_importer::legado::{ContinueActionError, LegadoImporter};
use lj_media::MediaGraphDelta;
use lj_rule_model::EventType;
#[cfg(feature = "test-support")]
use lj_runtime::EffectReplayLookup;
use lj_runtime::{ExecutionEventKind as RuntimeEventKind, ExecutionMode as RuntimeExecutionMode};
use lj_storage::{
    AppendRequest, DeltaCommit, EventProjectionStorage, ExecutionFinish, ExecutionStatus,
    ProjectionDelta, ProjectionTombstones, StoredEvent,
};
use serde_json::Value;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::error_mapping::{runtime_failure_error, storage_error};
use super::{RuleSystemState, lock, now_millis};
use crate::{ExecutionEvent, ExecutionEventKind, ExecutionId, RuleError, RuleErrorStage, SourceId};

/// 交给 detached runtime session runner 的私有上下文。
pub(super) struct SessionRun {
    pub(super) source_identity: String,
    pub(super) replay_continue_actions: Option<BTreeMap<String, Value>>,
    pub(super) runtime_events: futures::stream::BoxStream<'static, lj_runtime::ExecutionEvent>,
    pub(super) delivery_sender: mpsc::Sender<ExecutionEvent>,
    pub(super) persisted_sequence: u64,
    pub(super) trace_id: String,
}

/// 消费 runtime event、持久化状态，并仅 delivery 已提交事件。
///
/// 取消与 terminal 的竞争由 `terminal_observed` 收敛：一旦持久终态获胜，runner 立即结束，
/// 并从私有 cancellation registry 删除 execution，避免 detached registry 条目泄漏。
pub(super) async fn run_session(
    state: Arc<RuleSystemState>,
    execution_id: Uuid,
    SessionRun {
        source_identity,
        replay_continue_actions,
        mut runtime_events,
        delivery_sender,
        persisted_sequence,
        trace_id,
    }: SessionRun,
) {
    let mut persisted_sequence = persisted_sequence;
    let mut terminal_observed = false;
    while let Some(event) = runtime_events.next().await {
        let result = match event.kind {
            RuntimeEventKind::Started | RuntimeEventKind::EffectReplayed { .. } => Ok(()),
            #[cfg(feature = "test-support")]
            RuntimeEventKind::EffectCaptured {
                node_id,
                effect_id,
                kind,
                ..
            } => {
                let result = flush_persisted(
                    &state.storage,
                    execution_id,
                    &mut persisted_sequence,
                    &delivery_sender,
                    &trace_id,
                )
                .await;
                if result.is_ok() {
                    super::capture::remember_effect_capture(
                        &state,
                        execution_id,
                        effect_id,
                        EffectReplayLookup {
                            archived_execution_id: execution_id,
                            node_id,
                            kind,
                        },
                    );
                }
                result
            }
            #[cfg(not(feature = "test-support"))]
            RuntimeEventKind::EffectCaptured { .. } => {
                flush_persisted(
                    &state.storage,
                    execution_id,
                    &mut persisted_sequence,
                    &delivery_sender,
                    &trace_id,
                )
                .await
            }
            RuntimeEventKind::DeltaProduced { delta, .. } => {
                match now_millis(&trace_id).and_then(|now_ms| {
                    seal_legacy_continue_actions(
                        delta,
                        &source_identity,
                        replay_continue_actions.as_ref(),
                        now_ms,
                        &trace_id,
                    )
                }) {
                    Ok(delta) => {
                        commit_delta(
                            &state.storage,
                            execution_id,
                            persisted_sequence,
                            delta,
                            &delivery_sender,
                            &mut persisted_sequence,
                            &trace_id,
                        )
                        .await
                    }
                    Err(error) => Err(error),
                }
            }
            RuntimeEventKind::Completed => {
                terminal_observed = true;
                finish_execution(
                    &state.storage,
                    execution_id,
                    persisted_sequence,
                    ExecutionStatus::Completed,
                    &delivery_sender,
                    &mut persisted_sequence,
                    &trace_id,
                )
                .await
            }
            RuntimeEventKind::Cancelled => {
                terminal_observed = true;
                finish_execution(
                    &state.storage,
                    execution_id,
                    persisted_sequence,
                    ExecutionStatus::Cancelled,
                    &delivery_sender,
                    &mut persisted_sequence,
                    &trace_id,
                )
                .await
            }
            RuntimeEventKind::Failed { failure } => {
                terminal_observed = true;
                persist_runtime_failure(
                    &state.storage,
                    execution_id,
                    &source_identity,
                    runtime_failure_error(failure.code, &trace_id),
                    &delivery_sender,
                    &mut persisted_sequence,
                    &trace_id,
                )
                .await
            }
        };
        if let Err(error) = result {
            terminal_observed = true;
            persist_runner_failure(
                &state.storage,
                execution_id,
                &source_identity,
                error,
                &delivery_sender,
                &mut persisted_sequence,
                &trace_id,
            )
            .await;
            break;
        }
        if terminal_observed {
            break;
        }
    }
    if !terminal_observed {
        persist_runner_failure(
            &state.storage,
            execution_id,
            &source_identity,
            RuleError::new(
                RuleErrorStage::Execution,
                "runtime_stream_ended",
                "运行时在未发送终态前结束",
                trace_id.clone(),
                false,
                Vec::new(),
            ),
            &delivery_sender,
            &mut persisted_sequence,
            &trace_id,
        )
        .await;
    }
    lock(&state.executions).remove(&execution_id);
}

fn seal_legacy_continue_actions(
    mut delta: MediaGraphDelta,
    source_identity: &str,
    replay_payloads: Option<&BTreeMap<String, Value>>,
    now_ms: i64,
    trace_id: &str,
) -> Result<MediaGraphDelta, RuleError> {
    if !LegadoImporter::owns_source(source_identity) {
        return Ok(delta);
    }
    for action in &mut delta.actions {
        if action.intent != lj_capability::StandardIntent::ContinueAction {
            continue;
        }
        action.payload = match replay_payloads {
            Some(payloads) => payloads.get(&action.id.0).cloned().ok_or_else(|| {
                RuleError::new(
                    RuleErrorStage::Replay,
                    "replay_continue_action_missing",
                    "历史 execution 缺少继续动作 archive",
                    trace_id.to_string(),
                    false,
                    Vec::new(),
                )
            })?,
            None => LegadoImporter::seal_continue_action_payload(
                &action.payload,
                source_identity,
                now_ms,
            )
            .map_err(|error| continue_action_error(error, RuleErrorStage::Execution, trace_id))?,
        };
    }
    Ok(delta)
}

/// 将 Legado action 解析错误保持为安全、稳定的 façade error。
pub(super) fn continue_action_error(
    error: ContinueActionError,
    stage: RuleErrorStage,
    trace_id: &str,
) -> RuleError {
    RuleError::new(
        stage,
        error.code(),
        error.safe_message(),
        trace_id.to_string(),
        false,
        Vec::new(),
    )
}

/// 在 C2 同一 transaction 中提交 Delta 与投影，成功后才可 delivery。
async fn commit_delta(
    storage: &EventProjectionStorage,
    execution_id: Uuid,
    expected_version: u64,
    delta: MediaGraphDelta,
    delivery_sender: &mpsc::Sender<ExecutionEvent>,
    persisted_sequence: &mut u64,
    trace_id: &str,
) -> Result<(), RuleError> {
    storage
        .commit_execution_delta(DeltaCommit {
            execution_id,
            expected_version,
            event_id: Uuid::new_v4(),
            trace_id: trace_id.to_string(),
            occurred_at_ms: now_millis(trace_id)?,
            delta: ProjectionDelta {
                upserts: delta,
                tombstones: ProjectionTombstones::default(),
            },
        })
        .await
        .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, trace_id))?;
    flush_persisted(
        storage,
        execution_id,
        persisted_sequence,
        delivery_sender,
        trace_id,
    )
    .await
}

async fn persist_runtime_failure(
    storage: &EventProjectionStorage,
    execution_id: Uuid,
    source_identity: &str,
    error: RuleError,
    delivery_sender: &mpsc::Sender<ExecutionEvent>,
    persisted_sequence: &mut u64,
    trace_id: &str,
) -> Result<(), RuleError> {
    append_diagnostic(
        storage,
        execution_id,
        source_identity,
        *persisted_sequence,
        &error,
        trace_id,
    )
    .await?;
    flush_persisted(
        storage,
        execution_id,
        persisted_sequence,
        delivery_sender,
        trace_id,
    )
    .await?;
    finish_execution(
        storage,
        execution_id,
        *persisted_sequence,
        ExecutionStatus::Failed,
        delivery_sender,
        persisted_sequence,
        trace_id,
    )
    .await
}

async fn append_diagnostic(
    storage: &EventProjectionStorage,
    execution_id: Uuid,
    source_identity: &str,
    expected_version: u64,
    error: &RuleError,
    trace_id: &str,
) -> Result<u64, RuleError> {
    let receipt = storage
        .append_event(AppendRequest {
            stream_id: format!("execution/{execution_id}"),
            expected_version,
            event_id: Uuid::new_v4(),
            event_type: EventType::Execution,
            schema_version: 1,
            correlation_id: None,
            causation_id: None,
            trace_id: trace_id.to_string(),
            occurred_at_ms: now_millis(trace_id)?,
            payload: serde_json::json!({
                "kind": "diagnostic",
                "code": error.code.clone(),
                "message": error.message.clone(),
            }),
            source_id: Some(source_identity.to_string()),
            artifacts: Vec::new(),
        })
        .await
        .map_err(|failure| storage_error(&failure, RuleErrorStage::Persistence, trace_id))?;
    Ok(receipt.stream_version)
}

/// 追加唯一 terminal 并随后 catch-up/delivery；terminal 不会绕过持久化直接发送。
async fn finish_execution(
    storage: &EventProjectionStorage,
    execution_id: Uuid,
    expected_version: u64,
    status: ExecutionStatus,
    delivery_sender: &mpsc::Sender<ExecutionEvent>,
    persisted_sequence: &mut u64,
    trace_id: &str,
) -> Result<(), RuleError> {
    storage
        .finish_execution(ExecutionFinish {
            execution_id,
            expected_version,
            event_id: Uuid::new_v4(),
            status,
            finished_at_ms: now_millis(trace_id)?,
            trace_id: trace_id.to_string(),
        })
        .await
        .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, trace_id))?;
    flush_persisted(
        storage,
        execution_id,
        persisted_sequence,
        delivery_sender,
        trace_id,
    )
    .await
}

async fn persist_runner_failure(
    storage: &EventProjectionStorage,
    execution_id: Uuid,
    source_identity: &str,
    error: RuleError,
    delivery_sender: &mpsc::Sender<ExecutionEvent>,
    sequence: &mut u64,
    trace_id: &str,
) {
    let result = append_diagnostic(
        storage,
        execution_id,
        source_identity,
        *sequence,
        &error,
        trace_id,
    )
    .await;
    if result.is_err() {
        // 持久化不可用时不能伪造 delivery；C2 重启恢复会将未终态 execution 标为 incomplete。
        return;
    }
    let _ = flush_persisted(storage, execution_id, sequence, delivery_sender, trace_id).await;
    let _ = finish_execution(
        storage,
        execution_id,
        *sequence,
        ExecutionStatus::Failed,
        delivery_sender,
        sequence,
        trace_id,
    )
    .await;
}

/// 从 C2 读取连续 execution stream，拒绝所有 sequence 洞。
pub(super) async fn catch_up_execution(
    storage: &EventProjectionStorage,
    execution_id: ExecutionId,
    after_sequence: u64,
) -> Result<Vec<ExecutionEvent>, RuleError> {
    let trace_id = super::trace_id();
    let stored = storage
        .catch_up_execution(execution_id.as_uuid(), after_sequence)
        .await
        .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))?;
    let mut expected = after_sequence;
    let mut events = Vec::with_capacity(stored.len());
    for event in stored {
        if event.envelope.stream_version != expected.saturating_add(1) {
            return Err(RuleError::new(
                RuleErrorStage::Persistence,
                "catch_up_sequence_gap",
                "execution catch-up 的持久序列不连续",
                trace_id,
                false,
                Vec::new(),
            ));
        }
        expected = event.envelope.stream_version;
        events.push(map_stored_event(event, execution_id, &trace_id)?);
    }
    Ok(events)
}

/// 仅 delivery 已从 C2 读回的事件；发送失败表示订阅者已丢弃，不能取消 execution。
pub(super) async fn flush_persisted(
    storage: &EventProjectionStorage,
    execution_id: Uuid,
    sequence: &mut u64,
    delivery_sender: &mpsc::Sender<ExecutionEvent>,
    _trace_id: &str,
) -> Result<(), RuleError> {
    let events =
        catch_up_execution(storage, ExecutionId::from_uuid(execution_id), *sequence).await?;
    for event in events {
        *sequence = event.sequence;
        let _ = delivery_sender.send(event).await;
    }
    Ok(())
}

fn map_stored_event(
    stored: StoredEvent,
    execution_id: ExecutionId,
    trace_id: &str,
) -> Result<ExecutionEvent, RuleError> {
    let occurred_at_ms = stored.envelope.occurred_at.parse::<i64>().map_err(|_| {
        RuleError::new(
            RuleErrorStage::Persistence,
            "event_timestamp_invalid",
            "持久 execution event 的时间格式无效",
            trace_id.to_string(),
            false,
            Vec::new(),
        )
    })?;
    let kind = stored
        .envelope
        .payload
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let event_kind = match kind {
        "started" | "replay_started" => ExecutionEventKind::Started,
        "diagnostic" => ExecutionEventKind::Diagnostic {
            code: string_payload(&stored.envelope.payload, "code")
                .unwrap_or_else(|| "execution_diagnostic".to_string()),
            message: string_payload(&stored.envelope.payload, "message")
                .unwrap_or_else(|| "execution 诊断".to_string()),
        },
        "effect_captured" => ExecutionEventKind::EffectCaptured {
            effect_id: uuid_payload(&stored.envelope.payload, "effect_id", trace_id)?,
            artifact_refs: stored.envelope.artifact_refs,
            output_hash: string_payload(&stored.envelope.payload, "output_hash")
                .unwrap_or_default(),
        },
        "delta" => {
            let value = stored
                .envelope
                .payload
                .get("delta")
                .cloned()
                .ok_or_else(|| {
                    RuleError::new(
                        RuleErrorStage::Persistence,
                        "delta_payload_missing",
                        "持久 Delta event 缺少标准增量",
                        trace_id.to_string(),
                        false,
                        Vec::new(),
                    )
                })?;
            let delta = serde_json::from_value::<ProjectionDelta>(value).map_err(|_| {
                RuleError::new(
                    RuleErrorStage::Persistence,
                    "delta_payload_invalid",
                    "持久 Delta event 无法还原",
                    trace_id.to_string(),
                    false,
                    Vec::new(),
                )
            })?;
            ExecutionEventKind::DeltaCommitted {
                global_revision: stored.envelope.global_seq,
                source_revision: stored.envelope.stream_version,
                delta: delta.upserts,
            }
        }
        "terminal" => terminal_kind(&stored.envelope.payload, trace_id)?,
        _ => ExecutionEventKind::Diagnostic {
            code: "unknown_execution_event".to_string(),
            message: "遇到未知的持久 execution event".to_string(),
        },
    };
    Ok(ExecutionEvent {
        execution_id,
        sequence: stored.envelope.stream_version,
        trace_id: stored.envelope.trace_id,
        occurred_at_ms,
        kind: event_kind,
    })
}

fn terminal_kind(payload: &Value, trace_id: &str) -> Result<ExecutionEventKind, RuleError> {
    match payload.get("status").and_then(Value::as_str) {
        Some("completed") => Ok(ExecutionEventKind::Completed),
        Some("cancelled") => Ok(ExecutionEventKind::Cancelled),
        Some("failed" | "incomplete") => Ok(ExecutionEventKind::Failed {
            error: RuleError::new(
                RuleErrorStage::Execution,
                "execution_failed",
                "execution 以失败终态结束",
                trace_id.to_string(),
                false,
                Vec::new(),
            ),
        }),
        _ => Err(RuleError::new(
            RuleErrorStage::Persistence,
            "terminal_payload_invalid",
            "持久 execution terminal 无法识别",
            trace_id.to_string(),
            false,
            Vec::new(),
        )),
    }
}

fn string_payload(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn uuid_payload(payload: &Value, key: &str, trace_id: &str) -> Result<Uuid, RuleError> {
    let value = string_payload(payload, key).ok_or_else(|| {
        RuleError::new(
            RuleErrorStage::Persistence,
            "effect_payload_invalid",
            "持久 effect event 缺少 effect ID",
            trace_id.to_string(),
            false,
            Vec::new(),
        )
    })?;
    Uuid::parse_str(&value).map_err(|_| {
        RuleError::new(
            RuleErrorStage::Persistence,
            "effect_payload_invalid",
            "持久 effect event 的 effect ID 无效",
            trace_id.to_string(),
            false,
            Vec::new(),
        )
    })
}

/// 验证 replay pin 的 source、mode 与最小来源快照一致。
pub(super) fn replay_snapshot(
    requested_source: &SourceId,
    archived_execution_id: ExecutionId,
    pin: &lj_storage::ExecutionReplayPin,
    trace_id: &str,
) -> Result<(), RuleError> {
    if requested_source.as_identity() != pin.source_identity {
        return Err(RuleError::new(
            RuleErrorStage::Replay,
            "replay_source_mismatch",
            "请求来源不属于历史 execution pin",
            trace_id.to_string(),
            false,
            Vec::new(),
        ));
    }
    if !matches!(pin.mode, RuntimeExecutionMode::Replay { archived_execution_id: value } if value == archived_execution_id.as_uuid())
    {
        return Err(RuleError::new(
            RuleErrorStage::Replay,
            "replay_mode_mismatch",
            "历史 execution pin 的 replay mode 无效",
            trace_id.to_string(),
            false,
            Vec::new(),
        ));
    }
    if pin.profile.id.0 != pin.source_identity || pin.base_url.trim().is_empty() {
        return Err(RuleError::new(
            RuleErrorStage::Replay,
            "replay_source_snapshot_invalid",
            "历史 execution 的固定来源快照无效",
            trace_id.to_string(),
            false,
            Vec::new(),
        ));
    }
    Ok(())
}
