//! Html+`XPath` 提取集成测试。
//!
//! 测试 `html_xpath` 模块在 xmloxide html5 解析后的 `XPath` 提取。

use std::collections::HashMap;

use lj_core::extract_rule::{ExtractRule, ExtractType, RegexClean};
use lj_node_extract::html_xpath;
use lj_node_extract::regex_extract::RegexCache;

/// Html + `XPath` 提取标题文本。
#[test]
fn test_html_xpath_single_title() {
    let html = "<html><head><title>测试标题</title></head><body><p>内容</p></body></html>";
    let rules = vec![ExtractRule::XPath {
        expression: "//title/text()".to_string(),
        extract_type: ExtractType::Text,
        regex_clean: None,
    }];
    let cache = RegexCache::new();
    let result = html_xpath::extract_single(html, &rules, &cache);
    assert_eq!(result.len(), 1);
    match &result[0] {
        lj_core::node_data::NodeData::Media(lj_core::media::Media::Book(b)) => {
            assert_eq!(b.title, "测试标题");
        }
        other => panic!("期望 BookMedia, 得到 {other:?}"),
    }
}

/// Html + `XPath` 提取属性值。
#[test]
fn test_html_xpath_attr_href() {
    let html = r"<div class='x'><a href='/book/123'>link</a></div>";
    let rules = vec![ExtractRule::XPath {
        expression: "//div[@class='x']/a/@href".to_string(),
        extract_type: ExtractType::Text,
        regex_clean: None,
    }];
    let cache = RegexCache::new();
    let result = html_xpath::extract_single(html, &rules, &cache);
    match &result[0] {
        lj_core::node_data::NodeData::Media(lj_core::media::Media::Book(b)) => {
            assert!(b.title.contains("/book/123"));
        }
        other => panic!("期望 BookMedia, 得到 {other:?}"),
    }
}

/// 列表模式：Html + `XPath` 逐 item 逐字段提取。
#[test]
fn test_html_xpath_list() {
    let html = r"
        <ul>
            <li><span class='t'>书1</span><span class='a'>作者1</span></li>
            <li><span class='t'>书2</span><span class='a'>作者2</span></li>
        </ul>";
    let rules = vec![ExtractRule::XPath {
        expression: "//li".to_string(),
        extract_type: ExtractType::Text,
        regex_clean: None,
    }];
    let mut field_rules = HashMap::new();
    field_rules.insert(
        "name".to_string(),
        vec![ExtractRule::XPath {
            expression: "span[@class='t']/text()".to_string(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        }],
    );
    field_rules.insert(
        "author".to_string(),
        vec![ExtractRule::XPath {
            expression: "span[@class='a']/text()".to_string(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        }],
    );
    let cache = RegexCache::new();
    let result = html_xpath::extract_list(html, &rules, &field_rules, &cache, "");
    assert_eq!(result.len(), 2);
    match &result[0] {
        lj_core::node_data::NodeData::Media(lj_core::media::Media::Book(b)) => {
            assert_eq!(b.title, "书1");
            assert_eq!(b.author.as_deref(), Some("作者1"));
        }
        other => panic!("期望 BookMedia, 得到 {other:?}"),
    }
    match &result[1] {
        lj_core::node_data::NodeData::Media(lj_core::media::Media::Book(b)) => {
            assert_eq!(b.title, "书2");
            assert_eq!(b.author.as_deref(), Some("作者2"));
        }
        other => panic!("期望 BookMedia, 得到 {other:?}"),
    }
}

/// 空 HTML 不应 panic，返回 Error。
#[test]
fn test_html_xpath_empty_body() {
    let html = "";
    let rules = vec![ExtractRule::XPath {
        expression: "//title/text()".to_string(),
        extract_type: ExtractType::Text,
        regex_clean: None,
    }];
    let cache = RegexCache::new();
    let result = html_xpath::extract_single(html, &rules, &cache);
    assert_eq!(result.len(), 1);
    assert!(matches!(&result[0], lj_core::node_data::NodeData::Error(_)));
}

/// 无匹配时返回 Error。
#[test]
fn test_html_xpath_no_match() {
    let html = "<html><body><p>hello</p></body></html>";
    let rules = vec![ExtractRule::XPath {
        expression: "//title/text()".to_string(),
        extract_type: ExtractType::Text,
        regex_clean: None,
    }];
    let cache = RegexCache::new();
    let result = html_xpath::extract_single(html, &rules, &cache);
    assert!(matches!(&result[0], lj_core::node_data::NodeData::Error(_)));
}

/// 无效 `XPath` 表达式返回 Error，不 panic。
#[test]
fn test_html_xpath_invalid_expression() {
    let html = "<html><body><p>hello</p></body></html>";
    let rules = vec![ExtractRule::XPath {
        expression: "//[invalid".to_string(),
        extract_type: ExtractType::Text,
        regex_clean: None,
    }];
    let cache = RegexCache::new();
    let result = html_xpath::extract_single(html, &rules, &cache);
    assert!(matches!(&result[0], lj_core::node_data::NodeData::Error(_)));
}

/// `RegexClean` 后处理对 Html+`XPath` 生效。
#[test]
fn test_html_xpath_regex_clean() {
    let html = "<html><body><p>价格: 100元|其他信息</p></body></html>";
    let clean = RegexClean {
        pattern: r"\|.*".to_string(),
        replacement: String::new(),
    };
    let rules = vec![ExtractRule::XPath {
        expression: "//p/text()".to_string(),
        extract_type: ExtractType::Text,
        regex_clean: Some(clean),
    }];
    let mut cache = RegexCache::new();
    cache.insert(r"\|.*".to_string(), regex::Regex::new(r"\|.*").unwrap());
    let result = html_xpath::extract_single(html, &rules, &cache);
    match &result[0] {
        lj_core::node_data::NodeData::Media(lj_core::media::Media::Book(b)) => {
            assert_eq!(b.title, "价格: 100元");
        }
        other => panic!("期望 BookMedia, 得到 {other:?}"),
    }
}

/// Toc 列表模式：用 `XPath` 提取章节列表。
#[test]
fn test_html_xpath_toc() {
    let html = r"
        <div id='chapters'>
            <a href='/chapter/1.html'>第一章</a>
            <a href='/chapter/2.html'>第二章</a>
        </div>";
    let rules = vec![ExtractRule::XPath {
        expression: "//div[@id='chapters']/a".to_string(),
        extract_type: ExtractType::Text,
        regex_clean: None,
    }];
    let mut field_rules = HashMap::new();
    field_rules.insert(
        "chapterName".to_string(),
        vec![ExtractRule::XPath {
            expression: "text()".to_string(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        }],
    );
    field_rules.insert(
        "chapterUrl".to_string(),
        vec![ExtractRule::XPath {
            expression: "@href".to_string(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        }],
    );
    let cache = RegexCache::new();
    let result =
        html_xpath::extract_toc_list(html, &rules, &field_rules, &cache, "http://example.com");
    assert_eq!(result.len(), 1);
    match &result[0] {
        lj_core::node_data::NodeData::Media(lj_core::media::Media::Book(b)) => {
            assert_eq!(b.chapters.len(), 2);
            assert_eq!(b.chapters[0].title, "第一章");
            assert_eq!(
                b.chapters[0].chapter_url,
                "http://example.com/chapter/1.html"
            );
            assert_eq!(b.chapters[1].title, "第二章");
        }
        other => panic!("期望 BookMedia, 得到 {other:?}"),
    }
}
