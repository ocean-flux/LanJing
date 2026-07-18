//! Tauri IPC 命令 — 规则导入/执行/管理(U13)。
//!
//! 提供四个 IPC 命令：导入预览、确认导入、列出规则、按段执行。
//! 执行命令通过 Tauri event 流式产出 `NodeData` 供前端渲染。

use std::collections::HashMap;
use std::sync::Mutex;

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use lj_capability::{IntentInput, StandardIntent};
use lj_importer::preview::ImportPreview;
use lj_media::{MediaGraphDelta, MediaResourceId};
use lj_rule_model::PolicyCapabilities;
use lj_runtime::NodeData;
use lj_runtime::{ExecutionContext, NodeProcessor, SegmentSpec};
use lj_runtime::{Graph, NodeKind};
use lj_storage::RepoId;

use lj_importer::legado::{LegadoImporter, LegadoSourceJson};
use lj_importer::maccms::{MaccmsFormat, MaccmsImporter, MaccmsSourceUrl};
use lj_importer::native::NativeImporter;

use lj_runtime::executor::GraphExecutor;
use lj_runtime::tap::{TapSummary, summarize_data};

use lj_storage::repository::SqliteStorage;
use lj_storage::{LibraryEntry, LibraryProgress, LibraryProjection};

use lj_node_extract::processor::ExtractNodeProcessor;
use lj_node_http::processor::HttpNodeProcessor;
use lj_node_js::processor::JsNodeProcessor;

// ===== 应用状态 =====

/// 应用共享状态：存储 + 执行器。
pub struct AppState {
    /// `SQLite` 存储（线程安全）。
    pub storage: Mutex<SqliteStorage>,
    /// 图执行器。
    pub executor: GraphExecutor,
}

// ===== 请求/响应类型 =====

/// 导入预览请求。
#[derive(Debug, Deserialize)]
pub struct ImportPreviewRequest {
    /// 规则 JSON 文本（Legado 书源 JSON、原生节点图 JSON 或 Maccms 采集 API URL）。
    pub rule_json: String,
    /// Maccms 源格式（仅 `rule_json` 是 Maccms URL 时生效，默认 Json，KTD2）。
    #[serde(default)]
    pub maccms_format: Option<MaccmsFormat>,
}

/// 导入预览响应（给前端展示）。
#[derive(Debug, Serialize)]
pub struct ImportPreviewResponse {
    /// 源站 URL。
    pub source_url: String,
    /// 节点数量。
    pub node_count: usize,
    /// 边数量。
    pub edge_count: usize,
    /// JS 块数量。
    pub js_block_count: usize,
    /// 沙箱能力配置。
    pub sandbox: PolicyCapabilities,
    /// 所有 HTTP 目标 URL 模板（SSRF 审计）。
    pub http_target_urls: Vec<String>,
    /// JS 块源码（用户审计）。
    pub js_sources: Vec<String>,
    /// Graph JSON（供 `confirm_import` 使用）。
    pub graph_json: String,
}

/// 确认导入请求。
#[derive(Debug, Deserialize)]
pub struct ConfirmImportRequest {
    /// 序列化的 `Graph` JSON。
    pub graph_json: String,
}

/// 执行段请求。
#[derive(Debug, Deserialize)]
pub struct ExecuteSegmentRequest {
    /// 规则 ID。
    pub rule_id: String,
    /// 要执行的标准意图。
    pub intent: StandardIntent,
    /// 标准意图输入。
    pub input: IntentInput,
}

/// 规则列表项（给前端展示）。
#[derive(Debug, Serialize)]
pub struct RuleListItem {
    /// 规则 ID。
    pub id: String,
    /// 源站 URL。
    pub source_url: String,
    /// 节点数量。
    pub node_count: usize,
}

/// 规则执行错误事件。
#[derive(Debug, Serialize)]
pub struct RuleErrorEvent {
    /// 产出错误的节点 ID。
    pub node_id: String,
    /// 本次执行 trace ID。
    pub trace_id: String,
    /// 面向前端的错误消息。
    pub message: String,
}

/// 合并标准媒体资源图增量请求。
#[derive(Debug, Deserialize)]
pub struct MediaGraphDeltaRequest {
    /// 标准媒体资源图增量。
    pub delta: MediaGraphDelta,
}

/// 更新共享资料库状态请求。
#[derive(Debug, Deserialize)]
pub struct LibraryEntryRequest {
    /// 标准媒体资源 ID。
    pub resource_id: String,
    /// 收藏状态。
    #[serde(default)]
    pub favorite: bool,
    /// 固定状态。
    #[serde(default)]
    pub pinned: bool,
    /// 最近打开时间。
    pub last_opened_at: Option<String>,
    /// 消费进度。
    pub progress: Option<LibraryProgress>,
}

