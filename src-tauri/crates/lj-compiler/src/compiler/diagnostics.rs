//! 安全 diagnostic、稳定 path/span 与排序。

use super::{Diagnostic, DiagnosticSeverity, FlowEdge, FlowNode, SourceSpan, Uuid};

pub(in crate::compiler) fn diagnostic(
    code: &str,
    message: impl Into<String>,
    span: SourceSpan,
) -> Diagnostic {
    Diagnostic {
        code: code.to_string(),
        severity: DiagnosticSeverity::Error,
        message: message.into(),
        span: Some(span),
    }
}

pub(in crate::compiler) fn node_diagnostic(
    code: &str,
    message: impl Into<String>,
    node: &FlowNode,
    suffix: &str,
) -> Diagnostic {
    diagnostic(
        code,
        message,
        semantic_span(node.span.as_ref(), node_path(node.id, suffix)),
    )
}

pub(in crate::compiler) fn edge_diagnostic(
    code: &str,
    message: impl Into<String>,
    edge: &FlowEdge,
) -> Diagnostic {
    diagnostic(code, message, schema_span(edge_path(edge)))
}

pub(in crate::compiler) fn semantic_span(
    source: Option<&SourceSpan>,
    path: impl Into<String>,
) -> SourceSpan {
    SourceSpan {
        start: source.map_or(0, |span| span.start),
        end: source.map_or(0, |span| span.end),
        path: Some(path.into()),
    }
}

pub(in crate::compiler) fn schema_span(path: impl Into<String>) -> SourceSpan {
    semantic_span(None, path)
}

pub(in crate::compiler) fn node_path(node_id: Uuid, suffix: &str) -> String {
    format!("/flow/nodes/{node_id}/config{suffix}")
}

pub(in crate::compiler) fn edge_path(edge: &FlowEdge) -> String {
    format!(
        "/flow/edges/{}/{}/{}/{}",
        edge.from.node_id,
        pointer_token(&edge.from.handle),
        edge.to.node_id,
        pointer_token(&edge.to.handle)
    )
}

pub(in crate::compiler) fn pointer_token(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

pub(in crate::compiler) fn sort_diagnostics(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|left, right| {
        diagnostic_path(left)
            .cmp(diagnostic_path(right))
            .then_with(|| diagnostic_start(left).cmp(&diagnostic_start(right)))
            .then_with(|| left.code.as_bytes().cmp(right.code.as_bytes()))
            .then_with(|| left.message.as_bytes().cmp(right.message.as_bytes()))
    });
}

pub(in crate::compiler) fn diagnostic_path(diagnostic: &Diagnostic) -> &str {
    diagnostic
        .span
        .as_ref()
        .and_then(|span| span.path.as_deref())
        .unwrap_or("")
}

pub(in crate::compiler) fn diagnostic_start(diagnostic: &Diagnostic) -> usize {
    diagnostic.span.as_ref().map_or(0, |span| span.start)
}
