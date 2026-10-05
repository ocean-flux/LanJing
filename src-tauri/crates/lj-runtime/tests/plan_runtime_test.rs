//! Plan runtime 的 focused contract tests。
//!
//! archive fixture 每次 capture 都写入并 `sync_all` 临时文件；它不是宣称 durable 的
//! no-op，而是验证 runtime 只在真实确认收据之后推进下游节点。

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use lj_capability::{IntentExport, IntentInput, StandardIntent};
use lj_compiler::Compiler;
use lj_rule_model::definition::MapperOutputKind;
use lj_rule_model::{
    CapabilityManifest, CollectionSelector, ConditionConfig, ConditionPredicate, ControlExpression,
    ControlTrace, ControlledMapper, EffectKind, ExecutionPlan, ExecutionPlanParts,
    ExpectedDataType, ExtractSpec, FlowEdge, FlowGraph, FlowNode, FlowNodeConfig, FlowPortRef,
    ForEachConfig, HttpMethod, HttpSpec, JsConfig, JsOutputKind, LINEAR_INPUT_HANDLE,
    LINEAR_OUTPUT_HANDLE, LOOP_BODY_HANDLE, LOOP_COLLECTION_HANDLE, LOOP_DONE_HANDLE,
    LOOP_YIELD_HANDLE, MAX_LOOP_ITERATIONS, MergeConfig, MergeInput, MergeInputActivation,
    MergeStrategy, PlanNode, PlanNodeConfig, RuleDefinition, SourceIdentity, SystemCapabilities,
    TypedLiteral, execution_plan_hash, read_execution_plan,
};
use lj_runtime::effect_registry::{EffectHandler, EffectRegistry, FrozenEffectRegistry, builtin};
use lj_runtime::{
    CapturedEffectOutput, ControlReplayLookup, ControlTraceCapture, ControlTraceReceipt,
    DurableCaptureReceipt, EffectArchive, EffectArchiveError, EffectCancellation, EffectCapture,
    EffectError, EffectErrorCode, EffectFailure, EffectOutput, EffectReplayLookup, EffectWitness,
    ExtractEffectHandler, ExtractEffectRequest, ExtractEffectWitness, ExtractOutput,
    HttpEffectErrorKind, HttpEffectHandler, HttpEffectRequest, HttpEffectWitness,
    HttpExecutionCredentials, HttpRequestWitness, HttpResponse, PlanExecutionRequest, PlanRuntime,
    PlanRuntimeConfig, PlanSupport, QuickJsEffectHandler, QuickJsEffectRequest,
    QuickJsEffectWitness, QuickJsErrorKind, QuickJsOutput, ReplayCompletionLookup,
    RuntimeFailureCode, effect_input_hash, effect_output_hash, quickjs_script_hash,
};
use tokio::sync::Notify;
use uuid::Uuid;

#[derive(Clone, Copy)]
enum QuickJsWitnessHashField {
    Script,
    Input,
    Output,
}

struct DurableFileArchive {
    captures: Mutex<Vec<EffectCapture>>,
    control_traces: Mutex<Vec<ControlTraceCapture>>,
    path: PathBuf,
    persisted: Arc<Notify>,
}

impl DurableFileArchive {
    fn new() -> Self {
        Self {
            captures: Mutex::new(Vec::new()),
            control_traces: Mutex::new(Vec::new()),
            path: std::env::temp_dir().join(format!("lj-runtime-capture-{}.jsonl", Uuid::new_v4())),
            persisted: Arc::new(Notify::new()),
        }
    }

    fn captures(&self) -> Vec<EffectCapture> {
        self.captures.lock().expect("archive capture mutex").clone()
    }

    fn control_traces(&self) -> Vec<ControlTraceCapture> {
        self.control_traces
            .lock()
            .expect("archive control trace mutex")
            .clone()
    }

    fn from_records(
        captures: Vec<EffectCapture>,
        control_traces: Vec<ControlTraceCapture>,
    ) -> Self {
        let archive = Self::new();
        *archive.captures.lock().expect("archive capture mutex") = captures;
        *archive
            .control_traces
            .lock()
            .expect("archive control trace mutex") = control_traces;
        archive
    }

    fn remove_invocation(&self, ordinal: u64) {
        self.captures
            .lock()
            .expect("archive capture mutex")
            .retain(|capture| capture.invocation_path.ordinal() != ordinal);
        self.control_traces
            .lock()
            .expect("archive control trace mutex")
            .retain(|capture| capture.invocation_path.ordinal() != ordinal);
    }

    fn append_extra_effect_invocation(&self) {
        let next_ordinal = self
            .captures()
            .iter()
            .map(|capture| capture.invocation_path.ordinal())
            .chain(
                self.control_traces()
                    .iter()
                    .map(|capture| capture.invocation_path.ordinal()),
            )
            .max()
            .expect("archive fixture has invocations")
            + 1;
        let mut captures = self.captures.lock().expect("archive capture mutex");
        let mut extra = captures
            .last()
            .cloned()
            .expect("archive fixture has an effect capture");
        extra.invocation_path = lj_rule_model::InvocationPath::new(
            extra.invocation_path.node_id(),
            extra.invocation_path.loop_iterations().to_vec(),
            next_ordinal,
        )
        .expect("valid extra invocation fixture");
        captures.push(extra);
    }