// ===== 辅助函数 =====

/// 包装 `ImportPreview` 到响应结构。
fn preview_to_response(preview: ImportPreview) -> ImportPreviewResponse {
    let graph_json =
        serde_json::to_string(&preview.graph).unwrap_or_else(|e| format!("序列化 graph 失败: {e}"));
    ImportPreviewResponse {
        source_url: preview.source_url,
        node_count: preview.node_count,
        edge_count: preview.edge_count,
        js_block_count: preview.js_block_count,
        sandbox: preview.sandbox,
        http_target_urls: preview.http_target_urls,
        js_sources: preview.js_sources,
        graph_json,
    }
}

// ===== Tauri 命令 =====

/// 检测输入格式并导入（不含真实源站访问）。
///
/// 三类分发（ADR-0021 多入口）：先尝试 `LegadoSourceJson`（含 `bookSourceName` 字段），
/// 再尝试原生节点图 `Graph`（含 `nodes`/`edges` 字段），最后校验合法 Maccms 采集 URL。
/// 抽为纯函数便于单元测试覆盖三类分发与 URL 校验（#6/#7）。
fn detect_and_import(
    rule_json: &str,
    maccms_format: Option<MaccmsFormat>,
) -> Result<ImportPreview, String> {
    // 1. 尝试解析为 Legado 书源 JSON
    if let Ok(legado) = serde_json::from_str::<LegadoSourceJson>(rule_json) {
        return LegadoImporter
            .import(legado)
            .map_err(|e| format!("Legado 书源导入失败: {e}"));
    }

    // 2. 尝试解析为原生节点图 Graph
    if let Ok(graph) = serde_json::from_str::<Graph>(rule_json) {
        return NativeImporter
            .import(graph)
            .map_err(|e| format!("原生图导入失败: {e}"));
    }

    // 3. 校验合法 http/https URL + 路径含 /api.php/provide/vod (#7: 不再用 contains 子串)
    let trimmed = rule_json.trim();
    if let Ok(parsed) = url::Url::parse(trimmed)
        && matches!(parsed.scheme(), "http" | "https")
        && parsed.path().contains("/api.php/provide/vod")
    {
        let maccms_url = MaccmsSourceUrl {
            url: trimmed.to_string(),
            at: maccms_format.unwrap_or_default(),
        };
        return MaccmsImporter
            .import(maccms_url)
            .map_err(|e| format!("Maccms 源导入失败: {e}"));
    }

    Err("无法解析输入：既不是 Legado 书源格式、原生节点图格式，也不是合法 Maccms URL".to_string())
}

/// 导入规则并返回预览（不含真实源站访问）。
///
/// 自动检测格式并分发到三类 importer。Maccms 源可用 `maccms_format` 指定 XML/JSON（默认 JSON，KTD2）。
#[tauri::command]
pub fn import_rule_with_preview(
    request: ImportPreviewRequest,
) -> Result<ImportPreviewResponse, String> {
    let ImportPreviewRequest {
        rule_json,
        maccms_format,
    } = request;
    let preview = detect_and_import(&rule_json, maccms_format)?;
    Ok(preview_to_response(preview))
}

/// 确认导入规则并落库。
///
/// 将 `ConfirmImportRequest` 中的 `Graph` JSON 反序列化后存入 `SqliteStorage`，
/// 返回生成的规则 ID（`UUIDv4`）。
#[tauri::command]
pub fn confirm_import(
    state: State<'_, AppState>,
    request: ConfirmImportRequest,
) -> Result<String, String> {
    let ConfirmImportRequest { graph_json } = request;
    let graph: Graph =
        serde_json::from_str(&graph_json).map_err(|e| format!("反序列化图失败: {e}"))?;

    let rule_id = Uuid::new_v4().to_string();
    let repo_id = RepoId::<Graph>::new(rule_id.clone());

    let state = Some(state);
    let storage = state
        .as_ref()
        .ok_or_else(|| "应用状态缺失".to_string())?
        .storage
        .lock()
        .map_err(|e| format!("存储锁获取失败: {e}"))?;
    storage
        .save_graph(&repo_id, &graph)
        .map_err(|e| format!("保存规则失败: {e}"))?;

    Ok(rule_id)
}

