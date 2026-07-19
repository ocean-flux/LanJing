//! 集成测试：HTML 提取、回退链、charset 解码、正则清理。

use std::collections::HashMap;
use std::sync::Arc;

use lj_node_extract::html::{extract_elements_from_doc, extract_from_doc, parse_html};
use lj_node_extract::html_css::try_extract_on_doc;
use lj_node_extract::processor::ExtractEffectAdapter;
use lj_node_extract::processor::decode_body;
use lj_node_extract::regex_extract::{RegexCache, apply_regex_clean};
use lj_rule_model::{
    ExpectedDataType, ExtractRule, ExtractSpec, ExtractType, OutputTarget, RegexClean,
};
use lj_runtime::{
    CancellationHandle, EffectInput, EffectOutput, ExtractEffectHandler, ExtractEffectRequest,
    HttpResponse,
};
use regex::Regex;
use uuid::Uuid;

/// HTML + CSS 选择器提取文本。
#[test]
fn test_css_text() {
    let html = r"<h2 itemprop='name'>修罗武神</h2>";
    let doc = parse_html(html);
    let cache = RegexCache::new();
    assert_eq!(
        extract_from_doc(
            &doc,
            "h2[itemprop='name']",
            &ExtractType::Text,
            None,
            &cache
        )
        .unwrap(),
        "修罗武神"
    );
}

/// HTML + CSS 选择器提取 href 属性。
#[test]
fn test_css_href() {
    let html = r"<a itemprop='url' href='/book/123'>链接</a>";
    let doc = parse_html(html);
    let cache = RegexCache::new();
    assert_eq!(
        extract_from_doc(&doc, "a[itemprop='url']", &ExtractType::Href, None, &cache).unwrap(),
        "/book/123"
    );
}

/// HTML + CSS 选择器提取 src 属性。
#[test]
fn test_css_src() {
    let html = r"<img itemprop='image' src='/cover/123.jpg' />";
    let doc = parse_html(html);
    let cache = RegexCache::new();
    assert_eq!(
        extract_from_doc(
            &doc,
            "img[itemprop='image']",
            &ExtractType::Src,
            None,
            &cache
        )
        .unwrap(),
        "/cover/123.jpg"
    );
}

/// HTML + @html 提取元素内部 HTML。
#[test]
fn test_css_html() {
    let html = r"<div id='article-content'><p>正文内容</p></div>";
    let doc = parse_html(html);
    let cache = RegexCache::new();
    let result =
        extract_from_doc(&doc, "#article-content", &ExtractType::Html, None, &cache).unwrap();
    assert!(result.contains("正文内容"));
}

/// `regex_clean` 去掉无关信息。
#[test]
fn test_regex_clean() {
    let text = "作者: 张三|其他信息";
    let clean = RegexClean {
        pattern: r"\|.*".to_string(),
        replacement: String::new(),
    };
    let mut cache = RegexCache::new();
    cache.insert(r"\|.*".to_string(), Regex::new(r"\|.*").unwrap());
    assert_eq!(apply_regex_clean(text, &clean, &cache), "作者: 张三");
}

/// || 回退链(第一个非空胜出)。
#[test]
fn test_fallback_chain() {
    let html = r"<img src='/fallback.jpg' />";
    let doc = parse_html(html);
    let rules = vec![
        ExtractRule::CssSelector {
            selector: "figure img".to_string(),
            extract_type: ExtractType::Src,
            regex_clean: None,
        },
        ExtractRule::CssSelector {
            selector: "img".to_string(),
            extract_type: ExtractType::Src,
            regex_clean: None,
        },
    ];
    let cache = RegexCache::new();
    let result = try_extract_on_doc(&doc, &rules, &cache).unwrap();
    assert_eq!(result, "/fallback.jpg");
}

/// charset 解码 GBK。
#[test]
fn test_charset_gbk() {
    // "主人" in GBK
    let gbk_bytes = [0xD6, 0xF7, 0xC8, 0xCB];
    assert_eq!(decode_body(&gbk_bytes, Some("gbk")), "主人");
}

/// `:nth-child()` 伪类。
#[test]
fn test_nth_child() {
    let html = r"<ol><li>1</li><li>2</li><li><a>玄幻</a></li></ol>";
    let doc = parse_html(html);
    let cache = RegexCache::new();
    assert_eq!(
        extract_from_doc(
            &doc,
            "ol li:nth-child(3) a",
            &ExtractType::Text,
            None,
            &cache
        )
        .unwrap(),
        "玄幻"
    );
}

/// `:contains()` 伪类。
#[test]
fn test_contains_pseudo() {
    let html = r"<span>10万字</span>";
    let doc = parse_html(html);
    let cache = RegexCache::new();
    assert_eq!(
        extract_from_doc(
            &doc,
            "span:contains('万字')",
            &ExtractType::Text,
            None,
            &cache
        )
        .unwrap(),
        "10万字"
    );
}

/// `extract_elements_from_doc` 返回 `ElementRef` 列表(避免 reparse)。
#[test]
fn test_html_list() {
    let html = r#"<ul><li class="book">修罗武神</li><li class="book">凡人修仙传</li></ul>"#;
    let doc = parse_html(html);
    let list = extract_elements_from_doc(&doc, "li.book").unwrap();
    assert_eq!(list.len(), 2);
    let text0: String = list[0].text().collect();
    let text1: String = list[1].text().collect();
    assert!(text0.contains("修罗武神"));
    assert!(text1.contains("凡人修仙传"));
}

#[tokio::test]
async fn plan_extract_effect_borrows_typed_http_output() {
    let processor = ExtractEffectAdapter;
    let response = Arc::new(EffectOutput::Http(HttpResponse {
        status: 200,
        headers: HashMap::new(),
        body: "<h1>类型化提取</h1>".as_bytes().to_vec(),
        charset: Some("utf-8".to_string()),
    }));
    let capture = processor
        .execute_extract(
            ExtractEffectRequest {
                execution_id: Uuid::new_v4(),
                source_id: "extract-effect-test".to_string(),
                node_id: Uuid::new_v4(),
                effect_id: Uuid::new_v4(),
                trace_id: "extract-effect-trace".to_string(),
                spec: ExtractSpec {
                    rules: vec![ExtractRule::CssSelector {
                        selector: "h1".to_string(),
                        extract_type: ExtractType::Text,
                        regex_clean: None,
                    }],
                    field_rules: HashMap::new(),
                    expected_type: ExpectedDataType::Html,
                    output_target: OutputTarget::default(),
                },
                input: EffectInput::Output(response),
                base_url: "https://example.invalid".to_string(),
            },
            CancellationHandle::new().token(),
        )
        .await
        .expect("Extract effect 应返回类型化记录");

    let EffectOutput::Extract(output) = &capture.output else {
        panic!("Extract effect 必须返回 Extract 类型化输出");
    };
    assert_eq!(output.records.len(), 1);
    assert_eq!(output.records[0]["title"], "类型化提取");
    capture
        .validate()
        .expect("Extract output 必须与输入 hash witness 绑定");
}
