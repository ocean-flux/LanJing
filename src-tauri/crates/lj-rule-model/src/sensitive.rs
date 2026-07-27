//! 跨来源共享的敏感名称策略。
//!
//! Legado、Maccms 与 HTTP witness 必须调用这里的无状态策略；来源 adapter 不得复制
//! Authorization/Cookie/token 等名称表。名称比较大小写不敏感，并把 `_` 与 `-` 视为同一
//! 分隔符。策略只分类名称，不持有或记录对应值。

use serde::{Deserialize, Serialize};

/// 一个 request header 在 credential 边界的处理方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestHeaderDisposition {
    /// 可公开进入 `HttpSpec` 与安全 witness 摘要。
    Public,
    /// 必须从作者文档/Definition 分离，只能在 live request material 中注入。
    Credential,
    /// 即使作为 credential 也不能注入 request。
    Blocked,
}

/// 共享敏感名称策略的唯一 owner。
pub struct SensitiveNamePolicy;

impl SensitiveNamePolicy {
    /// 分类 request header。
    ///
    /// `Proxy-Authorization` 会改变代理 hop 的授权边界，`Set-Cookie` 只属于 response，二者
    /// 均返回 [`RequestHeaderDisposition::Blocked`]。其余敏感名称返回 `Credential`。
    #[must_use]
    pub fn request_header_disposition(name: &str) -> RequestHeaderDisposition {
        let name = name.trim();
        if normalized_eq(name, "proxy-authorization") || normalized_eq(name, "set-cookie") {
            return RequestHeaderDisposition::Blocked;
        }
        if normalized_eq(name, "authorization")
            || normalized_eq(name, "cookie")
            || normalized_contains(name, "token")
            || normalized_contains(name, "secret")
            || normalized_contains(name, "api-key")
            || normalized_contains(name, "apikey")
        {
            return RequestHeaderDisposition::Credential;
        }
        RequestHeaderDisposition::Public
    }

    /// 判断名称是否在 request/response、日志和 witness 中具有敏感语义。
    #[must_use]
    pub fn is_sensitive(name: &str) -> bool {
        Self::request_header_disposition(name) != RequestHeaderDisposition::Public
    }

    /// 判断 response header 是否必须留在受保护 material，不能进入安全 witness。
    #[must_use]
    pub fn is_sensitive_response_header(name: &str) -> bool {
        Self::is_sensitive(name)
    }

    /// 判断 URL/query template 是否使用敏感参数名。只解析参数名，不保留或返回 value。
    #[must_use]
    pub fn url_contains_sensitive_query_name(value: &str) -> bool {
        let Some((_, query_and_fragment)) = value.split_once('?') else {
            return false;
        };
        let query = query_and_fragment
            .split_once('#')
            .map_or(query_and_fragment, |(query, _)| query);
        query.split('&').any(|part| {
            let name = part.split_once('=').map_or(part, |(name, _)| name);
            Self::is_sensitive(&percent_decode_ascii(name))
        })
    }

    /// 比较两个 header/字段名是否在大小写与 `_`/`-` 归一化后相同。
    #[must_use]
    pub fn equivalent(left: &str, right: &str) -> bool {
        let left = left.trim().as_bytes();
        let right = right.trim().as_bytes();
        left.len() == right.len()
            && left
                .iter()
                .zip(right)
                .all(|(&left, &right)| normalize_byte(left) == normalize_byte(right))
    }
}

fn normalized_eq(value: &str, expected: &str) -> bool {
    let value = value.as_bytes();
    let expected = expected.as_bytes();
    value.len() == expected.len()
        && value
            .iter()
            .zip(expected)
            .all(|(&value, &expected)| normalize_byte(value) == expected)
}

fn normalized_contains(value: &str, needle: &str) -> bool {
    let value = value.as_bytes();
    let needle = needle.as_bytes();
    if needle.len() > value.len() {
        return false;
    }
    value.windows(needle.len()).any(|window| {
        window
            .iter()
            .zip(needle)
            .all(|(&value, &needle)| normalize_byte(value) == needle)
    })
}

fn percent_decode_ascii(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = String::with_capacity(value.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
        {
            let byte = high * 16 + low;
            if byte.is_ascii() {
                decoded.push(char::from(byte));
                index += 3;
                continue;
            }
        }
        let character = value[index..]
            .chars()
            .next()
            .expect("index remains on a UTF-8 boundary");
        decoded.push(character);
        index += character.len_utf8();
    }
    decoded
}

const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

const fn normalize_byte(byte: u8) -> u8 {
    match byte {
        b'A'..=b'Z' => byte + (b'a' - b'A'),
        b'_' => b'-',
        _ => byte,
    }
}

#[cfg(test)]
mod tests {
    use super::{RequestHeaderDisposition, SensitiveNamePolicy};

    #[test]
    fn shared_policy_normalizes_case_and_separator_without_widening_request_injection() {
        for name in [
            "Authorization",
            "Cookie",
            "X_TOKEN",
            "client-secret",
            "X_API_KEY",
        ] {
            assert_eq!(
                SensitiveNamePolicy::request_header_disposition(name),
                RequestHeaderDisposition::Credential
            );
        }
        for name in ["Proxy-Authorization", "proxy_authorization", "Set-Cookie"] {
            assert_eq!(
                SensitiveNamePolicy::request_header_disposition(name),
                RequestHeaderDisposition::Blocked
            );
        }
        assert_eq!(
            SensitiveNamePolicy::request_header_disposition("User-Agent"),
            RequestHeaderDisposition::Public
        );
        assert!(SensitiveNamePolicy::equivalent("X_API_KEY", "x-api-key"));
        assert_eq!(
            SensitiveNamePolicy::request_header_disposition("X-ApiKey"),
            RequestHeaderDisposition::Credential
        );
        assert!(SensitiveNamePolicy::url_contains_sensitive_query_name(
            "https://example.test/search?%61pi_key=secret"
        ));
        assert!(!SensitiveNamePolicy::url_contains_sensitive_query_name(
            "/search?q={{key}}&page={{page}}"
        ));
    }
}