    fn tamper_first_condition_trace(&self) {
        let mut traces = self
            .control_traces
            .lock()
            .expect("archive control trace mutex");
        let trace = traces
            .iter_mut()
            .find(|capture| matches!(capture.trace, ControlTrace::Condition { .. }))
            .expect("archive fixture has a Condition trace");
        trace.trace = ControlTrace::Condition {
            branch: "beta".to_string(),
        };
        trace.trace_hash =
            lj_runtime::control_trace_hash(&trace.trace).expect("tampered trace remains canonical");
    }

    fn corrupt_first_output_hash(&self) {
        let mut captures = self.captures.lock().expect("archive capture mutex");
        captures
            .first_mut()
            .expect("live execution must have a capture")
            .output_hash = "corrupted-output-hash".to_string();
    }

    /// 只改历史输出内容, 不动 `output_hash` / `witness_hash`: 验证 replay 从内容重算 hash,
    /// 而不是相信 archive 自己记的 hash 字段。
    fn tamper_first_output_payload(&self) {
        let mut captures = self.captures.lock().expect("archive capture mutex");
        let capture = captures
            .first_mut()
            .expect("live execution must have a capture");
        let tampered = match capture.output.as_ref() {
            EffectOutput::Http(response) => EffectOutput::Http(HttpResponse {
                status: response.status,
                headers: response.headers.clone(),
                body: b"tampered-historical-body".to_vec(),
                charset: response.charset.clone(),
            }),
            EffectOutput::QuickJs(_) => EffectOutput::QuickJs(QuickJsOutput::Json(
                serde_json::json!([{ "title": "被篡改的历史脚本输出" }]),
            )),
            other => panic!("该场景的首个 capture 必须是网络或脚本 effect: {other:?}"),
        };
        capture.output = Arc::new(tampered);
    }

    fn corrupt_first_fingerprint(&self) {
        let mut captures = self.captures.lock().expect("archive capture mutex");
        captures
            .first_mut()
            .expect("live execution must have a capture")
            .fingerprint = "corrupted-fingerprint".to_string();
    }

    fn corrupt_first_quickjs_witness_hash(&self, field: QuickJsWitnessHashField) {
        let mut captures = self.captures.lock().expect("archive capture mutex");
        let capture = captures
            .first_mut()
            .expect("live execution must have a capture");
        let EffectWitness::QuickJs(witness) = &mut capture.witness else {
            panic!("fixture must contain a QuickJS witness");
        };
        match field {
            QuickJsWitnessHashField::Script => witness.script_hash = "0".repeat(64),
            QuickJsWitnessHashField::Input => witness.input_hash = "0".repeat(64),
            QuickJsWitnessHashField::Output => witness.output_hash = "0".repeat(64),
        }
        capture.witness_hash = capture
            .witness
            .canonical_hash()
            .expect("tampered witness must remain canonical");
    }
    fn corrupt_first_extract_witness_input_hash(&self) {
        let mut captures = self.captures.lock().expect("archive capture mutex");
        let capture = captures
            .iter_mut()
            .find(|capture| matches!(&capture.witness, EffectWitness::Extract(_)))
            .expect("live execution must have an Extract capture");
        let EffectWitness::Extract(witness) = &mut capture.witness else {
            panic!("fixture must contain an Extract witness");
        };
        witness.input_hash = "0".repeat(64);
        capture.witness_hash = capture
            .witness
            .canonical_hash()
            .expect("tampered witness must remain canonical");
    }

    async fn wait_until_persisted(&self) {
        self.persisted.notified().await;
    }
}

impl Drop for DurableFileArchive {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[async_trait]
impl EffectArchive for DurableFileArchive {
    async fn persist_durable(
        &self,
        capture: EffectCapture,
    ) -> Result<DurableCaptureReceipt, EffectArchiveError> {
        let output = serde_json::to_vec(capture.output.as_ref())
            .map_err(|_| EffectArchiveError::new("测试 archive 无法序列化输出"))?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|_| EffectArchiveError::new("测试 archive 无法创建 capture 文件"))?;
        let archive_error = || EffectArchiveError::new("测试 archive 无法同步 capture 文件");
        file.write_all(capture.fingerprint.as_bytes())
            .map_err(|_| archive_error())?;
        file.write_all(b"\t").map_err(|_| archive_error())?;
        file.write_all(capture.output_hash.as_bytes())
            .map_err(|_| archive_error())?;
        file.write_all(b"\t").map_err(|_| archive_error())?;
        file.write_all(&output).map_err(|_| archive_error())?;
        file.write_all(b"\n").map_err(|_| archive_error())?;
        file.sync_all().map_err(|_| archive_error())?;

        let receipt = DurableCaptureReceipt {
            effect_id: capture.effect_id,
            invocation_path: capture.invocation_path.clone(),
            fingerprint: capture.fingerprint.clone(),
            output_hash: capture.output_hash.clone(),
            witness_hash: capture.witness_hash.clone(),
        };
        self.captures
            .lock()
            .expect("archive capture mutex")
            .push(capture);
        self.persisted.notify_one();
        Ok(receipt)
    }

