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
use blake3::Hasher;
use futures::StreamExt;
use lj_capability::{IntentExport, IntentInput, StandardIntent};
use lj_compiler::Compiler;
use lj_rule_model::definition::MapperOutputKind;
use lj_rule_model::{
    CapabilityManifest, ControlledMapper, EffectDeclaration, EffectKind, ExpectedDataType,
    ExtractSpec, FlowEdge, FlowGraph, FlowNode, FlowNodeKind, HttpMethod, HttpSpec, IntentEntry,
    PlanNode, PlanNodeKind, PlanPort, PolicyCapabilities, RuleDefinition, SourceIdentity,
    SystemCapabilities, canonical_json,
};
use lj_runtime::{
    CapturedEffectOutput, DurableCaptureReceipt, EffectArchive, EffectArchiveError,
    EffectCancellation, EffectCapture, EffectError, EffectErrorCode, EffectFailure, EffectHandlers,
    EffectOutput, EffectReplayLookup, EffectWitness, ExtractEffectHandler, ExtractEffectRequest,
    ExtractEffectWitness, ExtractOutput, HttpEffectErrorKind, HttpEffectHandler, HttpEffectRequest,
    HttpEffectWitness, HttpExecutionCredentials, HttpRequestWitness, HttpResponse,
    PlanExecutionRequest, PlanRuntime, PlanRuntimeConfig, QuickJsEffectHandler,
    QuickJsEffectRequest, QuickJsEffectWitness, QuickJsOutput, RuntimeFailureCode,
    effect_input_hash, effect_output_hash, quickjs_script_hash,
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
    path: PathBuf,
    persisted: Arc<Notify>,
}

impl DurableFileArchive {
    fn new() -> Self {
        Self {
            captures: Mutex::new(Vec::new()),
            path: std::env::temp_dir().join(format!("lj-runtime-capture-{}.jsonl", Uuid::new_v4())),
            persisted: Arc::new(Notify::new()),
        }
    }

    fn captures(&self) -> Vec<EffectCapture> {
        self.captures.lock().expect("archive capture mutex").clone()
    }

    fn corrupt_first_output_hash(&self) {
        let mut captures = self.captures.lock().expect("archive capture mutex");
        captures
            .first_mut()
            .expect("live execution must have a capture")
            .output_hash = "corrupted-output-hash".to_string();
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
                    && capture.node_id == lookup.node_id
                    && capture.kind == lookup.kind
            })
            .cloned())
    }
}

enum HttpBehavior {
    Success,
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
        _request: HttpEffectRequest,
        cancellation: EffectCancellation,
    ) -> Result<CapturedEffectOutput, EffectError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match &self.behavior {
            HttpBehavior::Success => Ok(fixture_http_capture(HttpResponse {
                status: 200,
                headers: HashMap::new(),
                body: "{\"title\":\"真实 capture 输出\",\"url\":\"https://example.invalid/item\"}"
                    .as_bytes()
                    .to_vec(),
                charset: Some("utf-8".to_string()),
            })),
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
                Ok(fixture_http_capture(HttpResponse {
                    status: 200,
                    headers: HashMap::new(),
                    body:
                        "{\"title\":\"真实 capture 输出\",\"url\":\"https://example.invalid/item\"}"
                            .as_bytes()
                            .to_vec(),
                    charset: Some("utf-8".to_string()),
                }))
            }
        }
    }
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

struct FixtureQuickJs;

