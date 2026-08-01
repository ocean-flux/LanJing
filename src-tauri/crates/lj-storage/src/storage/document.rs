//! 原生规则文档生命周期与文档凭证 secret façade。
//!
//! 写操作经 single writer dispatch；list/get/provenance text 走只读 lane。

use super::EventProjectionStorage;
use crate::types::{
    ClearDocumentCredentialSecretRequest, CreateDocumentRequest, DeleteDocumentRequest,
    DocumentDetail, DocumentSummary, RenameDocumentRequest, SaveDocumentOutcome,
    SaveDocumentRequest, SecretArtifactId, StorageError, WriteDocumentCredentialSecretRequest,
};
use crate::writer::WriterCommand;

impl EventProjectionStorage {
    /// 创建原生规则文档（单个 writer transaction；可选初始 `semantic`/`layout` 与 provenance）。
    ///
    /// # Errors
    ///
    /// 输入不合法、文档或 `source_identity` 已存在、transaction 或 writer 失败时返回
    /// `StorageError`。
    pub async fn create_native_rule_document(
        &self,
        request: CreateDocumentRequest,
    ) -> Result<DocumentSummary, StorageError> {
        self.dispatch(|reply| WriterCommand::CreateNativeDocument { request, reply })
            .await
    }

    /// 保存文档内容：semantic/layout 分域乐观并发，仅有效域写新 revision，
    /// 返回分域 outcome（冲突域不写）；任一底层失败整体回滚。
    ///
    /// # Errors
    ///
    /// 文档不存在或 transaction/writer 失败时返回 `StorageError`。
    pub async fn save_native_rule_document(
        &self,
        request: SaveDocumentRequest,
    ) -> Result<SaveDocumentOutcome, StorageError> {
        self.dispatch(|reply| WriterCommand::SaveNativeDocument { request, reply })
            .await
    }

    /// 按创建时刻升序列出全部文档摘要（只读 lane）。
    ///
    /// # Errors
    ///
    /// 只读查询失败时返回 `StorageError`。
    pub async fn list_native_rule_documents(&self) -> Result<Vec<DocumentSummary>, StorageError> {
        self.read(|conn, _| {
            Box::pin(
                async move { crate::repository::document::list_document_summaries(conn).await },
            )
        })
        .await
    }

    /// 读取文档详情：主行 + 各域当前快照 + provenance 摘要（只读 lane）。
    ///
    /// # Errors
    ///
    /// 只读查询失败时返回 `StorageError`。
    pub async fn get_native_rule_document(
        &self,
        document_id: &str,
    ) -> Result<Option<DocumentDetail>, StorageError> {
        let document_id = document_id.to_string();
        self.read(move |conn, _| {
            Box::pin(async move {
                crate::repository::document::document_detail(conn, &document_id).await
            })
        })
        .await
    }

    /// 重命名文档；expected revision 不匹配时拒绝，标题不推进 semantic revision。
    ///
    /// # Errors
    ///
    /// 文档不存在、revision 冲突或 transaction/writer 失败时返回 `StorageError`。
    pub async fn rename_native_rule_document(
        &self,
        request: RenameDocumentRequest,
    ) -> Result<DocumentSummary, StorageError> {
        self.dispatch(|reply| WriterCommand::RenameNativeDocument { request, reply })
            .await
    }

    /// 删除文档；linked 状态需显式确认，删除时释放全部 secret owner。
    ///
    /// # Errors
    ///
    /// 文档不存在、linked 守卫拒绝或 transaction/writer 失败时返回 `StorageError`。
    pub async fn delete_native_rule_document(
        &self,
        request: DeleteDocumentRequest,
    ) -> Result<(), StorageError> {
        self.dispatch(|reply| WriterCommand::DeleteNativeDocument { request, reply })
            .await
    }

    /// 解密并返回 provenance 原文（只读 lane，显式只读；无 provenance 或未保存原文返回
    /// `None`）。脱敏由 `RuleSystem` 层负责。
    ///
    /// # Errors
    ///
    /// secret 认证/解密失败或只读查询失败时返回 `StorageError`。
    pub async fn get_native_rule_provenance_text(
        &self,
        document_id: &str,
    ) -> Result<Option<String>, StorageError> {
        let document_id = document_id.to_string();
        self.read(move |conn, artifacts| {
            Box::pin(async move {
                crate::repository::document::provenance_text(conn, artifacts, &document_id).await
            })
        })
        .await
    }

    /// 写入（或替换）一个文档凭证槽位 secret；返回随机 secret ID 供 manifest 引用。
    /// 替换同一槽位时先释放旧 owner 再写新（同一 writer transaction）。
    ///
    /// # Errors
    ///
    /// 文档不存在、输入不合法或 transaction/writer 失败时返回 `StorageError`。
    pub async fn write_document_credential_secret(
        &self,
        request: WriteDocumentCredentialSecretRequest,
    ) -> Result<SecretArtifactId, StorageError> {
        self.dispatch(|reply| WriterCommand::WriteDocumentCredentialSecret { request, reply })
            .await
    }

    /// 清除一个文档凭证槽位的 secret owner；不存在视为幂等成功。
    ///
    /// # Errors
    ///
    /// 文档不存在或 transaction/writer 失败时返回 `StorageError`。
    pub async fn clear_document_credential_secret(
        &self,
        request: ClearDocumentCredentialSecretRequest,
    ) -> Result<(), StorageError> {
        self.dispatch(|reply| WriterCommand::ClearDocumentCredentialSecret { request, reply })
            .await
    }
}
