//! lj-node-http 集成测试。
//!
//! 覆盖 SSRF 校验、URL 模板渲染、URL 编码、字符集解析、`convert_response` 等核心路径。

use std::collections::HashMap;
use std::net::IpAddr;

use futures::StreamExt;

use lj_node_http::processor::{HttpNodeProcessor, convert_response};
use lj_node_http::ssrf::is_blocked_ip;
use lj_node_http::util::{parse_charset, render_url_template};
use lj_rule_model::Error;
use lj_rule_model::PolicyCapabilities;
use lj_runtime::NodeDataVariant;
use lj_runtime::NodeKind;
use lj_runtime::{ExecutionContext, NodeProcessor};

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

// ── HttpNodeProcessor 基本 ─────────────────────────────

#[test]
fn processor_kind_and_types() {
    let processor = HttpNodeProcessor::new();
    assert_eq!(processor.kind(), NodeKind::Http);
    assert_eq!(processor.input_type(), None);
    assert_eq!(processor.output_type(), NodeDataVariant::HttpResponse);
}

#[test]
fn processor_empty_spec_returns_empty_stream() {
    let rt = tokio::runtime::Runtime::new().expect("创建 tokio runtime");
    rt.block_on(async {
        let processor = HttpNodeProcessor::new();
        let ctx = ExecutionContext {
            cookies: HashMap::new(),
            caps: PolicyCapabilities::default(),
            trace_id: "test".into(),
            base_url: String::new(),
        };
        let spec = lj_runtime::NodeSpec {
            kind: NodeKind::Http,
            http: None,
            js: None,
            extract: None,
            mapper: None,
        };
        let input: futures::stream::BoxStream<'static, lj_runtime::NodeData> =
            futures::stream::empty().boxed();
        let mut output = processor.process(&ctx, &spec, input);
        let result = output.next().await;
        assert!(result.is_none(), "空 spec 应返回空 stream");
    });
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