    async fn load_replay(
        &self,
        lookup: EffectReplayLookup,
    ) -> Result<Option<EffectCapture>, EffectArchiveError> {
        Ok(self
            .captures
            .lock()
            .expect("archive capture mutex")
            .iter()
            .find(|capture| {
                capture.execution_id == lookup.archived_execution_id
                    && capture.invocation_path == lookup.invocation_path
                    && capture.kind == lookup.kind
            })
            .cloned())
    }

    async fn persist_control_trace(
        &self,
        capture: ControlTraceCapture,
    ) -> Result<ControlTraceReceipt, EffectArchiveError> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|_| EffectArchiveError::new("测试 archive 无法创建 control trace 文件"))?;
        file.write_all(capture.trace_hash.as_bytes())
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.sync_all())
            .map_err(|_| EffectArchiveError::new("测试 archive 无法同步 control trace 文件"))?;
        let receipt = ControlTraceReceipt {
            invocation_path: capture.invocation_path.clone(),
            trace_hash: capture.trace_hash.clone(),
        };
        self.control_traces
            .lock()
            .expect("archive control trace mutex")
            .push(capture);
        Ok(receipt)
    }

    async fn load_control_trace(
        &self,
        lookup: ControlReplayLookup,
    ) -> Result<Option<ControlTraceCapture>, EffectArchiveError> {
        Ok(self
            .control_traces
            .lock()
            .expect("archive control trace mutex")
            .iter()
            .find(|capture| {
                capture.execution_id == lookup.archived_execution_id
                    && capture.invocation_path == lookup.invocation_path
            })
            .cloned())
    }

    async fn validate_replay_complete(
        &self,
        lookup: ReplayCompletionLookup,
    ) -> Result<(), EffectArchiveError> {
        let captures = self.captures.lock().expect("archive capture mutex");
        let traces = self
            .control_traces
            .lock()
            .expect("archive control trace mutex");
        let mut ordinals = captures
            .iter()
            .filter(|capture| capture.execution_id == lookup.archived_execution_id)
            .map(|capture| capture.invocation_path.ordinal())
            .chain(
                traces
                    .iter()
                    .filter(|capture| capture.execution_id == lookup.archived_execution_id)
                    .map(|capture| capture.invocation_path.ordinal()),
            )
            .collect::<Vec<_>>();
        ordinals.sort_unstable();
        let complete = u64::try_from(ordinals.len()).ok() == Some(lookup.observed_invocation_count)
            && ordinals
                .iter()
                .copied()
                .eq(1..=lookup.observed_invocation_count);
        if complete {
            Ok(())
        } else {
            Err(EffectArchiveError::new(
                "测试 archive invocation ledger 不完整",
            ))
        }
    }
}

enum HttpBehavior {
    Success,
    ObserveJson(Arc<Mutex<Vec<serde_json::Value>>>),
    Failure,
    WaitForCancellation(Arc<Notify>),
    WaitForRelease {
        started: Arc<Notify>,
        release: Arc<Notify>,
    },
}

struct FixtureHttp {
    behavior: HttpBehavior,
    calls: Arc<AtomicUsize>,
}

impl FixtureHttp {
    fn success(calls: Arc<AtomicUsize>) -> Self {
        Self {
            behavior: HttpBehavior::Success,
            calls,
        }
    }

    fn observe_json(calls: Arc<AtomicUsize>, observed: Arc<Mutex<Vec<serde_json::Value>>>) -> Self {
        Self {
            behavior: HttpBehavior::ObserveJson(observed),
            calls,
        }
    }

    fn failure(calls: Arc<AtomicUsize>) -> Self {
        Self {
            behavior: HttpBehavior::Failure,
            calls,
        }
    }

    fn wait_for_cancellation(calls: Arc<AtomicUsize>, started: Arc<Notify>) -> Self {
        Self {
            behavior: HttpBehavior::WaitForCancellation(started),
            calls,
        }
    }
    fn wait_for_release(
        calls: Arc<AtomicUsize>,
        started: Arc<Notify>,
        release: Arc<Notify>,
    ) -> Self {
        Self {
            behavior: HttpBehavior::WaitForRelease { started, release },
            calls,
        }
    }
}

#[async_trait]
impl HttpEffectHandler for FixtureHttp {
    async fn execute_http(
        &self,
        request: HttpEffectRequest,
        cancellation: EffectCancellation,
    ) -> Result<CapturedEffectOutput, EffectError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match &self.behavior {
            HttpBehavior::Success => Ok(fixture_http_success_capture()),
            HttpBehavior::ObserveJson(observed) => {
                let input = request.input.json().cloned().ok_or_else(|| {
                    EffectError::new(EffectErrorCode::InputType, "Loop body 需要 JSON binding")
                })?;
                observed
                    .lock()
                    .map_err(|_| {
                        EffectError::new(EffectErrorCode::Internal, "Loop binding fixture 锁损坏")
                    })?
                    .push(input);
                Ok(fixture_http_success_capture())
            }
            HttpBehavior::Failure => Ok(fixture_http_failure_capture()),
            HttpBehavior::WaitForCancellation(started) => {
                started.notify_one();
                cancellation.cancelled().await;
                Err(EffectError::new(
                    EffectErrorCode::Cancelled,
                    "HTTP effect 已取消",
                ))
            }
            HttpBehavior::WaitForRelease { started, release } => {
                started.notify_one();
                release.notified().await;
                Ok(fixture_http_success_capture())
            }
        }
    }
}

