//! 存储层的安全错误合同。
//!
//! 错误值不携带 artifact 明文、凭证、HTTP body 或 URL query；调用方只能根据稳定类别决定
//! 是否重试、提示用户或停止 replay。

/// 存储层返回的安全失败类别。
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// 调用方使用了过期的 stream revision。
    #[error("事件流版本冲突：{stream_id} 期望 {expected}，实际为 {actual}")]
    VersionConflict {
        /// 冲突的事件流。
        stream_id: String,
        /// 调用方预期版本。
        expected: u64,
        /// 当前已提交版本。
        actual: u64,
    },
    /// 同一 event ID 试图写入不同内容。
    #[error("事件 ID 已对应不同的持久化内容")]
    IdempotencyMismatch,
    /// writer 已关闭，不能再接受请求。
    #[error("存储 writer 已关闭")]
    WriterClosed,
    /// writer 已在完成请求前退出。
    #[error("存储 writer 在完成请求前退出")]
    WriterUnavailable,
    /// candidate 不存在。
    #[error("candidate 不存在")]
    CandidateMissing,
    /// candidate 已过期。
    #[error("candidate 已过期")]
    CandidateExpired,
    /// candidate 已被消费或丢弃。
    #[error("candidate 已不可安装")]
    CandidateUnavailable,
    /// candidate 的 durable artifact、metadata 或 staging Event 不一致。
    #[error("candidate durable metadata 被篡改")]
    CandidateTampered,
    /// 已批准能力未覆盖 candidate 在 staging 时声明的必需能力。
    #[error("批准的能力不足以安装 candidate")]
    GrantInsufficient,
    /// source credential staging 缺失、已过期或与安装来源不匹配。
    #[error("source credential snapshot 不可用")]
    SourceCredentialUnavailable,
    /// candidate 的 document/source baseline 已变化。
    #[error("candidate 基线已过期")]
    CandidateStale,
    /// candidate schema 与当前 writer/consumer 不一致。
    #[error("candidate schema 不兼容")]
    CandidateSchemaMismatch,
    /// 已安装规则、package 或 Plan 声明了当前 reader 不认识的 schema 版本。
    #[error("规则合同 schema 不受支持: {contract:?} schema_version={version}")]
    ContractSchemaUnsupported {
        /// 无法读取的合同种类。
        contract: lj_rule_model::SchemaContract,
        /// artifact 声明的未知 wire 版本。
        version: u32,
    },
    /// 已安装规则、package 或 Plan 仍是 current schema 下的历史结构签名。
    #[error("历史规则合同不受支持: {contract:?}")]
    LegacyRuleContractUnsupported {
        /// 仍保留但不可解释的合同种类。
        contract: lj_rule_model::SchemaContract,
    },
    /// effect archive 缺少 current invocation path/ordinal。
    #[error("历史 effect invocation archive 不受支持")]
    LegacyInvocationArchiveUnsupported,
    /// 来源文档不存在。
    #[error("来源文档不存在")]
    DocumentMissing,
    /// 已关联来源或仍被 pin 的文档不能删除。
    #[error("来源文档仍被关联或固定")]
    DocumentDeleteUnsafe,
    /// credential slot 不属于请求的 document revision。
    #[error("credential slot ownership 不匹配")]
    CredentialOwnershipMismatch,
    /// 来源尚未安装。
    #[error("来源尚未安装")]
    SourceMissing,
    /// execution 尚未建立。
    #[error("execution 尚未建立")]
    ExecutionMissing,
    /// artifact 的元数据或文件不存在。
    #[error("artifact 不可用：{0}")]
    ArtifactUnavailable(String),
    /// secret 所需的安装级主密钥不可用。
    #[error("secret artifact 主密钥不可用，归档不能 replay")]
    MasterKeyUnavailable,
    /// secret artifact 未通过认证或无法解密。
    #[error("secret artifact 无法认证或解密，归档不能 replay")]
    SecretUnavailable,
    /// 历史 archive 不具备 replay 条件。
    #[error("execution archive 不可 replay：{0}")]
    ReplayUnavailable(String),
    /// `SQLite` 操作失败。
    #[error("SQLite 存储操作失败：{0}")]
    Database(String),
    /// 文件系统操作失败。
    #[error("artifact 文件操作失败：{0}")]
    FileSystem(String),
    /// 当前平台没有可用的原生 secure store。
    #[error("原生 secure store 不可用")]
    KeyringUnavailable,
    /// 原生 secure store 暂时锁定。
    #[error("原生 secure store 已锁定")]
    KeyringLocked,
    /// `SQLite` 记录的随机 key ID 已不在 secure store 中。
    #[error("vault key 已丢失")]
    KeyLost,
    /// secret envelope、ciphertext hash 或 AEAD 认证失败。
    #[error("secret artifact 密文损坏")]
    ArtifactCorrupt,
    /// vault schema/data migration 未能完成；旧数据未被删除。
    #[error("来源文档保险库迁移失败")]
    VaultMigrationFailed,
    /// keyring 操作失败。
    #[error("keyring 操作失败")]
    Keyring,
    /// JSON 编解码失败。
    #[error("存储 JSON 编解码失败")]
    Serialization,
    /// 输入不满足存储不变量。
    #[error("存储输入无效：{0}")]
    InvalidInput(String),
}
