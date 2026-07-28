//! effect/control invocation 的唯一 current wire identity 与 trace。
//!
//! `InvocationPath` 使用 execution-local 一基 ordinal，并以 outer-to-inner Loop segment
//! 消除同一 Plan 节点在多轮执行中的归属歧义。它不表示 nested Loop 已开放；compiler 仍拥有
//! 可执行拓扑的支持边界。

use std::collections::BTreeSet;

use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

/// 一段结构化 Loop invocation 路径。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoopInvocationSegment {
    /// Loop Plan 节点 ID。
    pub loop_id: Uuid,
    /// 当前 Loop 的零基 iteration index。
    pub iteration_index: u32,
}

/// invocation identity 不满足 current 合同。
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InvocationPathError {
    /// 节点 ID 或 Loop ID 是 nil UUID。
    #[error("invocation path 不允许 nil UUID")]
    NilNode,
    /// execution-local ordinal 必须从 1 开始。
    #[error("invocation ordinal 必须从 1 开始")]
    ZeroOrdinal,
    /// 同一 Loop 不能在一条 path 中重复出现。
    #[error("invocation path 包含重复 Loop segment")]
    DuplicateLoop,
}

/// 一个 effect 或 control decision 的 execution-local 唯一身份。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InvocationPath {
    node_id: Uuid,
    loop_iterations: Vec<LoopInvocationSegment>,
    ordinal: u64,
}

impl InvocationPath {
    /// 创建并验证 current invocation path。
    ///
    /// # Errors
    ///
    /// 节点 ID/Loop ID 为 nil、ordinal 为零或 Loop segment 重复时返回
    /// [`InvocationPathError`]。
    pub fn new(
        node_id: Uuid,
        loop_iterations: Vec<LoopInvocationSegment>,
        ordinal: u64,
    ) -> Result<Self, InvocationPathError> {
        if node_id.is_nil()
            || loop_iterations
                .iter()
                .any(|segment| segment.loop_id.is_nil())
        {
            return Err(InvocationPathError::NilNode);
        }
        if ordinal == 0 {
            return Err(InvocationPathError::ZeroOrdinal);
        }
        let mut loop_ids = BTreeSet::new();
        if loop_iterations
            .iter()
            .any(|segment| !loop_ids.insert(segment.loop_id))
        {
            return Err(InvocationPathError::DuplicateLoop);
        }
        Ok(Self {
            node_id,
            loop_iterations,
            ordinal,
        })
    }

    /// 返回 Plan 节点 ID。
    #[must_use]
    pub const fn node_id(&self) -> Uuid {
        self.node_id
    }

    /// 返回 outer-to-inner Loop iteration segments。
    #[must_use]
    pub fn loop_iterations(&self) -> &[LoopInvocationSegment] {
        &self.loop_iterations
    }

    /// 返回 execution-local 一基调用序号。
    #[must_use]
    pub const fn ordinal(&self) -> u64 {
        self.ordinal
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InvocationPathWire {
    node_id: Uuid,
    loop_iterations: Vec<LoopInvocationSegment>,
    ordinal: u64,
}

impl<'de> Deserialize<'de> for InvocationPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = InvocationPathWire::deserialize(deserializer)?;
        Self::new(wire.node_id, wire.loop_iterations, wire.ordinal)
            .map_err(serde::de::Error::custom)
    }
}

/// durable replay 需要验证的闭集 control trace。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ControlTrace {
    /// Condition 实际激活的命名 branch。
    Condition {
        /// 已声明并被激活的 branch handle。
        branch: String,
    },
    /// Merge 按显式 input order 实际激活的 input IDs。
    Merge {
        /// 活跃 input IDs；顺序就是 Merge semantic order。
        active_inputs: Vec<String>,
    },
    /// Loop 在 hard-limit 校验后执行的 iteration 数量。
    Loop {
        /// collection 长度，也就是实际 iteration 数量。
        iteration_count: u32,
    },
}
