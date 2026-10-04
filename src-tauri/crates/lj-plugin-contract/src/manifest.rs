//! plugin manifest 的稳定词汇。

use crate::identity::{OperationId, PluginId, Version};

/// 一个 plugin 对自身身份、host contract 与所提供 capability 的声明。
///
/// 本类型只承载声明与查询。manifest 校验、依赖解析、平台、冲突与资源限制检查，以及由 admitted
/// 集合生成 frozen plugin lock，属于 `#36`；本类型不代替那些校验做准入判断。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginManifest {
    plugin_id: PluginId,
    plugin_version: Version,
    contract_version: Version,
    provided: Vec<OperationId>,
}

impl PluginManifest {
    /// 创建 manifest 声明。
    #[must_use]
    pub fn new(
        plugin_id: PluginId,
        plugin_version: Version,
        contract_version: Version,
        provided: Vec<OperationId>,
    ) -> Self {
        Self {
            plugin_id,
            plugin_version,
            contract_version,
            provided,
        }
    }

    /// plugin identity。
    #[must_use]
    pub fn plugin_id(&self) -> &PluginId {
        &self.plugin_id
    }

    /// plugin 自身版本。
    #[must_use]
    pub fn plugin_version(&self) -> &Version {
        &self.plugin_version
    }

    /// plugin 编译时使用的 host contract version。
    #[must_use]
    pub fn contract_version(&self) -> &Version {
        &self.contract_version
    }

    /// manifest 声明的 operation 列表。
    #[must_use]
    pub fn provided(&self) -> &[OperationId] {
        &self.provided
    }

    /// 是否声明提供某个 operation。
    #[must_use]
    pub fn provides(&self, operation: &OperationId) -> bool {
        self.provided.contains(operation)
    }
}