fn fixture_http_success_capture() -> CapturedEffectOutput {
    fixture_http_capture(HttpResponse {
        status: 200,
        headers: HashMap::new(),
        body: "{\"title\":\"真实 capture 输出\",\"url\":\"https://example.invalid/item\"}"
            .as_bytes()
            .to_vec(),
        charset: Some("utf-8".to_string()),
    })
}

fn fixture_http_capture(response: HttpResponse) -> CapturedEffectOutput {
    CapturedEffectOutput::new(
        EffectOutput::Http(response),
        EffectWitness::Http(fixture_http_witness(None)),
    )
}

fn fixture_http_failure_capture() -> CapturedEffectOutput {
    CapturedEffectOutput::new(
        EffectOutput::Failure(EffectFailure::Http {
            error: HttpEffectErrorKind::Request,
        }),
        EffectWitness::Http(fixture_http_witness(Some(HttpEffectErrorKind::Request))),
    )
}

fn fixture_http_witness(error: Option<HttpEffectErrorKind>) -> HttpEffectWitness {
    HttpEffectWitness {
        request: HttpRequestWitness {
            method: HttpMethod::Get,
            safe_url: "https://example.invalid/search".to_string(),
            headers: Vec::new(),
            body: None,
        },
        redirects: Vec::new(),
        dns_targets: Vec::new(),
        error,
        duration_ms: 0,
    }
}

struct FixtureQuickJs {
    calls: Arc<AtomicUsize>,
}

impl FixtureQuickJs {
    /// 带调用计数的 handler, 供「live 必须 capture 每个跑过的脚本 effect」类断言使用。
    fn counting(calls: Arc<AtomicUsize>) -> Self {
        Self { calls }
    }
}

#[async_trait]
impl QuickJsEffectHandler for FixtureQuickJs {
    async fn execute_quickjs(
        &self,
        request: QuickJsEffectRequest,
        _cancellation: EffectCancellation,
    ) -> Result<CapturedEffectOutput, EffectError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let output = EffectOutput::QuickJs(QuickJsOutput::Json(serde_json::json!([
            {"title": "真实 typed QuickJS 输出", "url": "https://example.invalid/item"}
        ])));
        let input_hash = effect_input_hash(&request.input).map_err(|_| {
            EffectError::new(EffectErrorCode::Internal, "QuickJS 输入 hash 计算失败")
        })?;
        let output_hash = effect_output_hash(&output).map_err(|_| {
            EffectError::new(EffectErrorCode::Internal, "QuickJS 输出 hash 计算失败")
        })?;
        Ok(CapturedEffectOutput::new(
            output,
            EffectWitness::QuickJs(QuickJsEffectWitness {
                script_hash: quickjs_script_hash(&request.code),
                input_hash,
                output_hash,
                error: None,
                host_calls: Vec::new(),
                duration_ms: 0,
            }),
        ))
    }
}

/// 按需返回固定可归档脚本失败类别的 handler。
///
/// 真实 `QuickJS` watchdog 的 timeout 合同由 `lj-node-js` 与 e2e 测试覆盖; 本 fixture 只用来
/// 固定 runtime 对「已归档类型化脚本失败」的处理: 失败必须 durable capture 后进入稳定
/// code 的 Failed 终态, 且 replay 复现同一终态。
struct FailingQuickJs {
    calls: Arc<AtomicUsize>,
    error: QuickJsErrorKind,
}

#[async_trait]
impl QuickJsEffectHandler for FailingQuickJs {
    async fn execute_quickjs(
        &self,
        request: QuickJsEffectRequest,
        _cancellation: EffectCancellation,
    ) -> Result<CapturedEffectOutput, EffectError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let output = EffectOutput::QuickJs(QuickJsOutput::Error(self.error));
        let input_hash = effect_input_hash(&request.input).map_err(|_| {
            EffectError::new(EffectErrorCode::Internal, "QuickJS 输入 hash 计算失败")
        })?;
        let output_hash = effect_output_hash(&output).map_err(|_| {
            EffectError::new(EffectErrorCode::Internal, "QuickJS 输出 hash 计算失败")
        })?;
        Ok(CapturedEffectOutput::new(
            output,
            EffectWitness::QuickJs(QuickJsEffectWitness {
                script_hash: quickjs_script_hash(&request.code),
                input_hash,
                output_hash,
                error: Some(self.error),
                host_calls: Vec::new(),
                duration_ms: 0,
            }),
        ))
    }
}

