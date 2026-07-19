//! Extract Plan effect adapter crate。
//!
//! 支持 HTML、XML、JSON、正则和编码转换的受控内容提取。

pub mod error;
pub mod html;
pub mod html_css;
pub mod html_xpath;
pub mod json;
pub mod play_url;
pub mod processor;
pub mod regex_extract;
pub mod xml;

pub use lj_rule_model::FieldRules;
