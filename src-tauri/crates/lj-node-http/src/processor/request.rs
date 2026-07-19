//! HTTP request 构造、模板输入、安全摘要与响应读取。
//!
//! 本模块只处理一次请求的 material：live-only 凭据以 sensitive header 注入，witness 只保留
//! 非敏感 header hash、去敏 URL 与 request body hash。响应 body 使用流式上限读取，避免把
//! 不受限的数据放进 runtime 内存。

use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Duration;

use reqwest::header::{self, HeaderMap, HeaderName, HeaderValue};

use lj_capability::IntentInput;
use lj_media::{parse_item_resource_id, parse_unit_resource_id};
use lj_rule_model::{Error, HttpMethod};
use lj_runtime::{
    EffectCancellation, EffectError, EffectErrorCode, EffectInput, EffectOutput,
    HttpEffectErrorKind, HttpExecutionCredentials, HttpRequestHeaderWitness, HttpResponse,
    NodeData, effect_bytes_hash,
};
use serde_json::Value;

use crate::util;

/// HTTP body 最大 16 MiB(KTD15)。
const MAX_BODY_SIZE: usize = 16 * 1024 * 1024;

/// TCP 连接建立超时(reliability #4:防慢/挂第三方站点无限阻塞)。
pub(super) const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// 单次请求总超时(含 DNS/TLS/传输/响应,reliability #4)。
pub(super) const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// 测试模式共享 `Client`（无 SSRF、手动 redirect、连接池复用）。
///
/// 不启用 reqwest cookie store；来源凭据只能由 execution-only header 显式注入，避免
/// 共享 client 在不同 source cookie namespace 间泄漏响应 cookie。
pub(super) fn test_client() -> Result<&'static reqwest::Client, Error> {
    static CLIENT: LazyLock<Result<reqwest::Client, String>> = LazyLock::new(|| {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|error| format!("测试模式 reqwest client 创建失败: {error}"))
    });
    CLIENT
        .as_ref()
        .map_err(|message| Error::Other(message.clone()))
}

/// SSRF HTTP 模式共享 `Client`（禁用自动 redirect，连接池复用）。
/// HTTPS 场景需 per-host resolve，不适用此共享 client。
pub(super) fn ssrf_http_client() -> Result<&'static reqwest::Client, Error> {
    static CLIENT: LazyLock<Result<reqwest::Client, String>> = LazyLock::new(|| {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|error| format!("SSRF HTTP reqwest client 创建失败: {error}"))
    });
    CLIENT
        .as_ref()
        .map_err(|message| Error::Other(message.clone()))
}

#[derive(Clone, Copy)]
pub(super) enum HttpRequestError {
    Cancelled,
    TargetValidation,
    Request,
    Redirect,
    ResponseRead,
}

impl HttpRequestError {
    pub(super) const fn witness_kind(self) -> Option<HttpEffectErrorKind> {
        match self {
            Self::Cancelled => None,
            Self::TargetValidation => Some(HttpEffectErrorKind::TargetValidation),
            Self::Request => Some(HttpEffectErrorKind::Request),
            Self::Redirect => Some(HttpEffectErrorKind::Redirect),
            Self::ResponseRead => Some(HttpEffectErrorKind::ResponseRead),
        }
    }
}