struct ControlQuickJs {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl QuickJsEffectHandler for ControlQuickJs {
    async fn execute_quickjs(
        &self,
        request: QuickJsEffectRequest,
        _cancellation: EffectCancellation,
    ) -> Result<CapturedEffectOutput, EffectError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let value = if request.code.contains("control_condition_alpha") {
            serde_json::Value::String("alpha".to_string())
        } else if request.code.contains("control_loop_collection") {
            request.input.json().cloned().ok_or_else(|| {
                EffectError::new(EffectErrorCode::InputType, "control Loop 需要 JSON 输入")
            })?
        } else if request.code.contains("control_alpha_hard_max_plus_one") {
            serde_json::Value::Array(
                (0..=MAX_LOOP_ITERATIONS)
                    .map(|index| serde_json::json!({ "index": index }))
                    .collect(),
            )
        } else if request.code.contains("control_alpha_hard_max") {
            serde_json::Value::Array(
                (0..MAX_LOOP_ITERATIONS)
                    .map(|index| serde_json::json!({ "index": index }))
                    .collect(),
            )
        } else if request.code.contains("control_alpha_empty") {
            serde_json::json!([])
        } else if request.code.contains("control_alpha_object") {
            serde_json::json!({"title": "非数组"})
        } else {
            serde_json::json!([
                {"enabled": true, "title": "第一项", "url": "https://example.invalid/1"},
                {"enabled": true, "title": "第二项", "url": "https://example.invalid/2"}
            ])
        };
        let output = EffectOutput::QuickJs(QuickJsOutput::Json(value));
        let input_hash = effect_input_hash(&request.input).map_err(|_| {
            EffectError::new(EffectErrorCode::Internal, "QuickJS 输入 hash 计算失败")
        })?;
        let output_hash = effect_output_hash(&output).map_err(|_| {
            EffectError::new(EffectErrorCode::Internal, "QuickJS 输出 hash 计算失败")
        })?;
        Ok(CapturedEffectOutput::new(
            output,
            EffectWitness::QuickJs(QuickJsEffectWitness {
                script_hash: quickjs_script_hash(&request.code),
                input_hash,
                output_hash,
                error: None,
                host_calls: Vec::new(),
                duration_ms: 0,
            }),
        ))
    }
}

struct FixtureExtract {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl ExtractEffectHandler for FixtureExtract {
    async fn execute_extract(
        &self,
        request: ExtractEffectRequest,
        cancellation: EffectCancellation,
    ) -> Result<CapturedEffectOutput, EffectError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if cancellation.is_cancelled() {
            return Err(EffectError::new(
                EffectErrorCode::Cancelled,
                "Extract effect 已取消",
            ));
        }
        let Some(EffectOutput::Http(_)) = request.input.output() else {
            return Err(EffectError::new(
                EffectErrorCode::InputType,
                "Extract effect 需要 HTTP 响应输入",
            ));
        };
        let input_hash = effect_input_hash(&request.input).map_err(|_| {
            EffectError::new(EffectErrorCode::Internal, "Extract 输入 hash 计算失败")
        })?;
        Ok(CapturedEffectOutput::new(
            EffectOutput::Extract(ExtractOutput {
                records: vec![serde_json::json!({
                    "title": "真实 typed Extract 输出",
                    "url": "https://example.invalid/item",
                })],
            }),
            EffectWitness::Extract(ExtractEffectWitness {
                input_hash,
                duration_ms: 0,
            }),
        ))
    }
}

/// 用内置 effect 集合装配一个只注册指定 kind 的 frozen registry。
fn registry_from(effects: Vec<(EffectKind, EffectHandler)>) -> Arc<FrozenEffectRegistry> {
    let mut registry = EffectRegistry::new();
    registry
        .register_all(effects)
        .expect("内置 capability 注册必须成功");
    Arc::new(registry.freeze())
}

/// 完整注册内置 HTTP、QuickJS rule node 与 Extract handler 的 registry。
fn registry_with(
    http: Arc<dyn HttpEffectHandler>,
    quickjs: Arc<dyn QuickJsEffectHandler>,
    extract: Arc<dyn ExtractEffectHandler>,
) -> Arc<FrozenEffectRegistry> {
    let mut registry = EffectRegistry::new();
    registry
        .register_all(builtin::effects(http, quickjs, extract))
        .expect("内置 capability 注册必须成功");
    Arc::new(registry.freeze())
}

fn handlers(http: FixtureHttp, extract_calls: Arc<AtomicUsize>) -> Arc<FrozenEffectRegistry> {
    registry_with(
        Arc::new(http),
        Arc::new(FixtureQuickJs::counting(Arc::new(AtomicUsize::new(0)))),
        Arc::new(FixtureExtract {
            calls: extract_calls,
        }),
    )
}

/// 三类受控 effect 各自的 live 调用次数。
///
/// replay 的不变量「禁止 live fallback」只能靠调用计数证明: 只看返回的错误无法区分
/// 「读了 archive」与「静默重跑了 live handler 再报错」。
#[derive(Clone)]
struct LiveCalls {
    http: Arc<AtomicUsize>,
    quickjs: Arc<AtomicUsize>,
    extract: Arc<AtomicUsize>,
}