#[async_trait]
impl QuickJsEffectHandler for FixtureQuickJs {
    async fn execute_quickjs(
        &self,
        request: QuickJsEffectRequest,
        _cancellation: EffectCancellation,
    ) -> Result<CapturedEffectOutput, EffectError> {
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

fn handlers(http: FixtureHttp, extract_calls: Arc<AtomicUsize>) -> EffectHandlers {
    EffectHandlers::new(
        Arc::new(http),
        Arc::new(FixtureQuickJs),
        Arc::new(FixtureExtract {
            calls: extract_calls,
        }),
    )
}

fn runtime(event_channel_capacity: usize) -> PlanRuntime {
    PlanRuntime::new(PlanRuntimeConfig {
        compiler_version: "runtime-test-compiler@1".to_string(),
        plan_schema_version: 1,
        event_channel_capacity,
        max_concurrent_executions: 2,
        max_concurrent_effects: 2,
        max_concurrent_effects_per_source: 1,
    })
    .expect("runtime config")
}

fn sample_plan() -> lj_rule_model::ExecutionPlan {
    let http = Uuid::from_u128(1);
    let extract = Uuid::from_u128(2);
    let mapper = Uuid::from_u128(3);
    let mut intent_entries = BTreeMap::new();
    intent_entries.insert(
        StandardIntent::Search,
        IntentEntry {
            intent: StandardIntent::Search,
            entry_node: http,
            mapper_output: mapper,
        },
    );
    let mut plan = lj_rule_model::ExecutionPlan {
        schema_version: 1,
        compiler_version: "runtime-test-compiler@1".to_string(),
        definition_hash: "definition-hash".to_string(),
        plan_hash: String::new(),
        nodes: vec![
            PlanNode {
                id: http,
                kind: PlanNodeKind::Http,
                inputs: vec![port("value")],
                outputs: vec![port("http_response")],
                config: serde_json::to_value(HttpSpec {
                    method: HttpMethod::Get,
                    url: "https://example.invalid/search?q={{key}}".to_string(),
                    headers: HashMap::new(),
                    body: None,
                    charset: None,
                    expected_type: ExpectedDataType::Html,
                })
                .expect("HTTP config"),
            },
            PlanNode {
                id: extract,
                kind: PlanNodeKind::Extract,
                inputs: vec![port("http_response")],
                outputs: vec![port("json")],
                config: serde_json::to_value(ExtractSpec {
                    rules: Vec::new(),
                    field_rules: HashMap::new(),
                    expected_type: ExpectedDataType::Html,
                    output_target: lj_rule_model::OutputTarget::default(),
                })
                .expect("Extract config"),
            },
            PlanNode {
                id: mapper,
                kind: PlanNodeKind::Mapper,
                inputs: vec![port("json")],
                outputs: vec![port("delta")],
                config: serde_json::to_value(ControlledMapper {
                    output: MapperOutputKind::Items,
                    identity_fields: vec!["url".to_string()],
                })
                .expect("Mapper config"),
            },
        ],
        edges: vec![(http, extract), (extract, mapper)],
        intent_entries,
        effects: vec![
            EffectDeclaration {
                node_id: http,
                kind: EffectKind::Http,
                required_capabilities: vec!["network".to_string()],
            },
            EffectDeclaration {
                node_id: extract,
                kind: EffectKind::Extract,
                required_capabilities: Vec::new(),
            },
        ],
        capability_requirements: vec!["network".to_string()],
    };
    plan.plan_hash = hash(&plan);
    plan
}

fn quickjs_plan() -> lj_rule_model::ExecutionPlan {
    let quickjs = Uuid::from_u128(1);
    let extract = Uuid::from_u128(2);
    let mapper = Uuid::from_u128(3);
    let mut plan = sample_plan();
    plan.nodes.retain(|node| node.id != extract);
    let quickjs_node = plan
        .nodes
        .iter_mut()
        .find(|node| node.id == quickjs)
        .expect("sample Plan must include the first effect node");
    quickjs_node.kind = PlanNodeKind::Js;
    quickjs_node.outputs = vec![port("json")];
    quickjs_node.config = serde_json::json!({
        "code": "JSON.stringify([{ title: 'fixture', url: 'https://example.invalid/item' }])"
    });
    plan.edges = vec![(quickjs, mapper)];
    plan.effects = vec![EffectDeclaration {
        node_id: quickjs,
        kind: EffectKind::QuickJs,
        required_capabilities: vec!["network".to_string()],
    }];
    plan.plan_hash.clear();
    plan.plan_hash = hash(&plan);
    plan
}

fn port(type_tag: &str) -> PlanPort {
    PlanPort {
        name: "value".to_string(),
        type_tag: type_tag.to_string(),
    }
}

fn hash<T>(value: &T) -> String
where
    T: serde::Serialize,
{
    let canonical = canonical_json(value).expect("canonical JSON");
    let mut hasher = Hasher::new();
    hasher.update(canonical.as_bytes());
    hasher.finalize().to_hex().to_string()
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
        capabilities: PolicyCapabilities {
            network: true,
            system: SystemCapabilities::default(),
        },
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
    let mut http_node = flow_node(http, FlowNodeKind::Http);
    http_node.http = Some(HttpSpec {
        method: HttpMethod::Get,
        url: "https://example.invalid/search?q={{key}}".to_string(),
        headers: HashMap::new(),
        body: None,
        charset: None,
        expected_type: ExpectedDataType::Html,
    });
    let mut extract_node = flow_node(extract, FlowNodeKind::Extract);
    extract_node.extract = Some(ExtractSpec {
        rules: Vec::new(),
        field_rules: HashMap::new(),
        expected_type: ExpectedDataType::Html,
        output_target: lj_rule_model::OutputTarget::default(),
    });
    let mut mapper_node = flow_node(mapper, FlowNodeKind::Mapper);
    mapper_node.mapper = Some(ControlledMapper {
        output: MapperOutputKind::Items,
        identity_fields: vec!["url".to_string()],
    });
    let mut intent_exports = BTreeMap::new();
    intent_exports.insert(StandardIntent::Search, IntentExport::new(http, mapper));
    RuleDefinition {
        schema_version: 1,
        source_identity: SourceIdentity {
            id: "compiler-runtime-test".to_string(),
        },
        base_url: "https://example.invalid".to_string(),
        intent_exports,
        flow: FlowGraph {
            nodes: vec![http_node, extract_node, mapper_node],
            edges: vec![
                FlowEdge {
                    from: http,
                    to: extract,
                    condition_branch: None,
                },
                FlowEdge {
                    from: extract,
                    to: mapper,
                    condition_branch: None,
                },
            ],
        },
        capability_manifest: CapabilityManifest {
            required: PolicyCapabilities {
                network: true,
                system: SystemCapabilities::default(),
            },
        },
        source_id_rules: vec!["url".to_string()],
    }
}

fn flow_node(id: Uuid, kind: FlowNodeKind) -> FlowNode {
    FlowNode {
        id,
        kind,
        http: None,
        js_code: None,
        extract: None,
        mapper: None,
        span: None,
    }
}

#[path = "plan_runtime_test/replay_contract.rs"]
mod replay_contract;

#[path = "plan_runtime_test/scheduling_contract.rs"]
mod scheduling_contract;