/// 构建 Plan HTTP effect 请求，并将 execution-only 凭据作为 sensitive header 注入。
///
/// 机密 header 覆盖 Plan 中同名 header，且拒绝改变 Host/连接/传输长度等路由或 hop-by-hop
/// 字段。该函数不会记录 header 名称或值。
pub(super) fn build_request(
    client: &reqwest::Client,
    spec: &lj_rule_model::HttpSpec,
    url: &str,
    host_header: Option<&str>,
    credentials: &HttpExecutionCredentials,
) -> Result<reqwest::RequestBuilder, HttpRequestError> {
    let request = match spec.method {
        HttpMethod::Get => client.get(url),
        HttpMethod::Post => {
            let mut request = client.post(url);
            if let Some(body) = &spec.body {
                request = request.body(body.clone());
            }
            request
        }
    };
    let mut headers = HeaderMap::new();
    for (name, value) in &spec.headers {
        insert_header(&mut headers, name, value, false)?;
    }
    let secret_headers = credentials
        .decode_headers()
        .map_err(|_| HttpRequestError::Request)?;
    for (name, value) in secret_headers.iter() {
        let header_name =
            HeaderName::from_bytes(name.as_bytes()).map_err(|_| HttpRequestError::Request)?;
        if forbidden_secret_header(&header_name) {
            return Err(HttpRequestError::Request);
        }
        insert_header(&mut headers, name, value, true)?;
    }
    if let Some(host) = host_header {
        insert_header(&mut headers, header::HOST.as_str(), host, false)?;
    }
    Ok(request.headers(headers))
}

fn insert_header(
    headers: &mut HeaderMap,
    name: &str,
    value: &str,
    sensitive: bool,
) -> Result<(), HttpRequestError> {
    let name = HeaderName::from_bytes(name.as_bytes()).map_err(|_| HttpRequestError::Request)?;
    let mut value = HeaderValue::from_str(value).map_err(|_| HttpRequestError::Request)?;
    value.set_sensitive(sensitive);
    headers.insert(name, value);
    Ok(())
}

fn forbidden_secret_header(name: &HeaderName) -> bool {
    name == header::HOST
        || name == header::CONTENT_LENGTH
        || name == header::TRANSFER_ENCODING
        || name == header::CONNECTION
        || name == header::PROXY_AUTHORIZATION
}

#[derive(Default)]
pub(super) struct TemplateInput {
    pub(super) key: Option<String>,
    pub(super) page: Option<u32>,
    pub(super) book_url: Option<String>,
    pub(super) chapter_url: Option<String>,
    pub(super) vod_id: Option<String>,
    pub(super) type_: Option<String>,
}

pub(super) fn effect_input_to_node_data(input: &EffectInput) -> Result<NodeData, EffectError> {
    match input {
        EffectInput::Intent(
            IntentInput::Query(value)
            | IntentInput::ItemId(value)
            | IntentInput::UnitId(value)
            | IntentInput::ActionId(value)
            | IntentInput::Page(value),
        ) => Ok(NodeData::Raw(value.clone())),
        EffectInput::Intent(IntentInput::Opaque(value)) => Ok(NodeData::Json(value.clone())),
        EffectInput::Intent(IntentInput::None) => Ok(NodeData::Raw(String::new())),
        EffectInput::Output(output) => match output.as_ref() {
            EffectOutput::QuickJs(lj_runtime::QuickJsOutput::Json(value)) => {
                Ok(NodeData::Json(value.clone()))
            }
            EffectOutput::QuickJs(lj_runtime::QuickJsOutput::Raw(value)) => {
                Ok(NodeData::Raw(value.clone()))
            }
            EffectOutput::QuickJs(lj_runtime::QuickJsOutput::Error(_)) => Err(EffectError::new(
                EffectErrorCode::InputType,
                "HTTP effect 不能消费失败的 QuickJS 输出",
            )),
            EffectOutput::Extract(output) => Ok(NodeData::Json(serde_json::Value::Array(
                output.records.clone(),
            ))),
            EffectOutput::Http(_) => Err(EffectError::new(
                EffectErrorCode::InputType,
                "HTTP effect 不能直接消费 HTTP 响应",
            )),
            EffectOutput::Failure(_) => Err(EffectError::new(
                EffectErrorCode::InputType,
                "HTTP effect 不能消费失败的 effect 输出",
            )),
        },
    }
}

pub(super) fn template_input(item: Option<NodeData>) -> TemplateInput {
    match item {
        Some(NodeData::Raw(value)) => template_input_from_raw(&value),
        Some(NodeData::Json(value)) => template_input_from_json(&value),
        _ => TemplateInput::default(),
    }
}

