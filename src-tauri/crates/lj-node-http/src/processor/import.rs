//! Legado 导入地址的受限 HTTP 拉取 façade。
//!
//! 该模块复用 HTTP effect 的 URL 校验、逐跳 DNS pin、手动 redirect 与流式 body 读取，
//! 但不接触安装 grant、来源凭据或 `RuleSystem` candidate。所有公开错误都经过收敛，不携带 URL
//! query、响应 body 或底层网络错误。

use std::collections::HashMap;
use std::time::Duration;

use lj_rule_model::{ExpectedDataType, HttpMethod, HttpSpec};
use lj_runtime::{HttpEffectWitness, HttpExecutionCredentials, HttpRequestWitness};
use thiserror::Error;

use super::redirect::{execute_direct_response_with_limit, execute_ssrf_response_with_limit};
use super::request::{HttpRequestError, safe_url};

/// 深链导入响应体硬上限：2 MiB。
pub const IMPORT_BODY_MAX_BYTES: usize = 2 * 1024 * 1024;

/// 整次导入（含 DNS、redirect 与 body）总超时。
const IMPORT_FETCH_TIMEOUT: Duration = Duration::from_secs(30);

/// 导入地址拉取的安全失败类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ImportFetchError {
    /// 初始 URL 不是不含用户凭据的 HTTP(S) 地址。
    #[error("import_src_url_invalid: 仅支持不含用户凭据的 http/https 地址")]
    InvalidUrl,
    /// DNS、SSRF 或 redirect 目标校验失败。
    #[error("import_src_target_blocked: 导入地址未通过网络安全校验")]
    TargetBlocked,
    /// 整次拉取或底层请求超时。
    #[error("import_src_timeout: 获取导入内容超时")]
    Timeout,
    /// 请求构建、发送或响应读取失败。
    #[error("import_src_request_failed: 无法安全获取导入内容")]
    RequestFailed,
    /// redirect 缺少安全 Location 或超过五跳。
    #[error("import_src_redirect_invalid: 导入地址重定向无效或超过上限")]
    RedirectInvalid,
    /// 远端返回非 2xx 状态；只暴露非敏感状态码。
    #[error("import_src_http_status: 远程地址返回非成功状态 ({0})")]
    HttpStatus(u16),
    /// 解压后的响应体超过导入专用上限。
    #[error("import_src_body_too_large: 导入内容超过 2 MiB 上限")]
    BodyTooLarge,
    /// 响应体不是 UTF-8，不能安全返回 Tauri String wire。
    #[error("import_src_invalid_utf8: 导入内容不是有效 UTF-8")]
    InvalidUtf8,
}

#[derive(Clone, Copy)]
struct ImportFetchPolicy {
    body_max_bytes: usize,
    timeout: Duration,
    ssrf_enabled: bool,
}

const PRODUCTION_POLICY: ImportFetchPolicy = ImportFetchPolicy {
    body_max_bytes: IMPORT_BODY_MAX_BYTES,
    timeout: IMPORT_FETCH_TIMEOUT,
    ssrf_enabled: true,
};

/// 拉取一个供导入预览使用的 UTF-8 JSON 文本。
///
/// 仅允许不含用户凭据的 `http`/`https` URL。请求复用 production SSRF/DNS pin 与手动
/// redirect seam，解压后的 body 最多 2 MiB，整次操作最多 30 秒。函数不会携带来源安装
/// capability，也不会返回响应 header、body 片段或底层错误详情。
///
/// # Errors
///
/// URL/SSRF/redirect 校验、超时、非 2xx 状态、body 上限、响应读取或 UTF-8 校验失败时，
/// 返回不包含 URL query 与响应内容的 [`ImportFetchError`]。
pub async fn fetch_import_source(url: &str) -> Result<String, ImportFetchError> {
    fetch_import_source_with_policy(url, PRODUCTION_POLICY).await
}

async fn fetch_import_source_with_policy(
    url: &str,
    policy: ImportFetchPolicy,
) -> Result<String, ImportFetchError> {
    let safe_request_url = safe_url(url).map_err(|_| ImportFetchError::InvalidUrl)?;
    let fetch = fetch_validated_import_source(url, safe_request_url, policy);
    tokio::time::timeout(policy.timeout, fetch)
        .await
        .map_err(|_| ImportFetchError::Timeout)?
}

