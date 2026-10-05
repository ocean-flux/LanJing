//! 来源输入、candidate、grant 与 installed source DTO。

use std::fmt;

use lj_media::SourceProfile;
use lj_rule_model::{Diagnostic, SystemCapabilities};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// `RuleSystem` 当前接受的来源输入。
#[derive(PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuleInput {
    /// Maccms JSON 采集 API 端点。
    MaccmsJson {
        /// 来源端点。
        url: String,
    },
    /// Legado 书源 JSON，只在 prepare 阶段短暂存在。
    Legado {
        /// 原始 Legado 书源 JSON。
        source_json: String,
    },
    /// 导入的 Rule Package JSON，只在 prepare 阶段短暂存在。
    Package {
        /// 原始 Rule Package JSON。
        source_json: String,
    },
}

impl fmt::Debug for RuleInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MaccmsJson { url } => formatter
                .debug_struct("MaccmsJson")
                .field("url_bytes", &url.len())
                .finish(),
            Self::Legado { source_json } => formatter
                .debug_struct("Legado")
                .field("source_json_bytes", &source_json.len())
                .finish(),
            Self::Package { source_json } => formatter
                .debug_struct("Package")
                .field("source_json_bytes", &source_json.len())
                .finish(),
        }
    }
}

/// 引用未安装能力的节点摘要。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnavailableNodeSummary {
    /// 节点稳定 ID。
    pub node_id: Uuid,
    /// 未安装能力的 wire kind。
    pub kind: String,
}

/// 任意 Rule Package 文件的 ingest preflight 结果。
///
/// 只读取 metadata、稳定身份与 schema，并报告 canonical Definition hash、校验诊断与
/// 引用未安装能力的节点；不 staging、不安装、不编译。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RulePackageInspection {
    /// package 声明的来源身份。
    pub source_id: SourceId,
    /// package 声明的版本。
    pub version: String,
    /// canonical Definition BLAKE3。
    pub definition_hash: String,
    /// Definition 校验诊断。
    pub diagnostics: Vec<Diagnostic>,
    /// 引用未安装能力的节点；只可展示与保存，不可 validate/compile/execute。
    pub unavailable_nodes: Vec<UnavailableNodeSummary>,
}

/// 只可作为 install token 传递的 opaque candidate ID。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CandidateId(Uuid);

impl CandidateId {
    pub(crate) const fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    pub(crate) const fn as_uuid(&self) -> Uuid {
        self.0
    }
}

/// 已安装来源的稳定身份。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceId(String);

impl SourceId {
    pub(crate) fn from_identity(identity: String) -> Self {
        Self(identity)
    }

    pub(crate) fn as_identity(&self) -> &str {
        &self.0
    }
}

/// 用户批准的 capability 集合。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CapabilityGrant(SystemCapabilities);

impl CapabilityGrant {
    /// 创建空 grant。
    ///
    /// 应用不授予任何系统能力（fs/env/process），因此这是唯一的生产 grant。
    #[must_use]
    pub fn none() -> Self {
        Self(SystemCapabilities::default())
    }

    pub(crate) fn from_policy(value: SystemCapabilities) -> Self {
        Self(value)
    }
}

/// 已 staging、尚未安装的安全 candidate 预览。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallCandidate {
    /// opaque install token。
    pub id: CandidateId,
    /// prepare 时固定的 installed revision。
    pub expected_installed_revision: u64,
    /// prepare 依据已安装状态判定的来源操作。
    pub operation: SourceOperation,
    /// 稳定来源资料。
    pub profile: SourceProfile,
    /// 最小 capability grant。
    pub required_grant: CapabilityGrant,
    /// importer、validator 与 compiler 诊断。
    pub diagnostics: Vec<Diagnostic>,
    /// canonical Definition BLAKE3。
    pub definition_hash: String,
    /// immutable Plan BLAKE3。
    pub plan_hash: String,
    /// UTC epoch milliseconds 到期时间。
    pub expires_at_ms: i64,
}

/// 来源操作: 由 prepare 依据已安装状态判定, 调用方据此区分安装与更新。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceOperation {
    /// 首次安装来源, 此前没有已安装 revision。
    Install,
    /// 更新已安装来源, 基线是 `expected_installed_revision`。
    Update,
}

impl SourceOperation {
    /// 基线 revision 只有已安装来源才有, 因此它本身就判定操作类型。
    pub(crate) fn from_installed_revision(expected_installed_revision: u64) -> Self {
        if expected_installed_revision == 0 {
            Self::Install
        } else {
            Self::Update
        }
    }
}

/// 已安装来源的安全摘要。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledSource {
    /// 稳定来源 ID。
    pub source_id: SourceId,
    /// 固定来源版本。
    pub version: String,
    /// 来源资料。
    pub profile: SourceProfile,
    /// 当前 source revision 已批准的 capability。
    pub grant: CapabilityGrant,
    /// source stream revision。
    pub revision: u64,
}

/// 已安装来源的不可变 revision 安全摘要。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRevisionSummary {
    /// 稳定来源 ID。
    pub source_id: SourceId,
    /// source stream revision。
    pub revision: u64,
    /// Definition/package version。
    pub version: String,
    /// 来源展示资料。
    pub profile: SourceProfile,
    /// 该 revision 安装时批准的 capability。
    pub grant: CapabilityGrant,
    /// canonical Definition BLAKE3。
    pub definition_hash: String,
    /// immutable Plan BLAKE3。
    pub plan_hash: String,
    /// 安装时刻。
    pub installed_at_ms: i64,
}
