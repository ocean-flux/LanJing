//! HTTP 节点处理器 — 发 HTTP 请求产 `HttpResponse` stream(ADR-0022)。

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

use async_stream::stream;
use futures::stream::{BoxStream, StreamExt};

use lj_core::endpoint::HttpMethod;
use lj_core::error::CoreError;
use lj_core::media::{parse_item_resource_id, parse_unit_resource_id};
use lj_core::node::{NodeKind, NodeSpec};
use lj_core::node_data::{HttpResponse, NodeData, NodeDataVariant};
use lj_core::traits::{ExecutionContext, NodeProcessor};
use serde_json::Value;

use crate::ssrf;
use crate::util;

/// HTTP body 最大 16 MiB(KTD15)。
const MAX_BODY_SIZE: usize = 16 * 1024 * 1024;

/// 最大 redirect 跳数(KTD8)。
const MAX_REDIRECTS: usize = 5;

/// TCP 连接建立超时(reliability #4:防慢/挂第三方站点无限阻塞)。
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// 单次请求总超时(含 DNS/TLS/传输/响应,reliability #4)。
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// 测试模式共享 `Client`(无 SSRF,自动 redirect,连接池复用)。
fn test_client() -> Result<&'static reqwest::Client, CoreError> {
    static CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    let cached = CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::limited(5))
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|e| format!("测试模式 reqwest client 创建失败: {e}"))
    });
    cached.as_ref().map_err(|msg| CoreError::Other(msg.clone()))
}

/// SSRF HTTP 模式共享 `Client`(禁用自动 redirect,连接池复用)。
/// HTTPS 场景需 per-host resolve,不适用此共享 client。
fn ssrf_http_client() -> Result<&'static reqwest::Client, CoreError> {
    static CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    let cached = CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|e| format!("SSRF HTTP reqwest client 创建失败: {e}"))
    });
    cached.as_ref().map_err(|msg| CoreError::Other(msg.clone()))
}

/// 拼接 Cookie 请求头字符串。
fn cookie_str(ctx: &ExecutionContext) -> Option<String> {
    if ctx.cookies.is_empty() {
        return None;
    }
    Some(
        ctx.cookies
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; "),
    )
}

/// 构建 HTTP 请求(方法、Host 头、自定义头、Cookie)。
fn build_request(
    client: &reqwest::Client,
    spec: &lj_core::endpoint::HttpSpec,
    ctx: &ExecutionContext,
    url: &str,
    host_header: Option<&str>,
) -> reqwest::RequestBuilder {
    let mut req = match spec.method {
        HttpMethod::Get => client.get(url),
        HttpMethod::Post => {
            let mut r = client.post(url);
            if let Some(body) = &spec.body {
                r = r.body(body.clone());
            }
            r
        }
    };

    if let Some(h) = host_header {
        req = req.header("Host", h);
    }

    for (k, v) in &spec.headers {
        req = req.header(k.as_str(), v.as_str());
    }

    if let Some(c) = cookie_str(ctx) {
        req = req.header("Cookie", c);
    }

    req
}

/// HTTP 节点处理器。
///
/// 发送 HTTP 请求,将 `reqwest::Response` 转换为 `HttpResponse` 产出。
/// cookie jar 首刀通过 `ExecutionContext.cookies` 传递,后续可换 `cookie_store`。
pub struct HttpNodeProcessor {
    /// 是否启用 SSRF 防护(默认 true,测试可关闭)。
    ssrf_enabled: bool,
}

#[derive(Default)]
struct TemplateInput {
    key: Option<String>,
    page: Option<u32>,
    book_url: Option<String>,
    chapter_url: Option<String>,
    vod_id: Option<String>,
    type_: Option<String>,
}

impl HttpNodeProcessor {
    /// 创建生产环境处理器(SSRF 防护开启)。
    #[must_use]
    pub fn new() -> Self {
        Self { ssrf_enabled: true }
    }

    /// 创建测试环境处理器(SSRF 防护关闭,允许访问环回地址)。
    #[must_use]
    pub fn new_test() -> Self {
        Self {
            ssrf_enabled: false,
        }
    }
}