impl LiveCalls {
    fn new() -> Self {
        Self {
            http: Arc::new(AtomicUsize::new(0)),
            quickjs: Arc::new(AtomicUsize::new(0)),
            extract: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn total(&self) -> usize {
        self.http.load(Ordering::SeqCst)
            + self.quickjs.load(Ordering::SeqCst)
            + self.extract.load(Ordering::SeqCst)
    }

    /// 完整注册 HTTP/QuickJS/Extract, 三类 handler 都按实际调用计数。
    fn registry(&self) -> Arc<FrozenEffectRegistry> {
        registry_with(
            Arc::new(FixtureHttp::success(self.http.clone())),
            Arc::new(FixtureQuickJs::counting(self.quickjs.clone())),
            Arc::new(FixtureExtract {
                calls: self.extract.clone(),
            }),
        )
    }
}

/// 只让 `QuickJS` 节点失败 (timeout) 的 registry; HTTP/Extract 不需要被调用。
fn timeout_handlers(
    http: FixtureHttp,
    quickjs_calls: Arc<AtomicUsize>,
) -> Arc<FrozenEffectRegistry> {
    registry_with(
        Arc::new(http),
        Arc::new(FailingQuickJs {
            calls: quickjs_calls,
            error: QuickJsErrorKind::Timeout,
        }),
        Arc::new(FixtureExtract {
            calls: Arc::new(AtomicUsize::new(0)),
        }),
    )
}

fn control_handlers(
    http: FixtureHttp,
    quickjs_calls: Arc<AtomicUsize>,
    extract_calls: Arc<AtomicUsize>,
) -> Arc<FrozenEffectRegistry> {
    registry_with(
        Arc::new(http),
        Arc::new(ControlQuickJs {
            calls: quickjs_calls,
        }),
        Arc::new(FixtureExtract {
            calls: extract_calls,
        }),
    )
}

/// 故意不注册 `QuickJS` effect 的 registry，用于验证 lookup miss 的稳定失败。
fn registry_without_quickjs(
    http: FixtureHttp,
    extract_calls: Arc<AtomicUsize>,
) -> Arc<FrozenEffectRegistry> {
    registry_from(vec![
        (EffectKind::Http, EffectHandler::Http(Arc::new(http))),
        (
            EffectKind::Extract,
            EffectHandler::Extract(Arc::new(FixtureExtract {
                calls: extract_calls,
            })),
        ),
    ])
}

fn runtime(event_channel_capacity: usize) -> PlanRuntime {
    PlanRuntime::new(PlanRuntimeConfig {
        compiler_version: "runtime-test-compiler@1".to_string(),
        event_channel_capacity,
        max_concurrent_executions: 2,
        max_concurrent_effects: 2,
        max_concurrent_effects_per_source: 1,
    })
    .expect("runtime config")
}

fn sample_plan() -> ExecutionPlan {
    Compiler::with_version("runtime-test-compiler@1".to_string())
        .compile(&compiler_definition())
        .expect("linear Definition must compile")
}

fn quickjs_plan() -> ExecutionPlan {
    Compiler::with_version("runtime-test-compiler@1".to_string())
        .compile(&quickjs_definition())
        .expect("QuickJS Definition must compile")
}

fn control_plan() -> ExecutionPlan {
    ExecutionPlan::new(
        "runtime-test-compiler@1",
        "control-definition-hash",
        ExecutionPlanParts {
            nodes: vec![PlanNode {
                id: Uuid::from_u128(900),
                inputs: Vec::new(),
                outputs: Vec::new(),
                config: PlanNodeConfig::Merge(MergeConfig {
                    inputs: vec![MergeInput {
                        input_id: "primary".to_string(),
                        handle: "primary".to_string(),
                        order: 0,
                        activation: MergeInputActivation::Required,
                    }],
                    strategy: MergeStrategy::SingleActive,
                }),
            }],
            edges: Vec::new(),
            intent_entries: BTreeMap::new(),
            effects: Vec::new(),
            capability_requirements: Vec::new(),
            control_regions: Vec::new(),
        },
    )
    .expect("control Plan must seal")
}

fn plan_json(plan: &ExecutionPlan) -> serde_json::Value {
    serde_json::to_value(plan).expect("serialize current Plan fixture")
}

fn rewrite_plan(
    plan: &ExecutionPlan,
    mutate: impl FnOnce(&mut serde_json::Value),
    reseal: bool,
) -> ExecutionPlan {
    let mut value = plan_json(plan);
    mutate(&mut value);
    if reseal {
        value["plan_hash"] = serde_json::Value::String(String::new());
        let bytes = serde_json::to_vec(&value).expect("serialize unsealed Plan fixture");
        let unsealed = read_execution_plan(&bytes).expect("read unsealed current Plan fixture");
        value["plan_hash"] = serde_json::Value::String(
            execution_plan_hash(&unsealed).expect("hash rewritten current Plan fixture"),
        );
    }
    read_execution_plan(&serde_json::to_vec(&value).expect("serialize rewritten Plan fixture"))
        .expect("read rewritten current Plan fixture")
}

fn request(
    plan: lj_rule_model::ExecutionPlan,
    execution_id: Uuid,
    mode: lj_runtime::ExecutionMode,
) -> PlanExecutionRequest {
    PlanExecutionRequest {
        execution_id,
        source_id: "runtime-test-source".to_string(),
        trace_id: "runtime-test-trace".to_string(),
        plan,
        intent: StandardIntent::Search,
        input: IntentInput::Query("capture".to_string()),
        mode,
        capabilities: SystemCapabilities::default(),
        base_url: "https://example.invalid".to_string(),
        credentials: HttpExecutionCredentials::default(),
    }
}

fn request_with_credentials(
    plan: lj_rule_model::ExecutionPlan,
    execution_id: Uuid,
    mode: lj_runtime::ExecutionMode,
    credentials: HttpExecutionCredentials,
) -> PlanExecutionRequest {
    let mut request = request(plan, execution_id, mode);
    request.credentials = credentials;
    request
}

async fn collect_events(session: lj_runtime::ExecutionSession) -> Vec<lj_runtime::ExecutionEvent> {
    session.into_events().collect().await
}

fn terminal_count(events: &[lj_runtime::ExecutionEvent]) -> usize {
    events
        .iter()
        .filter(|event| {
            matches!(
                event.kind,
                lj_runtime::ExecutionEventKind::Completed
                    | lj_runtime::ExecutionEventKind::Failed { .. }
                    | lj_runtime::ExecutionEventKind::Cancelled
            )
        })
        .count()
}

fn compiler_definition() -> RuleDefinition {
    let http = Uuid::from_u128(101);
    let extract = Uuid::from_u128(102);
    let mapper = Uuid::from_u128(103);
    let nodes = vec![
        FlowNode::new(
            http,
            FlowNodeConfig::Http(HttpSpec {
                method: HttpMethod::Get,
                url: "https://example.invalid/search?q={{key}}".to_string(),
                headers: HashMap::new(),
                body: None,
                charset: None,
                expected_type: ExpectedDataType::Html,
            }),
        ),
        FlowNode::new(
            extract,
            FlowNodeConfig::Extract(ExtractSpec {
                rules: Vec::new(),
                field_rules: HashMap::new(),
                expected_type: ExpectedDataType::Html,
                output_target: lj_rule_model::OutputTarget::default(),
            }),
        ),
        FlowNode::new(
            mapper,
            FlowNodeConfig::Mapper(ControlledMapper {
                output: MapperOutputKind::Items,
                identity_fields: vec!["url".to_string()],
            }),
        ),
    ];
    let intent_exports =
        BTreeMap::from([(StandardIntent::Search, IntentExport::new(http, mapper))]);
    RuleDefinition::new(
        SourceIdentity {
            id: "compiler-runtime-test".to_string(),
        },
        "https://example.invalid",
        intent_exports,
        FlowGraph {
            nodes,
            edges: vec![linear_edge(http, extract), linear_edge(extract, mapper)],
        },
        CapabilityManifest {
            required: SystemCapabilities::default(),
        },
        vec!["url".to_string()],
    )
}

fn quickjs_definition() -> RuleDefinition {
    let quickjs = Uuid::from_u128(1);
    let mapper = Uuid::from_u128(3);
    RuleDefinition::new(
        SourceIdentity {
            id: "quickjs-runtime-test".to_string(),
        },
        "https://example.invalid",
        BTreeMap::from([(
            StandardIntent::Search,
            IntentExport::new(quickjs, mapper),
        )]),
        FlowGraph {
            nodes: vec![
                FlowNode::new(
                    quickjs,
                    FlowNodeConfig::Js(JsConfig::new("JSON.stringify([{ title: 'fixture', url: 'https://example.invalid/item' }])".to_string(), JsOutputKind::Json)),
                ),
                FlowNode::new(
                    mapper,
                    FlowNodeConfig::Mapper(ControlledMapper {
                        output: MapperOutputKind::Items,
                        identity_fields: vec!["url".to_string()],
                    }),
                ),
            ],
            edges: vec![linear_edge(quickjs, mapper)],
        },
        CapabilityManifest {
            required: SystemCapabilities::default(),
        },
        vec!["url".to_string()],
    )
}

fn compiled_control_plan(
    expression: ControlExpression,
    collection: CollectionSelector,
    alpha_code: &str,
    strategy: MergeStrategy,
    max_iterations: u16,
) -> ExecutionPlan {
    Compiler::with_version("runtime-test-compiler@1".to_string())
        .compile(&control_definition(
            expression,
            collection,
            alpha_code,
            strategy,
            max_iterations,
        ))
        .expect("structured control Definition must compile")
}

fn typed_control_expression() -> ControlExpression {
    ControlExpression::Typed {
        predicate: ConditionPredicate::Eq {
            pointer: "/0/enabled".to_string(),
            value: TypedLiteral::Bool(true),
        },
        true_branch: "alpha".to_string(),
        false_branch: "beta".to_string(),
    }
}

fn control_definition(
    expression: ControlExpression,
    collection: CollectionSelector,
    alpha_code: &str,
    strategy: MergeStrategy,
    max_iterations: u16,
) -> RuleDefinition {
    let entry = Uuid::from_u128(1_001);
    let condition = Uuid::from_u128(1_002);
    let alpha = Uuid::from_u128(1_003);
    let beta = Uuid::from_u128(1_004);
    let merge = Uuid::from_u128(1_005);
    let loop_node = Uuid::from_u128(1_006);
    let http = Uuid::from_u128(1_007);
    let extract = Uuid::from_u128(1_008);
    let mapper = Uuid::from_u128(1_009);
    let nodes = vec![
        FlowNode::new(
            entry,
            FlowNodeConfig::Js(JsConfig::new(
                "control_entry".to_string(),
                JsOutputKind::Json,
            )),
        ),
        FlowNode::new(
            condition,
            FlowNodeConfig::Condition(ConditionConfig {
                branches: vec!["alpha".to_string(), "beta".to_string()],
                expression,
            }),
        ),
        FlowNode::new(
            alpha,
            FlowNodeConfig::Js(JsConfig::new(alpha_code.to_string(), JsOutputKind::Json)),
        ),
        FlowNode::new(
            beta,
            FlowNodeConfig::Js(JsConfig::new(
                "control_beta".to_string(),
                JsOutputKind::Json,
            )),
        ),
        FlowNode::new(
            merge,
            FlowNodeConfig::Merge(MergeConfig {
                inputs: vec![
                    MergeInput {
                        input_id: "alpha".to_string(),
                        handle: "alpha".to_string(),
                        order: 0,
                        activation: MergeInputActivation::Required,
                    },
                    MergeInput {
                        input_id: "beta".to_string(),
                        handle: "beta".to_string(),
                        order: 1,
                        activation: MergeInputActivation::Required,
                    },
                ],
                strategy,
            }),
        ),
        FlowNode::new(
            loop_node,
            FlowNodeConfig::Loop(ForEachConfig {
                collection,
                item_binding: "item".to_string(),
                index_binding: "index".to_string(),
                max_iterations,
            }),
        ),
        FlowNode::new(
            http,
            FlowNodeConfig::Http(HttpSpec {
                method: HttpMethod::Get,
                url: "https://example.invalid/body".to_string(),
                headers: HashMap::new(),
                body: None,
                charset: None,
                expected_type: ExpectedDataType::Json,
            }),
        ),
        FlowNode::new(
            extract,
            FlowNodeConfig::Extract(ExtractSpec {
                rules: Vec::new(),
                field_rules: HashMap::new(),
                expected_type: ExpectedDataType::Json,
                output_target: lj_rule_model::OutputTarget::default(),
            }),
        ),
        FlowNode::new(
            mapper,
            FlowNodeConfig::Mapper(ControlledMapper {
                output: MapperOutputKind::Items,
                identity_fields: vec!["url".to_string()],
            }),
        ),
    ];
    let edges = vec![
        FlowEdge::new(
            FlowPortRef::new(entry, LINEAR_OUTPUT_HANDLE),
            FlowPortRef::new(condition, lj_rule_model::CONDITION_INPUT_HANDLE),
        ),
        FlowEdge::new(
            FlowPortRef::new(condition, "alpha"),
            FlowPortRef::new(alpha, LINEAR_INPUT_HANDLE),
        ),
        FlowEdge::new(
            FlowPortRef::new(condition, "beta"),
            FlowPortRef::new(beta, LINEAR_INPUT_HANDLE),
        ),
        FlowEdge::new(
            FlowPortRef::new(alpha, LINEAR_OUTPUT_HANDLE),
            FlowPortRef::new(merge, "alpha"),
        ),
        FlowEdge::new(
            FlowPortRef::new(beta, LINEAR_OUTPUT_HANDLE),
            FlowPortRef::new(merge, "beta"),
        ),
        FlowEdge::new(
            FlowPortRef::new(merge, lj_rule_model::MERGE_OUTPUT_HANDLE),
            FlowPortRef::new(loop_node, LOOP_COLLECTION_HANDLE),
        ),
        FlowEdge::new(
            FlowPortRef::new(loop_node, LOOP_BODY_HANDLE),
            FlowPortRef::new(http, LINEAR_INPUT_HANDLE),
        ),
        linear_edge(http, extract),
        FlowEdge::new(
            FlowPortRef::new(extract, LINEAR_OUTPUT_HANDLE),
            FlowPortRef::new(loop_node, LOOP_YIELD_HANDLE),
        ),
        FlowEdge::new(
            FlowPortRef::new(loop_node, LOOP_DONE_HANDLE),
            FlowPortRef::new(mapper, LINEAR_INPUT_HANDLE),
        ),
    ];
    RuleDefinition::new(
        SourceIdentity {
            id: "control-runtime-test".to_string(),
        },
        "https://example.invalid",
        BTreeMap::from([(StandardIntent::Search, IntentExport::new(entry, mapper))]),
        FlowGraph { nodes, edges },
        CapabilityManifest {
            required: SystemCapabilities::default(),
        },
        vec!["url".to_string()],
    )
}

fn linear_edge(from: Uuid, to: Uuid) -> FlowEdge {
    FlowEdge::new(
        FlowPortRef::new(from, LINEAR_OUTPUT_HANDLE),
        FlowPortRef::new(to, LINEAR_INPUT_HANDLE),
    )
}

#[path = "plan_runtime_test/control_contract.rs"]
mod control_contract;

#[path = "plan_runtime_test/replay_contract.rs"]
mod replay_contract;

#[path = "plan_runtime_test/scheduling_contract.rs"]
mod scheduling_contract;
