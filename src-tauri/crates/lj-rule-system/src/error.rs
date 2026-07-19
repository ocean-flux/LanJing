//! `RuleSystem` 对外错误合同。
//!
//! 错误只保留生命周期阶段、稳定 code、可安全展示的消息、trace 与诊断；不会包含
//! HTTP body、cookie、token、完整 URL query 或来源 opaque payload。

use std::fmt;

use lj_rule_model::Diagnostic;
use serde::{Deserialize, Serialize};

/// 规则生命周期失败所属阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleErrorStage {
    /// 来源输入或来源适配失败。
    Import,
    /// Definition 语义校验失败。
    Validation,
    /// Definition 编译为 immutable Plan 失败。
    Compile,
    /// candidate staging、读取、篡改或到期失败。
    Candidate,
    /// 安装提交失败。
    Install,
    /// 用户 grant 未覆盖所需能力。
    Capability,
    /// execution 启动或 session 状态失败。
    Execution,
    /// effect adapter 或 durable archive 失败。
    Effect,
    /// Event/projection 持久化失败。
    Persistence,
    /// replay pin、artifact 或历史 effect 验证失败。
    Replay,
    /// execution 已被取消。
    Cancelled,
    /// 不应暴露内部细节的意外状态。
    Internal,
}

/// 规则生命周期的安全错误。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleError {
    /// 失败阶段。
    pub stage: RuleErrorStage,
    /// 稳定机器可读 code。
    pub code: String,
    /// 可安全呈现给用户的短消息。
    pub message: String,
    /// 与 candidate、安装或 execution 关联的 trace ID。
    pub trace_id: String,
    /// 调用方是否可以在不改变输入的前提下重试。
    pub retryable: bool,
    /// compiler 返回的可定位诊断。
    pub diagnostics: Vec<Diagnostic>,
}

impl RuleError {
    /// 在 crate 内构造不泄露敏感载荷的错误。
    pub(crate) fn new(
        stage: RuleErrorStage,
        code: impl Into<String>,
        message: impl Into<String>,
        trace_id: impl Into<String>,
        retryable: bool,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        Self {
            stage,
            code: code.into(),
            message: message.into(),
            trace_id: trace_id.into(),
            retryable,
            diagnostics,
        }
    }
}

impl fmt::Display for RuleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for RuleError {}
