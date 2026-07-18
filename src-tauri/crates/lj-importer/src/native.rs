//! 本体规则导入器 — `LanJing` 节点图 JSON 直通反序列化及验证。

use lj_rule_model::{Error, PolicyCapabilities, SystemCapabilities};
use lj_runtime::Graph;

use crate::preview::ImportPreview;

/// 原生图导入器:直通 `Graph` + 标准意图契约验证。
pub struct NativeImporter;

impl NativeImporter {
    /// 直通: `Graph` 已是最终形态,只需验证 + 收集预览信息。
    ///
    /// # Errors
    ///
    /// 返回 `Error::GraphValidation` 当图结构不符合默认 schema。
    pub fn import(&self, graph: Graph) -> Result<ImportPreview, Error> {
        crate::validate::validate_graph(&graph)?;

        let node_count = graph.nodes.len();
        let edge_count = graph.edges.len();

        // 收集 HTTP 目标 URL 用于预览
        let http_target_urls: Vec<String> = graph
            .nodes
            .iter()
            .filter_map(|n| {
                n.spec.http.as_ref().map(|h| {
                    if h.url.is_empty() {
                        "{{runtime}}".to_string()
                    } else {
                        h.url.clone()
                    }
                })
            })
            .collect();

        // 收集 JS 源码用于预览
        let js_sources: Vec<String> = graph
            .nodes
            .iter()
            .filter_map(|n| n.spec.js.as_ref().map(|j| j.code.clone()))
            .collect();

        // 源 URL — `NativeImporter` 无外部源 URL，优先用第一个 Http 节点的 URL
        let source_url = http_target_urls.first().map_or_else(String::new, |u| {
            if u == "{{runtime}}" {
                String::new()
            } else {
                u.clone()
            }
        });

        Ok(ImportPreview {
            source_url,
            node_count,
            edge_count,
            js_block_count: js_sources.len(),
            sandbox: PolicyCapabilities {
                network: true,
                system: SystemCapabilities::default(),
            },
            http_target_urls,
            js_sources,
            graph,
        })
    }
}