fn template_input_from_raw(value: &str) -> TemplateInput {
    if let Some((_source_id, source_key)) = parse_item_resource_id(value) {
        return TemplateInput {
            key: Some(source_key.clone()),
            book_url: Some(source_key.clone()),
            chapter_url: Some(source_key.clone()),
            vod_id: Some(source_key),
            ..TemplateInput::default()
        };
    }
    if let Some((_source_id, item_source_key, unit_source_key)) = parse_unit_resource_id(value) {
        return TemplateInput {
            key: Some(unit_source_key.clone()),
            book_url: Some(item_source_key.clone()),
            chapter_url: Some(unit_source_key),
            vod_id: Some(item_source_key),
            ..TemplateInput::default()
        };
    }
    TemplateInput {
        key: Some(value.to_string()),
        book_url: Some(value.to_string()),
        chapter_url: Some(value.to_string()),
        vod_id: Some(value.to_string()),
        ..TemplateInput::default()
    }
}

fn template_input_from_json(value: &Value) -> TemplateInput {
    TemplateInput {
        key: json_string(value, &["key", "query", "url"]),
        page: json_page(value),
        book_url: json_string(value, &["bookUrl", "book_url", "url"]),
        chapter_url: json_string(value, &["chapterUrl", "chapter_url", "url"]),
        vod_id: json_string(value, &["vod_id", "vodId", "source_item_id", "id"]),
        type_: json_string(value, &["type", "type_id"]),
    }
}

fn json_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(ToString::to_string)
    })
}

fn json_page(value: &Value) -> Option<u32> {
    if let Some(page) = value.get("page").and_then(Value::as_u64) {
        return u32::try_from(page).ok();
    }
    value
        .get("page")
        .and_then(Value::as_str)
        .and_then(|page| page.parse::<u32>().ok())
}

pub(super) fn safe_url(raw_url: &str) -> Result<String, HttpRequestError> {
    let parsed = url::Url::parse(raw_url).map_err(|_| HttpRequestError::TargetValidation)?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.username() != ""
        || parsed.password().is_some()
    {
        return Err(HttpRequestError::TargetValidation);
    }
    let host = parsed
        .host_str()
        .filter(|host| !host.is_empty())
        .ok_or(HttpRequestError::TargetValidation)?;
    let host = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    let port = parsed
        .port()
        .map_or_else(String::new, |port| format!(":{port}"));
    let path = if parsed.path().is_empty() {
        "/"
    } else {
        parsed.path()
    };
    Ok(format!("{}://{host}{port}{path}", parsed.scheme()))
}

pub(super) fn safe_request_headers(
    headers: &HashMap<String, String>,
) -> Vec<HttpRequestHeaderWitness> {
    let mut safe_headers = headers
        .iter()
        .filter_map(|(name, value)| {
            let name = name.to_ascii_lowercase();
            if !is_safe_header_name(&name) || is_sensitive_header_name(&name) {
                return None;
            }
            Some(HttpRequestHeaderWitness {
                name,
                value_hash: effect_bytes_hash(value.as_bytes()),
            })
        })
        .collect::<Vec<_>>();
    safe_headers.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.value_hash.cmp(&right.value_hash))
    });
    safe_headers
}

fn is_safe_header_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn is_sensitive_header_name(name: &str) -> bool {
    matches!(
        name,
        "authorization" | "cookie" | "set-cookie" | "proxy-authorization"
    ) || name.contains("token")
        || name.contains("secret")
        || name.contains("api-key")
}

pub(super) fn resolve_request_url(url_str: &str, base_url: &str) -> String {
    if url_str.starts_with("http://") || url_str.starts_with("https://") || base_url.is_empty() {
        return url_str.to_string();
    }
    url::Url::parse(base_url).map_or_else(
        |_| url_str.to_string(),
        |base| {
            base.join(url_str)
                .map_or_else(|_| url_str.to_string(), |url| url.to_string())
        },
    )
}

