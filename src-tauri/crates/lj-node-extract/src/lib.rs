//! 提取节点 crate。
//!
//! 实现多种内容提取处理器：HTML（`scraper`）、XML（`xmloxide`）、
//! JSON（`jsonpath-rust`）、正则表达式，以及编码转换支持。

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
