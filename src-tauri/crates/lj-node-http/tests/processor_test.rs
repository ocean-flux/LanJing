//! lj-node-http 集成测试。
//!
//! 覆盖 SSRF 校验、URL 模板渲染、URL 编码、字符集解析、`convert_response` 等核心路径。

use std::collections::{BTreeMap, HashMap};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use lj_capability::IntentInput;
use lj_node_http::processor::{HttpEffectAdapter, convert_response};
use lj_node_http::ssrf::is_blocked_ip;
use lj_node_http::util::{parse_charset, render_url_template};
use lj_rule_model::Error;
use lj_rule_model::PolicyCapabilities;
use lj_runtime::{
    CancellationHandle, EffectCapture, EffectCaptureMaterialSensitivity, EffectErrorCode,
    EffectInput, EffectOutput, EffectWitness, HttpDnsTargetKind, HttpEffectErrorKind,
    HttpEffectHandler, HttpEffectRequest, HttpExecutionCredentials, effect_bytes_hash,
};
use uuid::Uuid;

// ── SSRF 校验 ──────────────────────────────────────────

#[test]
fn ssrf_block_loopback_v4() {
    assert!(is_blocked_ip(&"127.0.0.1".parse::<IpAddr>().unwrap()));
}

#[test]
fn ssrf_block_aws_metadata() {
    assert!(is_blocked_ip(&"169.254.169.254".parse::<IpAddr>().unwrap()));
}

#[test]
fn ssrf_block_alibaba_metadata() {
    assert!(is_blocked_ip(&"100.100.100.200".parse::<IpAddr>().unwrap()));
}

#[test]
fn ssrf_block_rfc1918() {
    assert!(is_blocked_ip(&"192.168.1.1".parse::<IpAddr>().unwrap()));
    assert!(is_blocked_ip(&"10.0.0.1".parse::<IpAddr>().unwrap()));
    assert!(is_blocked_ip(&"172.16.0.1".parse::<IpAddr>().unwrap()));
}

#[test]
fn ssrf_block_loopback_v6() {
    assert!(is_blocked_ip(&"::1".parse::<IpAddr>().unwrap()));
}

#[test]
fn ssrf_allow_public() {
    assert!(!is_blocked_ip(&"8.8.8.8".parse::<IpAddr>().unwrap()));
    assert!(!is_blocked_ip(&"1.1.1.1".parse::<IpAddr>().unwrap()));
}

// ── URL 模板渲染 ──────────────────────────────────────

#[test]
fn url_template_no_substitution() {
    assert_eq!(
        render_url_template(
            "http://example.com/search",
            None,
            None,
            None,
            None,
            None,
            None
        ),
        "http://example.com/search"
    );
}

#[test]
fn url_template_key_only() {
    assert_eq!(
        render_url_template(
            "http://example.com/search?q={{key}}",
            Some("修罗"),
            None,
            None,
            None,
            None,
            None,
        ),
        "http://example.com/search?q=%E4%BF%AE%E7%BD%97"
    );
}

#[test]
fn url_template_key_and_page() {
    assert_eq!(
        render_url_template(
            "http://example.com/search?q={{key}}&page={{page}}",
            Some("修罗"),
            Some(1),
            None,
            None,
            None,
            None,
        ),
        "http://example.com/search?q=%E4%BF%AE%E7%BD%97&page=1"
    );
}

#[test]
fn url_template_page_only() {
    assert_eq!(
        render_url_template(
            "http://example.com/list?page={{page}}",
            None,
            Some(3),
            None,
            None,
            None,
            None,
        ),
        "http://example.com/list?page=3"
    );
}

#[test]
fn url_template_book_url() {
    assert_eq!(
        render_url_template(
            "http://example.com/book?url={{bookUrl}}",
            None,
            None,
            Some("http://source.com/book/123"),
            None,
            None,
            None,
        ),
        "http://example.com/book?url=http://source.com/book/123"
    );
}

#[test]
fn url_template_chapter_url() {
    assert_eq!(
        render_url_template(
            "http://example.com/chapter?url={{chapterUrl}}",
            None,
            None,
            None,
            Some("http://source.com/chapter/456"),
            None,
            None,
        ),
        "http://example.com/chapter?url=http://source.com/chapter/456"
    );
}