pub(super) async fn send_request(
    request: reqwest::RequestBuilder,
    cancellation: Option<&EffectCancellation>,
) -> Result<reqwest::Response, HttpRequestError> {
    if let Some(cancellation) = cancellation {
        tokio::select! {
            () = cancellation.cancelled() => Err(HttpRequestError::Cancelled),
            response = request.send() => response.map_err(|_| HttpRequestError::Request),
        }
    } else {
        request.send().await.map_err(|_| HttpRequestError::Request)
    }
}

pub(super) async fn convert_response_cancellable(
    response: reqwest::Response,
    cancellation: Option<&EffectCancellation>,
) -> Result<HttpResponse, HttpRequestError> {
    if let Some(cancellation) = cancellation {
        tokio::select! {
            () = cancellation.cancelled() => Err(HttpRequestError::Cancelled),
            response = convert_response(response) => response.map_err(|_| HttpRequestError::ResponseRead),
        }
    } else {
        convert_response(response)
            .await
            .map_err(|_| HttpRequestError::ResponseRead)
    }
}

/// 将 `reqwest::Response` 转换为 `HttpResponse`。
///
/// body 流式读取,累计计数,超 `MAX_BODY_SIZE` 返回 `Error::BodyTooLarge`(KTD15)。
///
/// # Errors
///
/// 返回 `Error::NodeExecution` 当 chunk 读取失败。
/// 返回 `Error::BodyTooLarge` 当响应体超过 `MAX_BODY_SIZE`。
pub async fn convert_response(mut resp: reqwest::Response) -> Result<HttpResponse, Error> {
    let status = resp.status().as_u16();

    let headers: HashMap<String, String> = resp
        .headers()
        .iter()
        .map(|(key, value)| {
            let key = key.to_string();
            let value = value.to_str().unwrap_or_default().to_string();
            (key, value)
        })
        .collect();

    // 流式读取 body 带大小限制(KTD15)。
    let mut body = Vec::with_capacity(MAX_BODY_SIZE.min(4096));
    loop {
        let chunk = resp
            .chunk()
            .await
            .map_err(|error| Error::NodeExecution(error.to_string()))?;
        let Some(chunk) = chunk else { break };
        let remaining = MAX_BODY_SIZE.saturating_sub(body.len());
        if chunk.len() > remaining {
            body.extend_from_slice(&chunk[..remaining]);
            let actual = body.len();
            tracing::warn!("HTTP body 超过上限: {actual} bytes (上限 {MAX_BODY_SIZE})");
            // 返回错误而非静默截断(KTD15, P2-25)。
            return Err(Error::BodyTooLarge {
                actual,
                max: MAX_BODY_SIZE,
            });
        }
        body.extend_from_slice(&chunk);
    }

    let charset = util::parse_charset(&headers);

    Ok(HttpResponse {
        status,
        headers,
        body,
        charset,
    })
}

#[cfg(test)]
mod tests {
    use super::template_input_from_raw;

    #[test]
    fn item_id_raw_becomes_vod_and_book_url() {
        let input = template_input_from_raw("item:736f757263653a74657374:313430373839");
        assert_eq!(input.vod_id.as_deref(), Some("140789"));
        assert_eq!(input.book_url.as_deref(), Some("140789"));
    }

    #[test]
    fn unit_id_raw_becomes_chapter_and_item_input() {
        let input = template_input_from_raw(
            "unit:736f757263653a74657374:2f626f6f6b2f31:2f726561642f312e68746d6c",
        );
        assert_eq!(input.book_url.as_deref(), Some("/book/1"));
        assert_eq!(input.chapter_url.as_deref(), Some("/read/1.html"));
        assert_eq!(input.vod_id.as_deref(), Some("/book/1"));
    }
}
