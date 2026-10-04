//! Definition → immutable Execution Plan 的确定性编译。
//!
//! 本模块只处理作者合同的规范化、校验与编译；不解析来源专有格式，也不依赖
//! runtime、存储或 Tauri。closed value-kind compatibility matrix 与节点生成端口只在这里
//! 定义；Plan writer 始终由 `lj-rule-model` 的 current 构造器封存。本 crate 不维护第二套
//! 节点/端口清单，也不做 schema 版本分支或 legacy 读取。

use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

use lj_rule_model::definition::{
    CollectionSelector, ConditionConfig, ConditionPredicate, ControlExpression, ControlledMapper,
    FlowEdge, FlowNode, FlowNodeConfig, FlowPortRef, LoopIterationLimit, MAX_LOOP_ITERATIONS,
    MergeConfig, RuleDefinition, SourceSpan,
};
use lj_rule_model::literal::TypedLiteral;
use lj_rule_model::plan::{
    CONDITION_INPUT_HANDLE, ControlRegion, EffectDeclaration, EffectKind, ExecutionPlan,
    ExecutionPlanParts, IntentEntry, LINEAR_INPUT_HANDLE, LINEAR_OUTPUT_HANDLE, LOOP_BODY_HANDLE,
    LOOP_COLLECTION_HANDLE, LOOP_DONE_HANDLE, LOOP_YIELD_HANDLE, LoopControlRegion,
    MERGE_OUTPUT_HANDLE, PlanEdge, PlanForEachConfig, PlanNode, PlanNodeConfig, PlanPort,
    PortValueKind, PortValueType,
};
use lj_rule_model::{Diagnostic, DiagnosticSeverity, definition_hash};
use uuid::Uuid;

use crate::error::CompilerError;

mod analysis;
mod canonicalize;
mod diagnostics;
mod graph;
mod lowering;
mod ports;
mod validation;

use analysis::{NodePorts, ValidatedLoopRegion, analyze};
pub use canonicalize::canonicalize;
use diagnostics::{
    diagnostic, edge_diagnostic, node_diagnostic, node_path, pointer_token, schema_span,
    semantic_span, sort_diagnostics, unavailable_capability_diagnostic,
};
use graph::{
    adjacency, edges_by_input, edges_by_output, has_cycle, is_reachable, is_valid_json_pointer,
};
use lowering::build_plan;
use ports::{accepts_kind, find_port, ports_are_compatible, ports_for_node};
use validation::{
    validate_definition_header, validate_edges, validate_input_and_control_handles,
    validate_intent_exports, validate_loop_region_overlap, validate_loop_regions,
    validate_node_configuration,
};

/// 默认 compiler 身份；其值参与 Plan hash。
pub const DEFAULT_COMPILER_VERSION: &str = concat!("lj-compiler@", env!("CARGO_PKG_VERSION"));

const NETWORK_CAPABILITY: &str = "network";

/// 纯 Definition compiler。
#[derive(Debug, Clone)]
pub struct Compiler {
    version: String,
}

impl Compiler {
    /// 使用显式 compiler 身份创建 compiler。
    #[must_use]
    pub fn with_version(version: String) -> Self {
        Self { version }
    }

    /// 返回本 compiler 的稳定身份。
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// 校验并将 current Definition 编译成 immutable Plan。
    ///
    /// # Errors
    ///
    /// Definition 不满足 config、typed handle、端口兼容、structured Loop、意图可达性与
    /// 能力合同时返回 [`CompilerError::Validation`]；Plan canonical hash 无法生成时返回
    /// [`CompilerError::Serialization`]。
    pub fn compile(&self, definition: &RuleDefinition) -> Result<ExecutionPlan, CompilerError> {
        let definition = canonicalize(definition);
        let analysis = analyze(&definition);
        if analysis
            .diagnostics
            .iter()
            .any(|item| item.severity == DiagnosticSeverity::Error)
        {
            return Err(CompilerError::validation(analysis.diagnostics));
        }

        build_plan(&definition, &self.version, analysis.control_regions)
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::with_version(DEFAULT_COMPILER_VERSION.to_string())
    }
}

/// 返回 Definition 的全部稳定、可定位诊断。
///
/// 本函数不丢弃后续错误，并先 canonicalize 声明顺序，因此同一非法语义不会因编辑器
/// 数组顺序不同而改变诊断顺序。
#[must_use]
pub fn validate(definition: &RuleDefinition) -> Vec<Diagnostic> {
    analyze(&canonicalize(definition)).diagnostics
}