/// 列出所有已导入规则。
#[tauri::command]
pub fn list_rules(state: State<'_, AppState>) -> Result<Vec<RuleListItem>, String> {
    let state = Some(state);
    let storage = state
        .as_ref()
        .ok_or_else(|| "应用状态缺失".to_string())?
        .storage
        .lock()
        .map_err(|e| format!("存储锁获取失败: {e}"))?;
    let items: Vec<(RepoId<Graph>, Graph)> = storage
        .list_graphs()
        .map_err(|e| format!("读取规则列表失败: {e}"))?;

    // ponytail: `source_url` 暂不落库，后续 `confirm_import` 可存入独立字段
    Ok(items
        .into_iter()
        .map(|(id, graph)| RuleListItem {
            id: id.id,
            source_url: String::new(),
            node_count: graph.nodes.len(),
        })
        .collect())
}

/// 读取共享资料库投影。
#[tauri::command]
pub fn get_library_projection(state: State<'_, AppState>) -> Result<LibraryProjection, String> {
    let state = Some(state);
    let storage = state
        .as_ref()
        .ok_or_else(|| "应用状态缺失".to_string())?
        .storage
        .lock()
        .map_err(|e| format!("存储锁获取失败: {e}"))?;
    storage
        .library_projection()
        .map_err(|e| format!("读取资料库投影失败: {e}"))
}

/// 将标准媒体资源图增量合并到唯一权威资源图。
#[tauri::command]
pub fn merge_media_graph_delta(
    state: State<'_, AppState>,
    request: MediaGraphDeltaRequest,
) -> Result<MediaGraphDelta, String> {
    let state = Some(state);
    let storage = state
        .as_ref()
        .ok_or_else(|| "应用状态缺失".to_string())?
        .storage
        .lock()
        .map_err(|e| format!("存储锁获取失败: {e}"))?;
    storage
        .merge_media_graph_delta(request.delta)
        .map_err(|e| format!("合并媒体资源图增量失败: {e}"))
}

/// 更新共享资料库状态，供所有媒体空间投影共同观察。
#[tauri::command]
pub fn update_library_entry(
    state: State<'_, AppState>,
    request: LibraryEntryRequest,
) -> Result<(), String> {
    let entry = LibraryEntry {
        resource_id: MediaResourceId(request.resource_id),
        favorite: request.favorite,
        pinned: request.pinned,
        last_opened_at: request.last_opened_at,
        progress: request.progress,
    };
    let state = Some(state);
    let storage = state
        .as_ref()
        .ok_or_else(|| "应用状态缺失".to_string())?
        .storage
        .lock()
        .map_err(|e| format!("存储锁获取失败: {e}"))?;
    storage
        .set_library_entry(&entry)
        .map_err(|e| format!("更新资料库状态失败: {e}"))
}

