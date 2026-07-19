//! durable effect capture 与 replay archive seam。
//!
//! live capture 以 `EffectCapture` 交给 C2：实现必须在返回收据前完成 artifact/secret 写入
//! 及 event/WAL 追加。replay 只读取 archive 中已 pin 的记录；缺记录、缺 artifact 或密钥
//! 不可用都必须显式失败，不能调用 live adapter。

use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use lj_rule_model::EffectKind;
use uuid::Uuid;

use super::contracts::EffectOutput;
use super::witness::{
    CapturedEffectOutput, EffectCaptureMaterial, EffectCaptureMaterialSensitivity, EffectWitness,
    EffectWitnessError, effect_output_hash,
};

/// 已发生 effect 的 durable capture。
///
/// `output` 由 `Arc` 共享，archive 和下游节点不会重复复制 HTTP body。`witness` 只含
/// safe URL、hash、IP、timing 和确定性 host-call metadata；archive 必须将它和 output
/// 一起 durable 写入，且把敏感 response header 继续分离到 Secret Artifact。
#[derive(Clone)]
pub struct EffectCapture {
    /// 发生 effect 的执行 ID。
    pub execution_id: Uuid,
    /// effect 的执行唯一 ID。
    pub effect_id: Uuid,
    /// 所属 Plan 节点 ID。
    pub node_id: Uuid,
    /// effect 类型。
    pub kind: EffectKind,
    /// 由 Plan、节点配置与上游输入得出的稳定 fingerprint。
    pub fingerprint: String,
    /// 类型化输出的 canonical hash。
    pub output_hash: String,
    /// 安全 witness 的 canonical hash。
    pub witness_hash: String,
    /// 不复制的大型类型化输出。
    pub output: Arc<EffectOutput>,
    /// 与输出绑定的安全 execution witness。
    pub witness: EffectWitness,
    /// 仅供 C2 archive 写入 artifact 的原始 request body；不会进入 replay output。
    pub(crate) capture_material: EffectCaptureMaterial,
}

/// archive 已解码的 replay capture 字段。
///
/// 该请求不含 live request body material；replay 不得重新暴露或持久化它。
#[derive(Clone)]
pub struct ArchivedEffectCapture {
    /// 历史 execution ID。
    pub execution_id: Uuid,
    /// 历史 effect ID。
    pub effect_id: Uuid,
    /// 所属 Plan 节点 ID。
    pub node_id: Uuid,
    /// effect 类型。
    pub kind: EffectKind,
    /// 历史 Plan/input fingerprint。
    pub fingerprint: String,
    /// archive 保存的类型化输出 hash。
    pub output_hash: String,
    /// archive 保存的安全 witness hash。
    pub witness_hash: String,
    /// 已解码的类型化输出。
    pub output: Arc<EffectOutput>,
    /// 已解码的安全 witness。
    pub witness: EffectWitness,
}

impl EffectCapture {
    /// 从 live handler 的已验证输出构造待持久化 capture。
    ///
    /// 本构造器保留 handler 附带的 archive material；调用方无法直接读取或伪造其字段。runtime
    /// 与 C2 应通过它构造 live capture，而 replay 应使用 [`Self::from_archived`]。
    ///
    /// # Errors
    ///
    /// output/witness 不匹配、fingerprint 为空，或 canonical hash 无法生成时返回
    /// [`EffectWitnessError`]。
    pub fn from_live(
        execution_id: Uuid,
        effect_id: Uuid,
        node_id: Uuid,
        fingerprint: String,
        captured: CapturedEffectOutput,
    ) -> Result<Self, EffectWitnessError> {
        if fingerprint.trim().is_empty() {
            return Err(EffectWitnessError::InvalidHash);
        }
        captured.validate()?;
        let (output, witness, capture_material) = captured.into_parts();
        let kind = output.kind();
        let output = Arc::new(output);
        let output_hash = effect_output_hash(output.as_ref())?;
        let witness_hash = witness.canonical_hash()?;
        Ok(Self {
            execution_id,
            effect_id,
            node_id,
            kind,
            fingerprint,
            output_hash,
            witness_hash,
            output,
            witness,
            capture_material,
        })
    }