// ── URL 编码 ──────────────────────────────────────────

#[test]
fn url_encode_ascii() {
    let encoded = lj_node_http::util::url_encode("hello");
    assert_eq!(encoded, "hello");
}

#[test]
fn url_encode_chinese() {
    let encoded = lj_node_http::util::url_encode("修罗");
    assert_eq!(encoded, "%E4%BF%AE%E7%BD%97");
}

#[test]
fn url_encode_special_chars() {
    let encoded = lj_node_http::util::url_encode("a b");
    assert_eq!(encoded, "a%20b");
    let encoded = lj_node_http::util::url_encode("a/b");
    assert_eq!(encoded, "a%2Fb");
    let encoded = lj_node_http::util::url_encode("a?b");
    assert_eq!(encoded, "a%3Fb");
}

#[test]
fn url_encode_reserved_safe() {
    let encoded = lj_node_http::util::url_encode("a-b_c.d~e");
    assert_eq!(encoded, "a-b_c.d~e");
}

// ── 字符集解析 ────────────────────────────────────────

#[test]
fn parse_charset_utf8() {
    let mut headers = HashMap::new();
    headers.insert(
        "content-type".to_string(),
        "text/html; charset=utf-8".to_string(),
    );
    assert_eq!(parse_charset(&headers), Some("utf-8".to_string()));
}

#[test]
fn parse_charset_gbk() {
    let mut headers = HashMap::new();
    headers.insert(
        "content-type".to_string(),
        "text/html; charset=gbk".to_string(),
    );
    assert_eq!(parse_charset(&headers), Some("gbk".to_string()));
}

#[test]
fn parse_charset_no_charset() {
    let mut headers = HashMap::new();
    headers.insert("content-type".to_string(), "text/plain".to_string());
    assert_eq!(parse_charset(&headers), None);
}

#[test]
fn parse_charset_no_header() {
    let headers = HashMap::new();
    assert_eq!(parse_charset(&headers), None);
}

#[test]
fn parse_charset_edge_cases() {
    let mut headers = HashMap::new();
    // 无 charset
    headers.insert("content-type".into(), "text/plain".into());
    assert_eq!(parse_charset(&headers), None);
    // charset 带空格
    headers.insert("content-type".into(), "text/html; charset = utf-8".into());
    assert_eq!(parse_charset(&headers), Some("utf-8".into()));
    // 无 content-type 头
    let empty = HashMap::new();
    assert_eq!(parse_charset(&empty), None);
}

// ── wiremock 集成测试(convert_response) ───────────────

#[tokio::test]
async fn convert_response_get() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/test"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/plain; charset=utf-8")
                .set_body_string("Hello, world!"),
        )
        .mount(&mock_server)
        .await;

    let url = format!("{}/test", mock_server.uri());
    let resp = reqwest::get(&url).await.expect("HTTP 请求失败");
    let http_resp = convert_response(resp).await.expect("响应转换失败");

    assert_eq!(http_resp.status, 200);
    assert_eq!(http_resp.body, b"Hello, world!");
    assert!(
        http_resp.headers.contains_key("content-type"),
        "响应应包含 content-type 头"
    );
}

#[tokio::test]
async fn convert_response_body_size_limit() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;

    let large_body = vec![b'x'; 20 * 1024 * 1024]; // 20 MiB

    Mock::given(method("GET"))
        .and(path("/large"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(large_body))
        .mount(&mock_server)
        .await;

    let url = format!("{}/large", mock_server.uri());
    let resp = reqwest::get(&url).await.expect("HTTP 请求失败");
    let result = convert_response(resp).await;

    assert!(result.is_err(), "超过上限的 body 应返回错误");

    match &result.unwrap_err() {
        Error::BodyTooLarge { actual, max } => {
            assert_eq!(*max, 16 * 1024 * 1024, "上限应为 16 MiB");
            assert!(
                *actual <= 16 * 1024 * 1024,
                "实际大小 {actual} 不应超过上限"
            );
        }
        e => panic!("期望 BodyTooLarge 错误, 得到: {e}"),
    }
}