/// 按段执行规则，流式 emit `rule-output` 和 `node-output` 事件。
///
/// 1. 从存储读取 `Graph`。
/// 2. 构造段规格（`SegmentSpec`）。
/// 3. 构建处理器注册表。
/// 4. 调用 `GraphExecutor.execute` 获取输出 stream。
/// 5. 消费每个 `NodeData` item，emit `rule-output`（完整数据）和 `node-output`（摘要）。
#[tauri::command]
pub async fn execute_segment(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ExecuteSegmentRequest,
) -> Result<(), String> {
    let rule_id = RepoId::<Graph>::new(request.rule_id.clone());

    // 1. 从存储读取图
    let graph = {
        let storage = state
            .storage
            .lock()
            .map_err(|e| format!("存储锁获取失败: {e}"))?;
        storage
            .get_graph(&rule_id)
            .map_err(|e| format!("读取规则失败: {e}"))?
            .ok_or_else(|| format!("规则 {} 不存在", request.rule_id))?
    };

    // 2. 构造段规格
    let segment = SegmentSpec {
        intent: request.intent,
        input: request.input,
    };

    // 3. 构建处理器注册表
    let mut processors: HashMap<NodeKind, Box<dyn NodeProcessor>> = HashMap::new();
    processors.insert(NodeKind::Http, Box::new(HttpNodeProcessor::new()));
    processors.insert(NodeKind::Js, Box::new(JsNodeProcessor));
    processors.insert(NodeKind::Extract, Box::new(ExtractNodeProcessor));
    // ponytail: Merge/Condition/Loop 首刀 stub，处理器暂不注册

    // 3b. 创建执行上下文
    let ctx = ExecutionContext {
        cookies: HashMap::new(),
        caps: PolicyCapabilities::default(),
        trace_id: Uuid::new_v4().to_string(),
        base_url: graph.base_url.clone(),
    };

    // 4. 执行并流式 emit
    // executor 内部已用 tap_stream 包裹每个节点 output(含真实 NodeId 和 tracing 日志)
    let mut output = state.executor.execute(&graph, &segment, &ctx, &processors);

    while let Some((node_id, item)) = output.next().await {
        // 5a. 只 emit Media 给前端（避免传递 16MB body）
        match &item {
            NodeData::Delta(delta) => {
                {
                    let storage = state
                        .storage
                        .lock()
                        .map_err(|e| format!("存储锁获取失败: {e}"))?;
                    storage
                        .merge_media_graph_delta(delta.clone())
                        .map_err(|e| format!("持久化媒体资源图增量失败: {e}"))?;
                }
                app.emit("rule-output", &item)
                    .map_err(|e| format!("emit rule-output 失败: {e}"))?;
            }
            NodeData::Error(msg) => {
                let error_event = RuleErrorEvent {
                    node_id: node_id.0.to_string(),
                    trace_id: ctx.trace_id.clone(),
                    message: msg.clone(),
                };
                app.emit("rule-error", &error_event)
                    .map_err(|e| format!("emit rule-error 失败: {e}"))?;
            }
            _ => {} // HttpResponse/Raw/Json 不 emit 给前端
        }

        // 5b. emit 摘要 event（node-output），使用 executor 返回的真实 NodeId
        let (variant, summary_text) = summarize_data(&item);
        let summary = TapSummary {
            node_id,
            variant,
            summary: summary_text,
        };
        app.emit("node-output", &summary)
            .map_err(|e| format!("emit node-output 失败: {e}"))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    //! `import_rule_with_preview` 三类分发与 URL 校验测试（#6）。

    use super::detect_and_import;

    /// Legado 书源 JSON → `LegadoImporter` 分发。
    #[test]
    fn dispatch_legado_json() {
        // 最小合法 Legado 书源 JSON(含 bookSourceName 触发 LegadoSourceJson 解析)。
        let json = r#"{"bookSourceName":"测试源","bookSourceUrl":"https://example.com"}"#;
        let result = detect_and_import(json, None);
        assert!(
            result.is_ok(),
            "Legado JSON 应分发到 LegadoImporter: {result:?}"
        );
    }

    /// 原生节点图 JSON → `NativeImporter` 分发。
    #[test]
    fn dispatch_native_graph_json() {
        // 空图(无标准意图子图)可通过校验：`intent_exports` 为空时只验证结构与 I/O。
        let json = r#"{"nodes":[],"edges":[],"subroutines":{},"source_id":"00000000-0000-0000-0000-000000000000","base_url":""}"#;
        let result = detect_and_import(json, None);
        assert!(
            result.is_ok(),
            "空图 JSON 应分发到 NativeImporter: {result:?}"
        );
    }

    /// 合法 Maccms URL → `MaccmsImporter` 分发(默认 `Json`)。
    #[test]
    fn dispatch_maccms_url_default_json() {
        let url = "https://example.com/api.php/provide/vod/";
        let result = detect_and_import(url, None);
        assert!(
            result.is_ok(),
            "合法 Maccms URL 应分发到 MaccmsImporter: {result:?}"
        );
    }

    /// 合法 Maccms URL + 显式 `Xml` format → `MaccmsImporter` 分发(#2)。
    #[test]
    fn dispatch_maccms_url_xml_format() {
        let url = "https://example.com/api.php/provide/vod/";
        let result = detect_and_import(url, Some(lj_importer::maccms::MaccmsFormat::Xml));
        assert!(
            result.is_ok(),
            "XML format 应透传到 MaccmsImporter: {result:?}"
        );
    }

    /// 含 /api.php/provide/vod 子串但非合法 URL → 拒绝(#7)。
    #[test]
    fn reject_non_url_with_substring() {
        // javascript: 伪协议含子串,但非 http/https 应被拒绝
        let bad = "javascript://api.php/provide/vod/";
        let result = detect_and_import(bad, None);
        assert!(
            result.is_err(),
            "非 http/https URL 含子串应被拒绝: {result:?}"
        );
    }

    /// 完全无法识别的输入 → 拒绝。
    #[test]
    fn reject_unrecognizable() {
        let bad = "随机文本不是任何格式";
        let result = detect_and_import(bad, None);
        assert!(result.is_err(), "无法识别输入应被拒绝: {result:?}");
    }

    /// `base_url` 含已有 query 时 Maccms URL 仍可导入(#12 修复后)。
    #[test]
    fn dispatch_maccms_url_with_existing_query() {
        let url = "https://example.com/api.php/provide/vod/?token=abc";
        let result = detect_and_import(url, None);
        assert!(
            result.is_ok(),
            "含已有 query 的 Maccms URL 应可导入: {result:?}"
        );
    }
}
