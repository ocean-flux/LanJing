//! live HTTP effect adapter 编排。
//!
//! 这里是唯一解码 execution-only credential 并发起 live request 的位置。replay 在 runtime
//! archive 层完成，绝不能调用本 adapter；adapter 只返回类型化 output 和脱敏 witness，
//! 由 runtime 决定 durable-before-advance。

use std::time::Instant;

use lj_rule_model::Capability;
use lj_runtime::{
    CapturedEffectOutput, EffectCancellation, EffectError, EffectErrorCode, EffectFailure,
    EffectOutput, EffectWitness, HttpEffectHandler, HttpEffectRequest, HttpEffectWitness,
    HttpRequestBodyWitness, HttpRequestWitness, effect_bytes_hash,
};

use crate::util;

use super::redirect::{execute_direct_response, execute_ssrf_response};
use super::request::{
    HttpRequestError, effect_input_to_node_data, resolve_request_url, safe_request_headers,
    safe_url, template_input,
};

/// HTTP Plan effect adapter。
///
/// live mode 仅在此处解码 execution-only source secret，并将其标记为 sensitive request
/// headers；replay 在 runtime 层直接读 archive，因此不会调用本 adapter。
pub struct HttpEffectAdapter {
    /// 是否启用 SSRF 防护(默认 true,测试可关闭)。
    ssrf_enabled: bool,
}

impl HttpEffectAdapter {
    /// 创建生产环境 effect adapter（SSRF 防护开启）。
    #[must_use]
    pub fn new() -> Self {
        Self { ssrf_enabled: true }
    }

    /// 创建测试环境 effect adapter（SSRF 防护关闭，允许访问环回地址）。
    #[must_use]
    pub fn new_test() -> Self {
        Self {
            ssrf_enabled: false,
        }
    }
}

impl Default for HttpEffectAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl HttpEffectHandler for HttpEffectAdapter {
    async fn execute_http(
        &self,
        request: HttpEffectRequest,
        cancellation: EffectCancellation,
    ) -> Result<CapturedEffectOutput, EffectError> {
        if cancellation.is_cancelled() {
            return Err(EffectError::new(
                EffectErrorCode::Cancelled,
                "HTTP effect 已取消",
            ));
        }
        lj_runtime::check_capability(&request.capabilities, Capability::Network).map_err(|_| {
            EffectError::new(
                EffectErrorCode::CapabilityDenied,
                "安装 grant 未允许 network capability",
            )
        })?;
        let started = Instant::now();

        let HttpEffectRequest {
            spec,
            input,
            base_url,
            credentials,
            ..
        } = request;
        let template_input = template_input(Some(effect_input_to_node_data(&input)?));
        let url = util::render_url_template(
            &spec.url,
            template_input.key.as_deref(),
            template_input.page.or(Some(1)),
            template_input.book_url.as_deref(),
            template_input.chapter_url.as_deref(),
            template_input.vod_id.as_deref(),
            template_input.type_.as_deref(),
        );
        let url = resolve_request_url(&url, &base_url);
        let request_body = spec.body.as_ref().map(|body| body.as_bytes().to_vec());
        let mut witness = HttpEffectWitness {
            request: HttpRequestWitness {
                method: spec.method.clone(),
                safe_url: safe_url(&url).map_err(effect_error_from_http)?,
                headers: safe_request_headers(&spec.headers),
                body: request_body.as_ref().map(|body| HttpRequestBodyWitness {
                    hash: effect_bytes_hash(body),
                    byte_len: u64::try_from(body.len()).unwrap_or(u64::MAX),
                }),
            },
            redirects: Vec::new(),
            dns_targets: Vec::new(),
            error: None,
            duration_ms: 0,
        };
        let result = if self.ssrf_enabled {
            execute_ssrf_response(&spec, &url, &credentials, Some(&cancellation), &mut witness)
                .await
        } else {
            execute_direct_response(&spec, &url, &credentials, Some(&cancellation), &mut witness)
                .await
        };
        witness.duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        match result {
            Ok(response) => Ok(CapturedEffectOutput::new(
                EffectOutput::Http(response),
                EffectWitness::Http(witness),
            )
            .with_http_request_body(request_body)),
            Err(HttpRequestError::Cancelled) => {
                Err(effect_error_from_http(HttpRequestError::Cancelled))
            }
            Err(error) => {
                let error = error
                    .witness_kind()
                    .expect("non-cancelled HTTP error must have witness kind");
                witness.error = Some(error);
                Ok(CapturedEffectOutput::new(
                    EffectOutput::Failure(EffectFailure::Http { error }),
                    EffectWitness::Http(witness),
                )
                .with_http_request_body(request_body))
            }
        }
    }
}

fn effect_error_from_http(error: HttpRequestError) -> EffectError {
    match error {
        HttpRequestError::Cancelled => {
            EffectError::new(EffectErrorCode::Cancelled, "HTTP effect 已取消")
        }
        HttpRequestError::TargetValidation
        | HttpRequestError::Request
        | HttpRequestError::Redirect
        | HttpRequestError::ResponseRead => {
            EffectError::new(EffectErrorCode::HttpRequest, "HTTP effect 执行失败")
        }
    }
}
