//! 标准意图契约层。
//!
//! 只描述调用入口与输入形状，不携带来源端点或展示语义。

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 规则对调用方暴露的标准意图。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum StandardIntent {
    /// 搜索媒体资源。
    Search,
    /// 发现首页、分类、榜单、频道等资源集合。
    Discover,
    /// 补全媒体主体详情。
    ResolveItem,
    /// 列出章节、集、曲目、页等可消费单元。
    ListUnits,
    /// 获取正文、图片、流、字幕等资产。
    ResolveAsset,
    /// 执行上一轮返回的可继续动作。
    ContinueAction,
}

/// 标准意图入口声明。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentExport {
    /// Flow 入口节点 ID。
    pub flow_entry: Uuid,
    /// Mapper 输出节点 ID。
    pub mapper_output: Uuid,
}

impl IntentExport {
    /// 创建意图导出。
    #[must_use]
    pub const fn new(flow_entry: Uuid, mapper_output: Uuid) -> Self {
        Self {
            flow_entry,
            mapper_output,
        }
    }
}

/// 标准意图输入。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum IntentInput {
    /// 搜索关键词。
    Query(String),
    /// 媒体主体 ID。
    ItemId(String),
    /// 媒体单元 ID。
    UnitId(String),
    /// 动作 ID。
    ActionId(String),
    /// 继续动作的透明载荷。
    Opaque(serde_json::Value),
    /// 分页游标。
    Page(String),
    /// 无输入。
    None,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_intents_roundtrip() {
        let variants = [
            StandardIntent::Search,
            StandardIntent::Discover,
            StandardIntent::ResolveItem,
            StandardIntent::ListUnits,
            StandardIntent::ResolveAsset,
            StandardIntent::ContinueAction,
        ];
        for intent in variants {
            let json = serde_json::to_string(&intent).unwrap();
            let back: StandardIntent = serde_json::from_str(&json).unwrap();
            assert_eq!(intent, back);
        }
    }

    #[test]
    fn intent_export_roundtrip() {
        let export = IntentExport::new(Uuid::new_v4(), Uuid::new_v4());
        let json = serde_json::to_string(&export).unwrap();
        let back: IntentExport = serde_json::from_str(&json).unwrap();
        assert_eq!(export, back);
    }

    #[test]
    fn intent_input_roundtrip() {
        let inputs = [
            IntentInput::Query("测试".to_string()),
            IntentInput::ItemId("item:1".to_string()),
            IntentInput::UnitId("unit:1".to_string()),
            IntentInput::ActionId("action:1".to_string()),
            IntentInput::Opaque(serde_json::json!({"cursor":"2"})),
            IntentInput::Page("2".to_string()),
            IntentInput::None,
        ];
        for input in inputs {
            let json = serde_json::to_string(&input).unwrap();
            let back: IntentInput = serde_json::from_str(&json).unwrap();
            assert_eq!(input, back);
        }
    }
}
