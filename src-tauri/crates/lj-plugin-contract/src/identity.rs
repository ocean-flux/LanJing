//! namespaced stable identity 与版本词汇。

use std::fmt;

use crate::error::PluginError;

/// 一个 plugin 的稳定身份。
///
/// 形式为 `<namespace>.<name>`，例如 `lanjing.builtin`。identity 只允许小写 ASCII 字母、数字、
/// `-`、`_` 与作为分隔符的 `.`，且至少两段。大写与空白被拒绝，因此同一身份只有一种拼写，
/// 注册顺序或大小写差异不能制造两个 identity。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PluginId(String);

impl PluginId {
    /// 解析 plugin identity。
    ///
    /// # Errors
    ///
    /// 不是规范的 namespaced identity 时返回 [`PluginError::InvalidPluginId`]。
    pub fn parse(value: &str) -> Result<Self, PluginError> {
        validate_namespaced(value).map_err(|reason| {
            PluginError::InvalidPluginId(format!("{value:?} 不是合法的 plugin identity：{reason}"))
        })?;
        Ok(Self(value.to_string()))
    }

    /// 身份的规范文本。
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PluginId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// 一个 plugin 提供的 operation 的稳定身份。
///
/// 形式与 [`PluginId`] 相同，例如 `lanjing.effect.http`。它是 registry 的 lookup key，
/// 不依赖闭集 enum 变体、Rust 类型布局或注册顺序。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OperationId(String);

impl OperationId {
    /// 解析 operation identity。
    ///
    /// # Errors
    ///
    /// 不是规范的 namespaced identity 时返回 [`PluginError::InvalidOperationId`]。
    pub fn parse(value: &str) -> Result<Self, PluginError> {
        validate_namespaced(value).map_err(|reason| {
            PluginError::InvalidOperationId(format!(
                "{value:?} 不是合法的 operation identity：{reason}"
            ))
        })?;
        Ok(Self(value.to_string()))
    }

    /// 身份的规范文本。
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OperationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::borrow::Borrow<str> for OperationId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

/// 版本字符串，用于 plugin version 与 host contract version。
///
/// 只校验形状：非空、不含空白或控制字符、只含 `[0-9A-Za-z.+-]`。版本的语义比较与兼容性判定
/// 属于 manifest 校验，不在本类型里。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version(String);

impl Version {
    /// 解析版本字符串。
    ///
    /// # Errors
    ///
    /// 形状非法时返回 [`PluginError::InvalidVersion`]。
    pub fn parse(value: &str) -> Result<Self, PluginError> {
        let valid = !value.is_empty()
            && value.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '+')
            });
        if !valid {
            return Err(PluginError::InvalidVersion(format!(
                "{value:?} 不是合法的版本字符串"
            )));
        }
        Ok(Self(value.to_string()))
    }

    /// 版本的规范文本。
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// 校验 namespaced identity 的形状。
///
/// 空白、控制字符与大写字母都不在允许字符集内，因此只需检查字符集与段结构。
fn validate_namespaced(value: &str) -> Result<(), &'static str> {
    let segments: Vec<&str> = value.split('.').collect();
    if segments.len() < 2 {
        return Err("identity 必须包含 namespace 与 name 至少两段");
    }
    if segments.iter().any(|segment| segment.is_empty()) {
        return Err("identity 的段不能为空");
    }
    if !value.chars().all(|character| {
        character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || matches!(character, '-' | '_' | '.')
    }) {
        return Err("identity 只允许小写 ASCII 字母、数字、'-'、'_' 与 '.'");
    }
    Ok(())
}