#[tokio::test]
async fn convert_response_post() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/submit"))
        .respond_with(
            ResponseTemplate::new(201)
                .insert_header("content-type", "application/json")
                .set_body_string(r#"{"status":"ok"}"#),
        )
        .mount(&mock_server)
        .await;

    let url = format!("{}/submit", mock_server.uri());
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .body(r#"{"q":"test"}"#)
        .send()
        .await
        .expect("HTTP 请求失败");
    let http_resp = convert_response(resp).await.expect("响应转换失败");

    assert_eq!(http_resp.status, 201);
    assert_eq!(http_resp.body, br#"{"status":"ok"}"#);
}

#[tokio::test]
async fn plan_http_effect_returns_typed_response() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/typed"))
        .respond_with(ResponseTemplate::new(200).set_body_string("typed HTTP output"))
        .mount(&server)
        .await;
    let processor = HttpEffectAdapter::new_test();
    let capture = processor
        .execute_http(
            http_effect_request(format!("{}/typed", server.uri())),
            CancellationHandle::new().token(),
        )
        .await
        .expect("HTTP effect 应返回类型化响应");

    let EffectOutput::Http(response) = &capture.output else {
        panic!("HTTP effect 必须返回 HTTP 类型化输出");
    };
    assert_eq!(response.status, 200);
    assert_eq!(response.body, b"typed HTTP output");
    capture
        .validate()
        .expect("HTTP output 必须与安全 witness 绑定");
}

#[tokio::test]
async fn plan_http_effect_injects_source_secret_headers() {
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/credentials"))
        .and(header("authorization", "Bearer execution-secret"))
        .and(header("cookie", "session=private"))
        .and(header("x-api-key", "source-only-key"))
        .respond_with(ResponseTemplate::new(200).set_body_string("credentialed"))
        .mount(&server)
        .await;
    let mut request = http_effect_request(format!("{}/credentials", server.uri()));
    request.credentials = HttpExecutionCredentials::from_source_secret(
        "cookie-namespace:source-version".to_string(),
        Some(
            serde_json::to_vec(&BTreeMap::from([
                (
                    "Authorization".to_string(),
                    "Bearer execution-secret".to_string(),
                ),
                ("Cookie".to_string(), "session=private".to_string()),
                ("X-Api-Key".to_string(), "source-only-key".to_string()),
            ]))
            .expect("serialize source secret"),
        ),
    );

    let capture = HttpEffectAdapter::new_test()
        .execute_http(request, CancellationHandle::new().token())
        .await
        .expect("live HTTP effect 应注入 source secret headers");
    let EffectOutput::Http(response) = &capture.output else {
        panic!("credentialed HTTP effect 必须返回 HTTP 输出");
    };
    assert_eq!(response.body, b"credentialed");
}

#[tokio::test]
async fn plan_http_effect_marks_request_body_secret_and_witness_redacted() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/submit"))
        .respond_with(ResponseTemplate::new(201).set_body_string("saved"))
        .mount(&server)
        .await;
    let secret_body = "token=plain-request-secret";
    let mut request = http_effect_request(format!("{}/submit?private=query", server.uri()));
    request.spec.method = lj_rule_model::HttpMethod::Post;
    request.spec.body = Some(secret_body.to_string());
    let execution_id = request.execution_id;
    let node_id = request.node_id;
    let effect_id = request.effect_id;
    let captured = HttpEffectAdapter::new_test()
        .execute_http(request, CancellationHandle::new().token())
        .await
        .expect("POST effect 应产生 capture");
    let capture = EffectCapture::from_live(
        execution_id,
        effect_id,
        node_id,
        "request-body-contract".to_string(),
        captured,
    )
    .expect("runtime 必须保留 secret archive material");

    assert_eq!(capture.request_body(), Some(secret_body.as_bytes()));
    assert_eq!(
        capture.request_body_sensitivity(),
        Some(EffectCaptureMaterialSensitivity::Secret)
    );
    let EffectWitness::Http(witness) = &capture.witness else {
        panic!("HTTP capture 必须携带 HTTP witness");
    };
    let Some(body) = &witness.request.body else {
        panic!("POST request body 必须写入安全摘要");
    };
    assert_eq!(body.hash, effect_bytes_hash(secret_body.as_bytes()));
    assert_eq!(
        body.byte_len,
        u64::try_from(secret_body.len()).expect("body len fits u64")
    );
    let witness_json = serde_json::to_string(&capture.witness).expect("serialize safe witness");
    assert!(!witness_json.contains(secret_body));
    assert!(!witness_json.contains("private=query"));
    capture
        .validate_replay_integrity()
        .expect("secret request body 仍必须与 hash witness 绑定");
}

