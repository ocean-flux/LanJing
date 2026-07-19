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
    /// 已批准能力未覆盖 candidate 在 staging 时声明的必需能力。
    #[error("批准的能力不足以安装 candidate")]
    GrantInsufficient,
    /// source credential staging 缺失、已过期或与安装来源不匹配。
    #[error("source credential snapshot 不可用")]
    SourceCredentialUnavailable,
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
