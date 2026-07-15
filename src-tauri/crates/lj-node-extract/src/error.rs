//! 提取节点错误类型。

use thiserror::Error;

/// 提取节点错误。
#[derive(Debug, Error)]
pub enum ExtractError {
    /// CSS 选择器解析失败。
    #[error("CSS 选择器解析失败: {0}")]
    SelectorParse(String),

    /// 正则表达式错误。
    #[error("正则表达式错误: {0}")]
    InvalidRegex(String),

    /// 未匹配。
    #[error("未匹配: {0}")]
    NoMatch(String),

    /// 不支持的数据格式。
    #[error("不支持的数据格式: {0}")]
    UnsupportedFormat(String),

    /// charset 解码失败。
    #[error("charset 解码失败: {0}")]
    DecodeError(String),
}