impl Default for HttpNodeProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeProcessor for HttpNodeProcessor {
    fn kind(&self) -> NodeKind {
        NodeKind::Http
    }

    fn input_type(&self) -> Option<NodeDataVariant> {
        None
    }

    fn output_type(&self) -> NodeDataVariant {
        NodeDataVariant::HttpResponse
    }

    fn process<'a>(
        &'a self,
        ctx: &'a ExecutionContext,
        spec: &'a NodeSpec,
        input: BoxStream<'a, NodeData>,
    ) -> BoxStream<'a, NodeData> {
        let http_spec = match &spec.http {
            Some(h) => h.clone(),
            None => return Box::pin(futures::stream::empty()),
        };

        // 从输入流中提取首项作为搜索关键词
        let key_future = input.into_future();

        Box::pin(stream! {
            let (first_item, _rest) = key_future.await;
            let template_input = template_input(first_item);

            // 渲染 URL 模板(测试模式)
            let url_str = util::render_url_template(
                &http_spec.url,
                template_input.key.as_deref(),
                template_input.page.or(Some(1)),
                template_input.book_url.as_deref(),
                template_input.chapter_url.as_deref(),
                template_input.vod_id.as_deref(),
                template_input.type_.as_deref(),
            );
            let url_str = resolve_request_url(&url_str, &ctx.base_url);
            if self.ssrf_enabled {
                yield execute_ssrf_request(&http_spec, ctx, &url_str).await;
            } else {
                // ── 测试模式: 跳过 SSRF,自动 redirect ──
                let host = host_with_port(&url_str);

                let host_header = if host.is_empty() { None } else { Some(host.as_str()) };

                let shared_client = match test_client() {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!(error = %e, "测试模式共享 client 获取失败");
                        yield NodeData::Error(format!("测试模式共享 client 获取失败: {e}"));
                        return;
                    }
                };

                let req = build_request(shared_client, &http_spec, ctx, &url_str, host_header);

                let resp = match req.send().await {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::error!(%url_str, error = %e, "HTTP 请求失败");
                        yield NodeData::Error(format!("HTTP 请求失败: {e}"));
                        return;
                    }
                };

                match convert_response(resp).await {
                    Ok(r) => yield NodeData::HttpResponse(r),
                    Err(e) => {
                        tracing::error!(%url_str, error = %e, "响应转换失败");
                        yield NodeData::Error(format!("响应转换失败: {e}"));
                    }
                }
            }
        })
    }
}

