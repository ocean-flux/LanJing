//! 测试专用的 effect witness archive 读取边界。
//!
//! 此模块只在 `test-support` 编译。它用 C2 的真实 [`EffectArchive`] seam 读取已 durable 的
//! capture；不会公开 Plan node、effect topology、body、cookie、token、authorization、URL
//! query 或 storage handle。
//!
//! 不变量：lookup 仅在 `EffectCaptured` 已从 C2 catch-up 后登记；因此测试 reader 不会把未持久
//! 化的 effect 伪装成 archive，也不会触发 live handler 或网络请求。

#[cfg(feature = "test-support")]
use lj_runtime::{EffectArchive, EffectReplayLookup};
#[cfg(feature = "test-support")]
use uuid::Uuid;

#[cfg(feature = "test-support")]
use super::{RuleSystem, RuleSystemState, lock, trace_id};
#[cfg(feature = "test-support")]
use crate::{
    EffectWitnessCaptureForTest, EffectWitnessForTest, ExecutionId, ExtractEffectWitnessForTest,
    HttpDnsTargetKindForTest, HttpDnsTargetWitnessForTest, HttpEffectErrorKindForTest,
    HttpEffectWitnessForTest, HttpMethodForTest, HttpRedirectWitnessForTest,
    HttpRequestBodyWitnessForTest, HttpRequestHeaderWitnessForTest, HttpRequestWitnessForTest,
    QuickJsEffectWitnessForTest, QuickJsErrorKindForTest, QuickJsHostCallForTest,
    QuickJsHostCallWitnessForTest, RuleError, RuleErrorStage,
};

/// 将已 durable 的 runtime capture 保留为 test-only、私有 archive lookup。
///
/// session delivery 只会在 C2 `flush_persisted` 成功之后调用本函数；故记录 lookup 不改变
/// 生产 lifecycle，也不会使未提交 effect 被 reader 观察到。
#[cfg(feature = "test-support")]
pub(super) fn remember_effect_capture(
    state: &RuleSystemState,
    execution_id: Uuid,
    effect_id: Uuid,
    lookup: EffectReplayLookup,
) {
    lock(&state.effect_capture_lookups).insert((execution_id, effect_id), lookup);
}

#[cfg(feature = "test-support")]
impl RuleSystem {
    /// 读取指定 live capture 的脱敏 witness；仅供 `test-support` golden contract 使用。
    ///
    /// 调用方只能提供 opaque execution/effect ID。Plan node、effect kind、body、secret 与 C2
    /// lookup 均保持私有；本方法通过正常 [`EffectArchive::load_replay`] seam 验证 archive，
    /// 不会重放或调用任何 live handler。
    ///
    /// # Errors
    ///
    /// capture 不属于当前进程记录、archive 缺失/篡改或 witness 完整性校验失败时返回
    /// [`RuleError`]。
    pub async fn read_effect_witness_for_test(
        &self,
        execution_id: ExecutionId,
        effect_id: Uuid,
    ) -> Result<EffectWitnessCaptureForTest, RuleError> {
        let trace_id = trace_id();
        let lookup = lock(&self.state.effect_capture_lookups)
            .get(&(execution_id.as_uuid(), effect_id))
            .cloned()
            .ok_or_else(|| {
                RuleError::new(
                    RuleErrorStage::Replay,
                    "effect_witness_lookup_missing",
                    "当前进程未记录该 effect 的安全 archive lookup",
                    trace_id.clone(),
                    false,
                    Vec::new(),
                )
            })?;
        let capture = self
            .state
            .storage
            .load_replay(lookup)
            .await
            .map_err(|_| {
                RuleError::new(
                    RuleErrorStage::Replay,
                    "effect_witness_unavailable",
                    "历史 effect witness 不可读取",
                    trace_id.clone(),
                    false,
                    Vec::new(),
                )
            })?
            .ok_or_else(|| {
                RuleError::new(
                    RuleErrorStage::Replay,
                    "effect_witness_missing",
                    "历史 execution 缺少 effect witness",
                    trace_id.clone(),
                    false,
                    Vec::new(),
                )
            })?;
        if capture.execution_id != execution_id.as_uuid() || capture.effect_id != effect_id {
            return Err(RuleError::new(
                RuleErrorStage::Replay,
                "effect_witness_identity_mismatch",
                "历史 effect witness 身份不一致",
                trace_id,
                false,
                Vec::new(),
            ));
        }
        capture.validate_replay_integrity().map_err(|_| {
            RuleError::new(
                RuleErrorStage::Replay,
                "effect_witness_invalid",
                "历史 effect witness 完整性校验失败",
                trace_id,
                false,
                Vec::new(),
            )
        })?;
        Ok(effect_witness_capture_for_test(
            capture.witness_hash,
            capture.witness,
        ))
    }
}

