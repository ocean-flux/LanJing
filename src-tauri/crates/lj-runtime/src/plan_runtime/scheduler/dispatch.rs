//! bounded permit、typed handler dispatch 与 capability gate。

use super::{
    Arc, CapturedEffectOutput, EffectCancellation, EffectDeclaration, EffectError, EffectErrorCode,
    EffectFailure, EffectInput, EffectOutput, ExtractEffectRequest, HttpEffectRequest, JsBudget,
    MediaResourceId, OwnedSemaphorePermit, PlanExecutionRequest, PlanNode, PlanNodeConfig,
    QuickJsEffectRequest, QuickJsOutput, RunOutcome, RuntimeFailureCode, SystemCapabilities, Uuid,
    effective_js_budget, failed,
};

use crate::capability::check_capability;
use crate::effect_registry::EffectHandler;
use lj_rule_model::Capability;

pub(in crate::plan_runtime::scheduler) async fn acquire_permit(
    semaphore: Arc<tokio::sync::Semaphore>,
    cancellation: EffectCancellation,
    request: &PlanExecutionRequest,
    node_id: Option<Uuid>,
    effect_id: Option<Uuid>,
) -> Result<OwnedSemaphorePermit, RunOutcome> {
    tokio::select! {
        () = cancellation.cancelled() => Err(RunOutcome::Cancelled),
        permit = semaphore.acquire_owned() => permit.map_err(|_| failed(
            request,
            RuntimeFailureCode::Internal,
            "并发控制器已关闭",
            node_id,
            effect_id,
        )),
    }
}

pub(in crate::plan_runtime::scheduler) async fn invoke_live_effect(
    request: &PlanExecutionRequest,
    node: &PlanNode,
    effect_id: Uuid,
    input: EffectInput,
    js_code_override: Option<&str>,
    handler: &EffectHandler,
    cancellation: EffectCancellation,
) -> Result<CapturedEffectOutput, EffectError> {
    match handler {
        EffectHandler::Http(http) => {
            let PlanNodeConfig::Http(spec) = &node.config else {
                return Err(handler_config_mismatch());
            };
            http.execute_http(
                HttpEffectRequest {
                    execution_id: request.execution_id,
                    source_id: request.source_id.clone(),
                    node_id: node.id,
                    effect_id,
                    trace_id: request.trace_id.clone(),
                    spec: spec.clone(),
                    input,
                    base_url: request.base_url.clone(),
                    credentials: request.credentials.clone(),
                },
                cancellation,
            )
            .await
        }
        EffectHandler::QuickJs(quickjs) => {
            let ExecutedJs { code, budgets } = executed_js(node, js_code_override)?;
            quickjs
                .execute_quickjs(
                    QuickJsEffectRequest {
                        execution_id: request.execution_id,
                        source_id: request.source_id.clone(),
                        node_id: node.id,
                        effect_id,
                        trace_id: request.trace_id.clone(),
                        code,
                        budgets,
                        input,
                    },
                    cancellation,
                )
                .await
        }
        EffectHandler::Extract(extract) => {
            let PlanNodeConfig::Extract(spec) = &node.config else {
                return Err(handler_config_mismatch());
            };
            extract
                .execute_extract(
                    ExtractEffectRequest {
                        execution_id: request.execution_id,
                        source_id: request.source_id.clone(),
                        node_id: node.id,
                        effect_id,
                        trace_id: request.trace_id.clone(),
                        spec: spec.clone(),
                        input,
                        base_url: request.base_url.clone(),
                    },
                    cancellation,
                )
                .await
        }
    }
}

/// frozen registry 中的 handler 与 Plan 节点配置不一致时的内部错误。
fn handler_config_mismatch() -> EffectError {
    EffectError::new(
        EffectErrorCode::Internal,
        "registry handler 与 Plan 节点配置不匹配",
    )
}

/// 实际执行的 JS：脚本源码与生效资源预算。
///
/// 生效预算在这里落定，因为这是「Plan 节点声明」到「effect 请求」的唯一必经之处：
/// control JS（condition/loop 表达式覆盖）没有自己的预算声明，用 host policy 默认值；
/// 显式 JS 节点的声明值被 host policy 夹住，规则无法抬高自己的上限。
pub(in crate::plan_runtime::scheduler) struct ExecutedJs {
    pub(in crate::plan_runtime::scheduler) code: String,
    pub(in crate::plan_runtime::scheduler) budgets: JsBudget,
}

