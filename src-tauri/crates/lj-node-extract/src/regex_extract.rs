//! 正则提取 — regex 引擎 + `##regex##replacement` 清理。

use std::collections::HashMap;

use lj_core::extract_rule::RegexClean;
use regex::Regex;

/// 正则预编译缓存(全局共享，避免重复编译)。
pub type RegexCache = HashMap<String, Regex, std::collections::hash_map::RandomState>;

/// 应用正则清理(`##pattern##replacement`)。
///
/// 使用 `regex_cache`(预编译缓存)避免重复编译正则。
/// 缓存中不存在时返回原文(不 panic)。
#[must_use]
pub fn apply_regex_clean(text: &str, clean: &RegexClean, regex_cache: &RegexCache) -> String {
    match regex_cache.get(&clean.pattern) {
        Some(re) => re.replace_all(text, clean.replacement.as_str()).to_string(),
        None => text.to_string(),
    }
}

/// 正则提取(`ExtractRule::Regex`)。
///
/// 使用 `regex_cache`(预编译缓存)避免重复编译。
///
/// # Errors
///
/// 返回 `InvalidRegex` 当正则表达式未在缓存中，`NoMatch` 当未匹配或组不存在。
pub fn extract_regex(
    text: &str,
    pattern: &str,
    group: usize,
    regex_cache: &RegexCache,
) -> Result<String, crate::error::ExtractError> {
    let re = regex_cache.get(pattern).ok_or_else(|| {
        crate::error::ExtractError::InvalidRegex(format!("正则 '{pattern}' 未预编译"))
    })?;
    match re.captures(text) {
        Some(caps) => caps
            .get(group)
            .map(|m| m.as_str().to_string())
            .ok_or_else(|| crate::error::ExtractError::NoMatch("正则匹配组不存在".to_string())),
        None => Err(crate::error::ExtractError::NoMatch(
            "正则未匹配".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cache() -> HashMap<String, Regex> {
        let mut m = HashMap::new();
        m.insert(r"\|.*".to_string(), Regex::new(r"\|.*").unwrap());
        m.insert(r"(\d+)".to_string(), Regex::new(r"(\d+)").unwrap());
        m
    }

    #[test]
    fn test_apply_regex_clean() {
        let cache = make_cache();
        let clean = RegexClean {
            pattern: r"\|.*".to_string(),
            replacement: String::new(),
        };
        assert_eq!(
            apply_regex_clean("作者: 张三|其他信息", &clean, &cache),
            "作者: 张三"
        );
    }

    #[test]
    fn test_apply_regex_clean_invalid_pattern() {
        let cache = make_cache();
        let clean = RegexClean {
            pattern: r"[[invalid".to_string(),
            replacement: String::new(),
        };
        // 非法 pattern 不入缓存 → 返回原文
        assert_eq!(apply_regex_clean("hello", &clean, &cache), "hello");
    }

    #[test]
    fn test_extract_regex() {
        let cache = make_cache();
        let result = extract_regex("abc123def", r"(\d+)", 1, &cache).unwrap();
        assert_eq!(result, "123");
    }

    #[test]
    fn test_extract_regex_no_match() {
        let cache = make_cache();
        let result = extract_regex("abcdef", r"(\d+)", 1, &cache);
        assert!(result.is_err());
    }
}