#[cfg(feature = "test-support")]
fn effect_witness_capture_for_test(
    witness_hash: String,
    witness: lj_runtime::EffectWitness,
) -> EffectWitnessCaptureForTest {
    EffectWitnessCaptureForTest {
        witness_hash,
        witness: effect_witness_for_test(witness),
    }
}

#[cfg(feature = "test-support")]
fn effect_witness_for_test(witness: lj_runtime::EffectWitness) -> EffectWitnessForTest {
    match witness {
        lj_runtime::EffectWitness::Http(witness) => {
            let lj_runtime::HttpEffectWitness {
                request,
                redirects,
                dns_targets,
                error,
                duration_ms,
            } = witness;
            let lj_runtime::HttpRequestWitness {
                method,
                safe_url,
                headers,
                body,
            } = request;
            EffectWitnessForTest::Http(HttpEffectWitnessForTest {
                request: HttpRequestWitnessForTest {
                    method: match method {
                        lj_rule_model::HttpMethod::Get => HttpMethodForTest::Get,
                        lj_rule_model::HttpMethod::Post => HttpMethodForTest::Post,
                    },
                    safe_url,
                    headers: headers
                        .into_iter()
                        .map(|header| HttpRequestHeaderWitnessForTest {
                            name: header.name,
                            value_hash: header.value_hash,
                        })
                        .collect(),
                    body: body.map(|body| HttpRequestBodyWitnessForTest {
                        hash: body.hash,
                        byte_len: body.byte_len,
                    }),
                },
                redirects: redirects
                    .into_iter()
                    .map(|redirect| HttpRedirectWitnessForTest {
                        status: redirect.status,
                        from_url: redirect.from_url,
                        to_url: redirect.to_url,
                    })
                    .collect(),
                dns_targets: dns_targets
                    .into_iter()
                    .map(|target| HttpDnsTargetWitnessForTest {
                        host: target.host,
                        addresses: target.addresses,
                        kind: match target.kind {
                            lj_runtime::HttpDnsTargetKind::PinnedDns => {
                                HttpDnsTargetKindForTest::PinnedDns
                            }
                            lj_runtime::HttpDnsTargetKind::IpLiteral => {
                                HttpDnsTargetKindForTest::IpLiteral
                            }
                            lj_runtime::HttpDnsTargetKind::DirectHost => {
                                HttpDnsTargetKindForTest::DirectHost
                            }
                        },
                    })
                    .collect(),
                error: error.map(|error| match error {
                    lj_runtime::HttpEffectErrorKind::TargetValidation => {
                        HttpEffectErrorKindForTest::TargetValidation
                    }
                    lj_runtime::HttpEffectErrorKind::Request => HttpEffectErrorKindForTest::Request,
                    lj_runtime::HttpEffectErrorKind::Redirect => {
                        HttpEffectErrorKindForTest::Redirect
                    }
                    lj_runtime::HttpEffectErrorKind::ResponseRead => {
                        HttpEffectErrorKindForTest::ResponseRead
                    }
                }),
                duration_ms,
            })
        }
        lj_runtime::EffectWitness::QuickJs(witness) => {
            EffectWitnessForTest::QuickJs(QuickJsEffectWitnessForTest {
                script_hash: witness.script_hash,
                input_hash: witness.input_hash,
                output_hash: witness.output_hash,
                error: witness.error.map(|error| match error {
                    lj_runtime::QuickJsErrorKind::Evaluation => QuickJsErrorKindForTest::Evaluation,
                    lj_runtime::QuickJsErrorKind::RuntimeInitialization => {
                        QuickJsErrorKindForTest::RuntimeInitialization
                    }
                    lj_runtime::QuickJsErrorKind::ContextInitialization => {
                        QuickJsErrorKindForTest::ContextInitialization
                    }
                    lj_runtime::QuickJsErrorKind::Timeout => QuickJsErrorKindForTest::Timeout,
                    lj_runtime::QuickJsErrorKind::Watchdog => QuickJsErrorKindForTest::Watchdog,
                    lj_runtime::QuickJsErrorKind::WorkerFailure => {
                        QuickJsErrorKindForTest::WorkerFailure
                    }
                }),
                host_calls: witness
                    .host_calls
                    .into_iter()
                    .map(|call| QuickJsHostCallWitnessForTest {
                        sequence: call.sequence,
                        call: match call.call {
                            lj_runtime::QuickJsHostCall::Time { epoch_millis } => {
                                QuickJsHostCallForTest::Time { epoch_millis }
                            }
                            lj_runtime::QuickJsHostCall::Random { value_bits } => {
                                QuickJsHostCallForTest::Random { value_bits }
                            }
                        },
                    })
                    .collect(),
                duration_ms: witness.duration_ms,
            })
        }
        lj_runtime::EffectWitness::Extract(witness) => {
            EffectWitnessForTest::Extract(ExtractEffectWitnessForTest {
                input_hash: witness.input_hash,
                duration_ms: witness.duration_ms,
            })
        }
    }
}