    /// 从 C2 archive 已解码的安全 output/witness 重建 replay capture。
    ///
    /// archive material 在 replay 时为空：request body 只用于 live persistence，不应再次进入
    /// runtime。返回值在构造时立即执行完整性校验。
    ///
    /// # Errors
    ///
    /// output/witness 类型、字段安全性或 canonical hash 不匹配时返回 [`EffectWitnessError`]。
    pub fn from_archived(archived: ArchivedEffectCapture) -> Result<Self, EffectWitnessError> {
        let ArchivedEffectCapture {
            execution_id,
            effect_id,
            node_id,
            kind,
            fingerprint,
            output_hash,
            witness_hash,
            output,
            witness,
        } = archived;
        let capture = Self {
            execution_id,
            effect_id,
            node_id,
            kind,
            fingerprint,
            output_hash,
            witness_hash,
            output,
            witness,
            capture_material: EffectCaptureMaterial::default(),
        };
        capture.validate_replay_integrity()?;
        Ok(capture)
    }

    /// 返回仅供 C2 archive 写入的 HTTP request body。
    ///
    /// body 不会写入 witness、tracing 或 delivery event，且必须作为 Secret Artifact 持久化。
    #[must_use]
    pub fn request_body(&self) -> Option<&[u8]> {
        self.capture_material.request_body()
    }

    /// 返回 request body 的强制 archive 敏感性。
    #[must_use]
    pub fn request_body_sensitivity(&self) -> Option<EffectCaptureMaterialSensitivity> {
        self.capture_material.request_body_sensitivity()
    }

    /// 校验 archive 重建的 capture 是否仍符合 output/witness 完整性合同。
    ///
    /// C2 在 replay 解码 body/secret artifact 后调用本方法；runtime 随后还会校验当前 Plan
    /// fingerprint。此方法不记录或返回任何 payload。
    ///
    /// # Errors
    ///
    /// 输出/witness 类型不一致、字段不安全，或 output/witness canonical hash 不匹配时返回
    /// [`EffectWitnessError`]。
    pub fn validate_replay_integrity(&self) -> Result<(), EffectWitnessError> {
        if self.fingerprint.trim().is_empty() || self.kind != self.output.kind() {
            return Err(EffectWitnessError::KindMismatch);
        }
        CapturedEffectOutput::new((*self.output).clone(), self.witness.clone()).validate()?;
        if self.output_hash != effect_output_hash(self.output.as_ref())?
            || self.witness_hash != self.witness.canonical_hash()?
        {
            return Err(EffectWitnessError::InvalidHash);
        }
        Ok(())
    }
}

/// archive 成功持久化 capture 后返回的收据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurableCaptureReceipt {
    /// 已持久化的 effect ID。
    pub effect_id: Uuid,
    /// 已持久化的 effect fingerprint。
    pub fingerprint: String,
    /// 已持久化的输出 hash。
    pub output_hash: String,
    /// 已持久化的安全 witness hash。
    pub witness_hash: String,
}

/// replay archive 的查找键。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectReplayLookup {
    /// 被 replay 的历史执行 ID。
    pub archived_execution_id: Uuid,
    /// 需要重放的 Plan 节点 ID。
    pub node_id: Uuid,
    /// 需要重放的 effect 类型。
    pub kind: EffectKind,
}

/// archive seam 的安全错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectArchiveError {
    /// 可安全展示的简短消息。
    pub message: String,
}

impl EffectArchiveError {
    /// 用安全消息创建 archive 错误。
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for EffectArchiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for EffectArchiveError {}

/// effect capture/replay 的真实持久化 seam。
///
/// runtime 不提供 no-op 实现：live 模式必须由调用方注入会在返回前完成 artifact/secret
/// 写入及 event/WAL 追加的 archive。这样 runtime 才能在确认 durable receipt 前阻止
/// 依赖节点推进。
#[async_trait]
pub trait EffectArchive: Send + Sync {
    /// 将 live effect 的输出 durable 持久化，并在完成后返回匹配收据。
    ///
    /// archive 事务一旦开始不得因 cancellation 被中间抢占；它必须自行完成 commit 或
    /// rollback。runtime 会在收据后检查取消状态，确保不会用未确认结果推进下游。
    ///
    /// # Errors
    ///
    /// artifact/secret 写入、event/WAL 追加或事务提交失败时返回 [`EffectArchiveError`]。
    async fn persist_durable(
        &self,
        capture: EffectCapture,
    ) -> Result<DurableCaptureReceipt, EffectArchiveError>;

    /// 读取历史 execution 的 effect capture 供 replay 使用。
    ///
    /// `None` 表示该历史 execution 没有对应记录；runtime 会将其视为硬失败，而不是
    /// 回退到 live effect。
    ///
    /// # Errors
    ///
    /// archive 不可读、artifact 缺失或密钥不可用时返回 [`EffectArchiveError`]。
    async fn load_replay(
        &self,
        lookup: EffectReplayLookup,
    ) -> Result<Option<EffectCapture>, EffectArchiveError>;
}