fn template_input(item: Option<NodeData>) -> TemplateInput {
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

fn resolve_request_url(url_str: &str, base_url: &str) -> String {
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

/// SSRF 模式请求执行:手动 redirect 循环,每跳 IP 固定防 DNS rebinding TOCTOU。
///
/// 抽为独立函数避免 `process` 过长(clippy too-many-lines),逻辑内聚。
async fn execute_ssrf_request(
    http_spec: &lj_core::endpoint::HttpSpec,
    ctx: &ExecutionContext,
    url_str: &str,
) -> NodeData {
    let initial_target = match ssrf::validate_url_and_pin(url_str).await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(url = url_str, error = %e, "SSRF 阻断请求");
            return NodeData::Error(format!("SSRF 阻断: {e}"));
        }
    };
    let is_https = url_str.starts_with("https://");
    let mut current_target = initial_target;
    let mut redirect_count = 0usize;
    loop {
        // 每跳构建 client:HTTPS 重建 + resolve 当前 host 防 DNS rebinding TOCTOU;
        // HTTP 复用共享 client(IP 已在 URL 中,无需 per-host resolve)。
        // 关键:HTTPS 重定向到新 host 时这里重新 pin,消除 TOCTOU 窗口。
        let owned_client;
        let client: &reqwest::Client = if is_https {
            let mut builder = reqwest::Client::builder()
                .cookie_store(true)
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(CONNECT_TIMEOUT)
                .timeout(REQUEST_TIMEOUT);
            if let Some(first_addr) = current_target.addrs.first()
                && let Ok(parsed) = url::Url::parse(&current_target.url)
                && let Some(host) = parsed.host_str()
            {
                builder = builder.resolve(host, *first_addr);
            }
            owned_client = match builder.build() {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!(error = %e, "reqwest 客户端创建失败");
                    return NodeData::Error(format!("reqwest 客户端创建失败: {e}"));
                }
            };
            &owned_client
        } else {
            match ssrf_http_client() {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!(error = %e, "SSRF HTTP 共享 client 获取失败");
                    return NodeData::Error(format!("SSRF HTTP 共享 client 获取失败: {e}"));
                }
            }
        };
        let req = build_request(
            client,
            http_spec,
            ctx,
            &current_target.url,
            Some(&current_target.host_header),
        );
        let resp = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::error!(url = %current_target.url, host = %current_target.host_header, error = %e, "HTTP 请求失败");
                return NodeData::Error(format!("HTTP 请求失败: {e}"));
            }
        };
        if resp.status().is_redirection() {
            redirect_count += 1;
            if redirect_count > MAX_REDIRECTS {
                tracing::warn!(url = %current_target.url, "超过最大重定向次数");
                return NodeData::Error(format!("超过最大重定向次数({MAX_REDIRECTS})"));
            }
            let Some(loc) = resp.headers().get("location") else {
                return NodeData::Error("重定向响应缺少 Location 头".into());
            };
            let location = loc.to_str().unwrap_or_default().to_string();
            let next_url =
                match url::Url::parse(&current_target.url).and_then(|base| base.join(&location)) {
                    Ok(u) => u.to_string(),
                    Err(e) => return NodeData::Error(format!("重定向 URL 解析失败: {e}")),
                };
            current_target = match ssrf::validate_url_and_pin(&next_url).await {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!(url = %next_url, error = %e, "SSRF 阻断重定向");
                    return NodeData::Error(format!("SSRF 阻断重定向: {e}"));
                }
            };
            continue;
        }
        return match convert_response(resp).await {
            Ok(r) => NodeData::HttpResponse(r),
            Err(e) => {
                tracing::error!(url = %current_target.url, error = %e, "响应转换失败");
                NodeData::Error(format!("响应转换失败: {e}"))
            }
        };
    }
}

/// 从 URL 解析 `host:port` 字符串(无 port 则仅 host)，解析失败返回空串。
fn host_with_port(url_str: &str) -> String {
    url::Url::parse(url_str)
        .ok()
        .and_then(|p| {
            p.host_str().map(|h| {
                if let Some(port) = p.port() {
                    format!("{h}:{port}")
                } else {
                    h.to_string()
                }
            })
        })
        .unwrap_or_default()
}

/// 将 `reqwest::Response` 转换为 `HttpResponse`。
///
/// body 流式读取,累计计数,超 `MAX_BODY_SIZE` 返回 `CoreError::BodyTooLarge`(KTD15)。
///
/// # Errors
///
/// 返回 `CoreError::NodeExecution` 当 chunk 读取失败。
/// 返回 `CoreError::BodyTooLarge` 当响应体超过 `MAX_BODY_SIZE`。
pub async fn convert_response(mut resp: reqwest::Response) -> Result<HttpResponse, CoreError> {
    let status = resp.status().as_u16();

    let headers: HashMap<String, String> = resp
        .headers()
        .iter()
        .map(|(k, v)| {
            let key = k.to_string();
            let val = v.to_str().unwrap_or_default().to_string();
            (key, val)
        })
        .collect();

    // 流式读取 body 带大小限制(KTD15)
    let mut body = Vec::with_capacity(MAX_BODY_SIZE.min(4096));
    loop {
        let chunk = resp
            .chunk()
            .await
            .map_err(|e| CoreError::NodeExecution(e.to_string()))?;
        let Some(chunk) = chunk else { break };
        let remaining = MAX_BODY_SIZE.saturating_sub(body.len());
        if chunk.len() > remaining {
            body.extend_from_slice(&chunk[..remaining]);
            let actual = body.len();
            tracing::warn!("HTTP body 超过上限: {actual} bytes (上限 {MAX_BODY_SIZE})",);
            // 返回错误而非静默截断(KTD15, P2-25)
            return Err(CoreError::BodyTooLarge {
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
