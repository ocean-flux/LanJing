//! 图 schema — 端点子图模板 + 条件分支(ADR-0023, ADR-0026)。

use serde::{Deserialize, Serialize};

use crate::endpoint::EndpointKind;
use crate::node::NodeKind;
use crate::node_data::NodeDataVariant;

/// 端点子图模板(描述合法子图形态)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndpointTemplate {
    /// 端点类型。
    pub kind: EndpointKind,
    /// 合法节点序列(如 `[Http, Extract]`)。
    pub node_sequence: Vec<NodeKind>,
    /// 入边数据类型。
    pub input_type: Option<NodeDataVariant>,
    /// 出边数据类型。
    pub output_type: NodeDataVariant,
}

/// 图 schema(含所有端点模板，用于导入验证)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphSchema {
    /// 端点模板列表。
    pub templates: Vec<EndpointTemplate>,
}

impl GraphSchema {
    /// 默认 schema(5 端点模板)。
    #[must_use]
    pub fn default_schema() -> Self {
        Self {
            templates: vec![
                EndpointTemplate {
                    kind: EndpointKind::Search,
                    node_sequence: vec![NodeKind::Http, NodeKind::Extract],
                    input_type: None,
                    output_type: NodeDataVariant::Media,
                },
                EndpointTemplate {
                    kind: EndpointKind::Discover,
                    node_sequence: vec![NodeKind::Js, NodeKind::Http, NodeKind::Extract],
                    input_type: None,
                    output_type: NodeDataVariant::Media,
                },
                EndpointTemplate {
                    kind: EndpointKind::Detail,
                    node_sequence: vec![NodeKind::Http, NodeKind::Extract],
                    input_type: Some(NodeDataVariant::Media),
                    output_type: NodeDataVariant::Media,
                },
                EndpointTemplate {
                    kind: EndpointKind::Toc,
                    node_sequence: vec![NodeKind::Http, NodeKind::Extract],
                    input_type: Some(NodeDataVariant::Media),
                    output_type: NodeDataVariant::Media,
                },
                EndpointTemplate {
                    kind: EndpointKind::Content,
                    node_sequence: vec![NodeKind::Http, NodeKind::Extract],
                    input_type: Some(NodeDataVariant::Media),
                    output_type: NodeDataVariant::Media,
                },
            ],
        }
    }
}

/// 条件分支标签(Condition 节点出边,ADR-0026)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionBranch {
    /// if 条件为真。
    True,
    /// if 条件为假。
    False,
    /// match 分支(按值路由)。
    Case(String),
}
