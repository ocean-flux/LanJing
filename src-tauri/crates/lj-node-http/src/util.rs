//! HTTP 节点处理器工具函数 — URL 模板渲染、URL 编码、字符集解析。

use std::collections::HashMap;
use std::hash::BuildHasher;

/// 渲染 URL 模板,替换 `{{key}}`/`{{page}}`/`{{bookUrl}}`/`{{chapterUrl}}`/`{{vod_id}}`/`{{type}}` 变量。
///
/// - `{{key}}` → URL 编码后的搜索关键词
/// - `{{page}}` → 页码字符串
/// - `{{bookUrl}}` → 书籍 URL(Detail/Toc 段,路径位置不编码)
/// - `{{chapterUrl}}` → 章节 URL(Content 段,路径位置不编码)
/// - `{{vod_id}}` → URL 编码后的视频 `vod_id`(视频 Detail 段,KTD1;不可信提取,query 值需编码防注入)
/// - `{{type}}` → Maccms 分类 ID(Discover 段,未填替换为空走全站最新流,R10)
#[must_use]
pub fn render_url_template(
    template: &str,
    key: Option<&str>,
    page: Option<u32>,
    book_url: Option<&str>,
    chapter_url: Option<&str>,
    vod_id: Option<&str>,
    type_: Option<&str>,
) -> String {
    let mut url = template.to_string();
    if let Some(k) = key {
        url = url.replace("{{key}}", &url_encode(k));
    }
    if let Some(p) = page {
        url = url.replace("{{page}}", &p.to_string());
    }
    // bookUrl/chapterUrl 假定为路径替换位置(如 /book/{{bookUrl}}),
    // 不适用于 query string 位置(如 ?url={{bookUrl}}),后者需 URL 编码。
    // Legado 规则中 bookUrl/chapterUrl 几乎总是路径替换,首刀场景 OK。
    // Encoding them would break scheme/host/path separators.
    if let Some(bu) = book_url {
        url = url.replace("{{bookUrl}}", bu);
    }
    if let Some(cu) = chapter_url {
        url = url.replace("{{chapterUrl}}", cu);
    }
    // vod_id 来自不可信第三方提取,query 值位置需编码防注入(#8)。
    if let Some(v) = vod_id {
        url = url.replace("{{vod_id}}", &url_encode(v));
    }
    // type 未填替换为空(Maccms ?t= 走全站最新流,R10)。
    url = url.replace("{{type}}", type_.unwrap_or(""));
    url
}

/// URL 编码(保留字母数字及 `-` `_` `.` `~`,其余百分号编码)。
#[must_use]
pub fn url_encode(s: &str) -> String {
    let mut encoded = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            b' ' => encoded.push_str("%20"),
            _ => {
                use std::fmt::Write;
                let _ = write!(encoded, "%{b:02X}");
            }
        }
    }
    encoded
}

/// 从 Content-Type 响应头解析字符集。
#[must_use]
pub fn parse_charset<S: BuildHasher>(headers: &HashMap<String, String, S>) -> Option<String> {
    headers.get("content-type").and_then(|ct| {
        ct.split(';').nth(1).map(str::trim).and_then(|part| {
            part.strip_prefix("charset=")
                .or_else(|| part.strip_prefix("charset ="))
                .map(|s| s.trim().to_string())
        })
    })
}