#[tokio::test]
async fn plan_http_effect_records_real_manual_redirect_hop() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/start"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "/final"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/final"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(20))
                .set_body_string("redirected"),
        )
        .mount(&server)
        .await;

    let capture = HttpEffectAdapter::new_test()
        .execute_http(
            http_effect_request(format!("{}/start?private=query", server.uri())),
            CancellationHandle::new().token(),
        )
        .await
        .expect("manual redirect 应返回 capture");
    let EffectWitness::Http(witness) = &capture.witness else {
        panic!("HTTP effect 必须生成 HTTP witness");
    };
    assert_eq!(witness.redirects.len(), 1);
    assert_eq!(witness.redirects[0].status, 302);
    assert!(!witness.redirects[0].from_url.contains('?'));
    assert!(!witness.redirects[0].to_url.contains('?'));
    assert_eq!(witness.dns_targets.len(), 2);
    assert!(witness.dns_targets.iter().all(|target| {
        target.kind == HttpDnsTargetKind::IpLiteral
            && target.addresses == vec!["127.0.0.1".to_string()]
    }));
    assert!(
        witness.duration_ms >= 10,
        "witness 必须记录实际 HTTP/redirect 耗时"
    );
    capture
        .validate()
        .expect("实际 redirect hop 必须通过安全 witness 校验");
}

#[tokio::test]
async fn plan_http_effect_archives_typed_request_failure() {
    let mut request = http_effect_request("https://example.invalid/never".to_string());
    request
        .spec
        .headers
        .insert("invalid\nheader".to_string(), "value".to_string());
    let capture = HttpEffectAdapter::new_test()
        .execute_http(request, CancellationHandle::new().token())
        .await
        .expect("已开始的 HTTP 失败必须产生可 archive output");
    assert_eq!(
        capture.output,
        EffectOutput::Failure(lj_runtime::EffectFailure::Http {
            error: HttpEffectErrorKind::Request,
        })
    );
    let EffectWitness::Http(witness) = &capture.witness else {
        panic!("失败 HTTP effect 必须生成 HTTP witness");
    };
    assert_eq!(witness.error, Some(HttpEffectErrorKind::Request));
    capture
        .validate()
        .expect("HTTP failure output 必须与失败 witness 绑定");
}

#[tokio::test]
async fn plan_http_effect_cancellation_aborts_pending_request() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/slow"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_secs(2))
                .set_body_string("too late"),
        )
        .mount(&server)
        .await;
    let cancellation = CancellationHandle::new();
    let processor = Arc::new(HttpEffectAdapter::new_test());
    let task_processor = processor.clone();
    let task_cancellation = cancellation.token();
    let task = tokio::spawn(async move {
        task_processor
            .execute_http(
                http_effect_request(format!("{}/slow", server.uri())),
                task_cancellation,
            )
            .await
    });

    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(cancellation.cancel());
    let result = tokio::time::timeout(Duration::from_millis(500), task)
        .await
        .expect("取消必须中止等待中的 HTTP future")
        .expect("HTTP task 不应 panic");
    let Err(error) = result else {
        panic!("应被取消");
    };
    assert_eq!(error.code, EffectErrorCode::Cancelled);
}

fn http_effect_request(url: String) -> HttpEffectRequest {
    HttpEffectRequest {
        execution_id: Uuid::new_v4(),
        source_id: "http-effect-test".to_string(),
        node_id: Uuid::new_v4(),
        effect_id: Uuid::new_v4(),
        trace_id: "http-effect-trace".to_string(),
        spec: lj_rule_model::HttpSpec {
            method: lj_rule_model::HttpMethod::Get,
            url,
            headers: HashMap::new(),
            body: None,
            charset: None,
            expected_type: lj_rule_model::ExpectedDataType::Html,
        },
        input: EffectInput::Intent(IntentInput::Query("typed".to_string())),
        capabilities: PolicyCapabilities {
            network: true,
            ..PolicyCapabilities::default()
        },
        base_url: String::new(),
        credentials: HttpExecutionCredentials::default(),
    }
}