pub(in crate::plan_runtime::scheduler) fn executed_js(
    node: &PlanNode,
    js_code_override: Option<&str>,
) -> Result<ExecutedJs, EffectError> {
    if let Some(code) = js_code_override {
        return (!code.trim().is_empty())
            .then(|| ExecutedJs {
                code: code.to_string(),
                budgets: effective_js_budget(JsBudget::default()),
            })
            .ok_or_else(|| EffectError::new(EffectErrorCode::Internal, "control JS 配置无法读取"));
    }
    let PlanNodeConfig::Js(config) = &node.config else {
        return Err(EffectError::new(
            EffectErrorCode::Internal,
            "Plan JS 配置无法读取",
        ));
    };
    (!config.code.trim().is_empty())
        .then(|| ExecutedJs {
            code: config.code.clone(),
            budgets: effective_js_budget(config.budgets),
        })
        .ok_or_else(|| EffectError::new(EffectErrorCode::Internal, "Plan JS 配置无法读取"))
}

pub(in crate::plan_runtime::scheduler) fn captured_output_failure(
    output: &EffectOutput,
) -> Option<&'static str> {
    match output {
        EffectOutput::Failure(EffectFailure::Http { .. }) => Some("HTTP effect 执行失败"),
        EffectOutput::Failure(EffectFailure::QuickJs { .. })
        | EffectOutput::QuickJs(QuickJsOutput::Error(_)) => Some("QuickJS effect 执行失败"),
        EffectOutput::Failure(EffectFailure::Extract) => Some("Extract effect 执行失败"),
        EffectOutput::Http(_) | EffectOutput::QuickJs(_) | EffectOutput::Extract(_) => None,
    }
}

pub(in crate::plan_runtime::scheduler) fn source_media_id(source_id: &str) -> MediaResourceId {
    if source_id.starts_with("source:") {
        MediaResourceId(source_id.to_string())
    } else {
        MediaResourceId(format!("source:{source_id}"))
    }
}

pub(in crate::plan_runtime::scheduler) fn enforce_capabilities(
    declaration: &EffectDeclaration,
    capabilities: &SystemCapabilities,
) -> Result<(), &'static str> {
    for capability in &declaration.required_capabilities {
        let capability = match capability.as_str() {
            "fs" => Capability::Fs,
            "env" => Capability::Env,
            "process" => Capability::Process,
            _ => return Err("Plan 声明了 runtime 不支持的 capability"),
        };
        check_capability(capabilities, capability).map_err(|_| "安装 grant 未允许该 capability")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use lj_rule_model::{JsBudget, JsConfig, JsOutputKind, PlanNode, PlanNodeConfig};
    use uuid::Uuid;

    use super::executed_js;

    fn js_node(budgets: JsBudget) -> PlanNode {
        PlanNode {
            id: Uuid::new_v4(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            config: PlanNodeConfig::Js(JsConfig {
                code: "1".to_string(),
                output: JsOutputKind::Json,
                budgets,
            }),
        }
    }

    #[test]
    fn plan_declaration_cannot_raise_the_js_budget_ceiling() {
        let node = js_node(JsBudget {
            timeout_ms: 600_000,
            memory_bytes: 1024 * 1024 * 1024,
            output_bytes: 512 * 1024 * 1024,
        });
        let executed = executed_js(&node, None).expect("JS 节点必须能取出执行脚本");
        assert_eq!(executed.budgets, JsBudget::HOST_CEILING);
    }

    #[test]
    fn control_js_uses_the_host_policy_default_budget() {
        let node = js_node(JsBudget {
            timeout_ms: 10,
            memory_bytes: 1024,
            output_bytes: 2048,
        });
        let executed = executed_js(&node, Some("1 + 1")).expect("control JS 必须能取出执行脚本");
        assert_eq!(executed.budgets, JsBudget::HOST_CEILING);
        assert_eq!(executed.code, "1 + 1");
    }

    #[test]
    fn explicit_js_declaration_is_clamped_not_replaced() {
        let node = js_node(JsBudget {
            timeout_ms: 250,
            memory_bytes: 2 * 1024 * 1024,
            output_bytes: 8192,
        });
        let executed = executed_js(&node, None).expect("JS 节点必须能取出执行脚本");
        assert_eq!(
            executed.budgets,
            JsBudget {
                timeout_ms: 250,
                memory_bytes: 2 * 1024 * 1024,
                output_bytes: 8192,
            }
        );
    }

    #[test]
    fn sub_floor_memory_declaration_is_raised_to_the_engine_minimum() {
        let node = js_node(JsBudget {
            memory_bytes: 1024,
            ..JsBudget::HOST_CEILING
        });
        let executed = executed_js(&node, None).expect("JS 节点必须能取出执行脚本");
        assert_eq!(executed.budgets.memory_bytes, JsBudget::MIN_MEMORY_BYTES);
    }
}