async fn fetch_validated_import_source(
    url: &str,
    safe_request_url: String,
    policy: ImportFetchPolicy,
) -> Result<String, ImportFetchError> {
    let spec = HttpSpec {
        method: HttpMethod::Get,
        url: url.to_string(),
        headers: HashMap::new(),
        body: None,
        charset: None,
        expected_type: ExpectedDataType::Json,
    };
    let credentials = HttpExecutionCredentials::default();
    let mut witness = HttpEffectWitness {
        request: HttpRequestWitness {
            method: HttpMethod::Get,
            safe_url: safe_request_url,
            headers: Vec::new(),
            body: None,
        },
        redirects: Vec::new(),
        dns_targets: Vec::new(),
        error: None,
        duration_ms: 0,
    };

    let response = if policy.ssrf_enabled {
        execute_ssrf_response_with_limit(
            &spec,
            url,
            &credentials,
            None,
            &mut witness,
            policy.body_max_bytes,
        )
        .await
    } else {
        execute_direct_response_with_limit(
            &spec,
            url,
            &credentials,
            None,
            &mut witness,
            policy.body_max_bytes,
        )
        .await
    }
    .map_err(map_http_error)?;

    if !(200..300).contains(&response.status) {
        return Err(ImportFetchError::HttpStatus(response.status));
    }

    String::from_utf8(response.body).map_err(|_| ImportFetchError::InvalidUtf8)
}

const fn map_http_error(error: HttpRequestError) -> ImportFetchError {
    match error {
        HttpRequestError::TargetValidation => ImportFetchError::TargetBlocked,
        HttpRequestError::Timeout => ImportFetchError::Timeout,
        HttpRequestError::Redirect => ImportFetchError::RedirectInvalid,
        HttpRequestError::BodyTooLarge => ImportFetchError::BodyTooLarge,
        HttpRequestError::Cancelled
        | HttpRequestError::Request
        | HttpRequestError::ResponseRead => ImportFetchError::RequestFailed,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::{
        IMPORT_BODY_MAX_BYTES, ImportFetchError, ImportFetchPolicy, fetch_import_source,
        fetch_import_source_with_policy,
    };

    fn local_policy(body_max_bytes: usize, timeout: Duration) -> ImportFetchPolicy {
        ImportFetchPolicy {
            body_max_bytes,
            timeout,
            ssrf_enabled: false,
        }
    }

    #[test]
    fn import_body_limit_is_two_mib() {
        assert_eq!(IMPORT_BODY_MAX_BYTES, 2 * 1024 * 1024);
    }

    #[tokio::test]
    async fn import_fetch_rejects_non_http_and_userinfo_before_network() {
        assert_eq!(
            fetch_import_source("file:///tmp/source.json").await,
            Err(ImportFetchError::InvalidUrl)
        );
        assert_eq!(
            fetch_import_source("https://user:secret@example.com/source.json").await,
            Err(ImportFetchError::InvalidUrl)
        );
    }

    #[tokio::test]
    async fn import_fetch_reuses_production_ssrf_for_loopback() {
        assert_eq!(
            fetch_import_source("http://127.0.0.1:9/source.json").await,
            Err(ImportFetchError::TargetBlocked)
        );
    }

    #[tokio::test]
    async fn import_fetch_follows_manual_redirect_and_returns_utf8() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/start"))
            .respond_with(ResponseTemplate::new(302).insert_header("location", "/final"))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/final"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(r#"{"bookSourceName":"测试"}"#),
            )
            .mount(&server)
            .await;

        let body = fetch_import_source_with_policy(
            &format!("{}/start", server.uri()),
            local_policy(1024, Duration::from_secs(1)),
        )
        .await
        .expect("本地测试 seam 应跟随手动 redirect");
        assert_eq!(body, r#"{"bookSourceName":"测试"}"#);
    }

    #[tokio::test]
    async fn import_fetch_rejects_body_over_configured_limit() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/large"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![b'x'; 129]))
            .mount(&server)
            .await;

        let result = fetch_import_source_with_policy(
            &format!("{}/large", server.uri()),
            local_policy(128, Duration::from_secs(1)),
        )
        .await;
        assert_eq!(result, Err(ImportFetchError::BodyTooLarge));
    }

    #[tokio::test]
    async fn import_fetch_times_out_as_one_bounded_operation() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/slow"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_millis(200))
                    .set_body_string("too late"),
            )
            .mount(&server)
            .await;

        let result = fetch_import_source_with_policy(
            &format!("{}/slow", server.uri()),
            local_policy(1024, Duration::from_millis(20)),
        )
        .await;
        assert_eq!(result, Err(ImportFetchError::Timeout));
    }
}
