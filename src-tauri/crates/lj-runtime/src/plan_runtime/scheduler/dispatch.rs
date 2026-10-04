//! bounded permit、typed handler dispatch 与 capability gate。

use super::{
    Arc, CapturedEffectOutput, EffectCancellation, EffectDeclaration, EffectError, EffectErrorCode,
    EffectFailure, EffectInput, EffectKind, EffectOutput, ExtractEffectRequest, HttpEffectRequest,
    MediaResourceId, OwnedSemaphorePermit, PlanExecutionRequest, PlanNode, PlanNodeConfig,
    PolicyCapabilities, QuickJsEffectRequest, QuickJsOutput, RunOutcome, RuntimeFailureCode, Uuid,
    failed,
};

use crate::effect_registry::EffectHandler;

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
                    capabilities: request.capabilities.clone(),
                    base_url: request.base_url.clone(),
                    credentials: request.credentials.clone(),
                },
                cancellation,
            )
            .await
        }
        EffectHandler::QuickJs(quickjs) => {
            let code = executed_js_code(node, js_code_override)?;
            quickjs
                .execute_quickjs(
                    QuickJsEffectRequest {
                        execution_id: request.execution_id,
                        source_id: request.source_id.clone(),
                        node_id: node.id,
                        effect_id,
                        trace_id: request.trace_id.clone(),
                        code,
                        input,
                        capabilities: request.capabilities.clone(),
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

pub(in crate::plan_runtime::scheduler) fn executed_js_code(
    node: &PlanNode,
    js_code_override: Option<&str>,
) -> Result<String, EffectError> {
    if let Some(code) = js_code_override {
        return (!code.trim().is_empty())
            .then(|| code.to_string())
            .ok_or_else(|| EffectError::new(EffectErrorCode::Internal, "control JS 配置无法读取"));
    }
    let PlanNodeConfig::Js(config) = &node.config else {
        return Err(EffectError::new(
            EffectErrorCode::Internal,
            "Plan JS 配置无法读取",
        ));
    };
    (!config.code.trim().is_empty())
        .then(|| config.code.clone())
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
    capabilities: &PolicyCapabilities,
) -> Result<(), &'static str> {
    for capability in &declaration.required_capabilities {
        match capability.as_str() {
            "network" if capabilities.network => {}
            "network" => return Err("安装 grant 未允许 network capability"),
            _ => return Err("Plan 声明了 runtime 不支持的 capability"),
        }
    }
    if matches!(declaration.kind, EffectKind::Http | EffectKind::QuickJs) && !capabilities.network {
        return Err("安装 grant 未允许 network capability");
    }
    Ok(())
}
