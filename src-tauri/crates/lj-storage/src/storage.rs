//! `EventProjectionStorage` 公开 facade 与 blocking-lane 边界。
//!
//! 此文件只持有具体 storage state、公开 API、single-writer dispatch 和独立 read lane；Event
//! transaction、candidate/install、execution/archive、projection/query、checkpoint/GC/recovery 各由
//! sibling 深模块实现。外部仍只看到 concrete `EventProjectionStorage` 与既有 re-export，不能取得
//! Diesel connection、writer command 或内部 helper。

use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use diesel::sqlite::SqliteConnection;
use lj_media::{MediaAsset, MediaItem, MediaResourceId, MediaUnit};
use lj_runtime::{DurableCaptureReceipt, EffectCapture, EffectReplayLookup};
use tokio::sync::{Semaphore, mpsc, oneshot};
use uuid::Uuid;

use crate::artifact::ArtifactStore;
use crate::candidate_install::{
    get_candidate_source_credentials_ref_sync, get_candidate_summary, get_installed_source_sync,
    list_installed_sources_sync,
};
use crate::connection::{open_connection, validate_config};
use crate::execution::{
    events_after_source_sync, events_after_stream_sync, execution_stream_id, get_execution_sync,
    load_execution_replay_pin_sync, load_execution_source_credentials_sync,
};
use crate::projection_query::{
    get_library_entry_sync, get_payload_by_id, library_projection_sync, payloads_by_column,
    source_projection_sync,
};
use crate::retention_recovery::{
    backfill_source_version_snapshots, load_library_checkpoint_sync, load_source_checkpoint_sync,
    mark_interrupted_executions, normalize_artifact_relative_paths, recover_orphans_sync,
    recover_source_snapshot_sync,
};
use crate::types::{
    AppendRequest, CandidateDraft, CandidateSummary, CheckpointReceipt, CommitReceipt, DeltaCommit,
    ExecutionFinish, ExecutionPin, ExecutionRecord, ExecutionReplayPin, ExecutionSourceCredentials,
    ExecutionStart, GcReport, InstallCandidateRequest, InstalledSource, InstalledSourceRecord,
    LibraryEntry, LibraryProjection, LibraryProjectionSnapshot, LibraryUpdate, OrphanRecovery,
    ReplayExecutionStart, RetentionPolicy, SourceCredentialInput, SourceCredentialSnapshot,
    SourceProjectionSnapshot, SourceProjectionView, StorageConfig, StorageError, StoredEvent,
    WRITER_CAPACITY,
};
use crate::writer::{WriterCommand, writer_loop};

/// C4 使用的具体 Event Store。
///
/// 克隆值只克隆 Tokio writer sender 和只读配置；不会复制 `SQLite` 连接或在外层叠加
/// `Mutex`。所有同步 I/O 均由 writer 专用线程或短生命周期 `spawn_blocking` read lane 完成。
#[derive(Clone)]
pub struct EventProjectionStorage {
    writer: mpsc::Sender<WriterCommand>,
    database_path: PathBuf,
    artifacts: ArtifactStore,
    read_permits: Arc<Semaphore>,
}

impl EventProjectionStorage {
    /// 在 blocking lane 初始化真实 `SQLite` 文件、migration、artifact recovery 与单 writer。
    ///
    /// # Errors
    ///
    /// 数据库、artifact 根目录、migration 或 writer 启动失败时返回 [`StorageError`]。
    pub async fn open(config: StorageConfig) -> Result<Self, StorageError> {
        tokio::task::spawn_blocking(move || Self::open_blocking(config))
            .await
            .map_err(|_| StorageError::WriterUnavailable)?
    }

    /// 同步初始化入口，仅供已经位于 blocking lane 的装配和测试使用。
    ///
    /// async 调用方必须使用 [`Self::open`]，以免阻塞 Tokio worker。
    ///
    /// # Errors
    ///
    /// 数据库、artifact 根目录、migration 或 writer 启动失败时返回 [`StorageError`]。
    pub fn open_blocking(config: StorageConfig) -> Result<Self, StorageError> {
        validate_config(&config)?;
        let mut writer_connection = open_connection(&config.database_path, true)?;
        let artifacts = ArtifactStore::new(config.artifact_root, &config.keyring_service)?;
        normalize_artifact_relative_paths(&mut writer_connection)?;
        backfill_source_version_snapshots(&mut writer_connection, &artifacts)?;
        recover_orphans_sync(&mut writer_connection, &artifacts)?;
        mark_interrupted_executions(&mut writer_connection)?;

        let (writer, receiver) = mpsc::channel(WRITER_CAPACITY);
        let writer_artifacts = artifacts.clone();
        thread::Builder::new()
            .name("lanjing-sqlite-writer".to_string())
            .spawn(move || writer_loop(receiver, writer_connection, &writer_artifacts))
            .map_err(|error| StorageError::FileSystem(error.to_string()))?;

        Ok(Self {
            writer,
            database_path: config.database_path,
            artifacts,
            read_permits: Arc::new(Semaphore::new(config.read_concurrency)),
        })
    }

    /// 返回唯一 writer 的固定队列容量。
    #[must_use]
    pub const fn writer_capacity(&self) -> usize {
        WRITER_CAPACITY
    }

    /// 追加一个不影响投影的领域事件。
    ///
    /// # Errors
    ///
    /// expected version 冲突、artifact durable 写入、SQLite transaction 或 writer 停止时返回
    /// [`StorageError`]。成功返回时 Event 及其 artifact ref 已 durable 提交。
    pub async fn append_event(
        &self,
        request: AppendRequest,
    ) -> Result<CommitReceipt, StorageError> {
        self.dispatch(|reply| WriterCommand::Append { request, reply })
            .await
    }

    /// 将 opaque candidate、作者包和 immutable Plan durable staging。
    ///
    /// # Errors
    ///
    /// candidate 输入不一致、artifact/SQLite 写入失败或 writer 停止时返回 [`StorageError`]。
    pub async fn stage_candidate(
        &self,
        draft: CandidateDraft,
    ) -> Result<CandidateSummary, StorageError> {
        self.dispatch(|reply| WriterCommand::StageCandidate {
            draft: Box::new(draft),
            reply,
        })
        .await
    }

    /// 将 source-owned credential snapshot 加密为可由安装原子消费的 Secret Artifact。
    ///
    /// 返回值仅含 namespace 与 Secret Artifact ref；凭证明文不会写入 Event、投影或 body
    /// artifact。
    ///
    /// 成功后 candidate 被标记为需要该 snapshot；因此 staging 行在安装前丢失会显式失败，而不会
    /// 被误判为 credential-free。
    ///
    /// # Errors
    ///
    /// 输入无效、keyring/文件系统/`SQLite` 写入失败或 writer 停止时返回 [`StorageError`]。
    pub async fn stage_source_credentials(
        &self,
        input: SourceCredentialInput,
    ) -> Result<SourceCredentialSnapshot, StorageError> {
        self.dispatch(|reply| WriterCommand::StageSourceCredentials {
            input: Box::new(input),
            reply,
        })
        .await
    }

    /// 读取绑定到 opaque candidate 的 durable source credential ref。
    ///
    /// 此方法只返回 cookie namespace 和 Secret Artifact ref，绝不解密或返回凭证明文；它让
    /// prepare 与 install 之间的进程重启无需回退到 live credential。
    ///
    /// # Errors
    ///
    /// `SQLite` 读取失败、staging metadata 损坏，或已标记为需要 credential 的 staged candidate
    /// 缺少其 ref 时返回 [`StorageError`]。只有 credential-free candidate 才返回 `Ok(None)`。
    pub async fn get_candidate_source_credentials_ref(
        &self,
        candidate_id: Uuid,
    ) -> Result<Option<SourceCredentialSnapshot>, StorageError> {
        self.read(move |conn, artifacts| {
            get_candidate_source_credentials_ref_sync(conn, artifacts, candidate_id)
        })
        .await
    }

    /// 读取 opaque candidate 的安全预览；不会读取或返回作者包、Definition 或 Plan JSON。
    ///
    /// # Errors
    ///
    /// `SQLite` 或 preview JSON 损坏时返回 [`StorageError`]。
    pub async fn get_candidate_summary(
        &self,
        candidate_id: Uuid,
    ) -> Result<Option<CandidateSummary>, StorageError> {
        self.read(move |conn, _artifacts| get_candidate_summary(conn, candidate_id))
            .await
    }
    /// 原子消费 candidate 并安装 source version、package、Plan、grant 与 profile。
    ///
    /// # Errors
    ///
    /// candidate 缺失、过期、已消费、source expected-version 冲突或持久化失败时返回
    /// [`StorageError`]。
    pub async fn install_candidate(
        &self,
        request: InstallCandidateRequest,
    ) -> Result<InstalledSource, StorageError> {
        self.dispatch(|reply| WriterCommand::InstallCandidate { request, reply })
            .await
    }

    /// 读取一个已安装来源以及其 immutable package/Plan。
    ///
    /// # Errors
    ///
    /// SQLite、artifact 或 JSON 读取失败时返回 [`StorageError`]。
    pub async fn get_installed_source(
        &self,
        source_identity: impl Into<String>,
    ) -> Result<Option<InstalledSource>, StorageError> {
        let source_identity = source_identity.into();
        self.read(move |conn, artifacts| {
            get_installed_source_sync(conn, artifacts, &source_identity)
        })
        .await
    }

    /// 按稳定 source identity 升序读取所有已安装来源的安全投影记录。
    ///
    /// 返回记录不含作者包、Definition、Plan、artifact ref 或 secret；需要 immutable Plan 的内部
    /// 生命周期路径必须使用 [`Self::get_installed_source`]，不得把它暴露给查询 facade。
    ///
    /// # Errors
    ///
    /// `SQLite`、投影 JSON 损坏或 source ownership 不一致时返回 [`StorageError`]。
    pub async fn list_installed_sources(&self) -> Result<Vec<InstalledSourceRecord>, StorageError> {
        self.read(move |conn, _| list_installed_sources_sync(conn))
            .await
    }

    /// 创建 execution archive，并把当前 source version 的 immutable Plan 引用固定下来。
    ///
    /// # Errors
    ///
    /// 来源不存在、execution ID 冲突、artifact ref/SQLite transaction 失败时返回
    /// [`StorageError`]。
    pub async fn start_execution(
        &self,
        request: ExecutionStart,
    ) -> Result<ExecutionRecord, StorageError> {
        self.dispatch(|reply| WriterCommand::StartExecution { request, reply })
            .await
    }

    /// 基于已验证历史 pin 建立新的 replay execution archive，不读取当前 source 配置。
    ///
    /// # Errors
    ///
    /// 输入 pin 与 archive 不一致、archive 已 GC、artifact 缺失/篡改、mode 非法或写入失败时
    /// 返回 [`StorageError`]。
    pub async fn start_replay_execution(
        &self,
        request: ReplayExecutionStart,
    ) -> Result<ExecutionRecord, StorageError> {
        self.dispatch(|reply| WriterCommand::StartReplayExecution {
            request: Box::new(request),
            reply,
        })
        .await
    }

    /// 在同一 transaction 内写入 execution Event 与 O(delta) 规范化资源投影。
    ///
    /// # Errors
    ///
    /// execution 不存在、expected version 冲突、资源来源不匹配或 `SQLite` transaction 失败时
    /// 返回 [`StorageError`]；失败时 Event 与 projection 均不会半提交。
    pub async fn commit_execution_delta(
        &self,
        request: DeltaCommit,
    ) -> Result<CommitReceipt, StorageError> {
        self.dispatch(|reply| WriterCommand::CommitDelta {
            request: Box::new(request),
            reply,
        })
        .await
    }

    /// 将 execution 推进到唯一终态。
    ///
    /// # Errors
    ///
    /// execution 不存在、expected version 冲突、终态非法或持久化失败时返回
    /// [`StorageError`]。
    pub async fn finish_execution(
        &self,
        request: ExecutionFinish,
    ) -> Result<ExecutionRecord, StorageError> {
        self.dispatch(|reply| WriterCommand::FinishExecution { request, reply })
            .await
    }

    /// 修改 execution archive pin；pin archive 不参与 TTL/容量自动回收。
    ///
    /// # Errors
    ///
    /// execution 不存在、expected version 冲突或持久化失败时返回 [`StorageError`]。
    pub async fn set_execution_pin(
        &self,
        request: ExecutionPin,
    ) -> Result<ExecutionRecord, StorageError> {
        self.dispatch(|reply| WriterCommand::SetExecutionPin { request, reply })
            .await
    }

    /// 读取 execution summary。
    ///
    /// # Errors
    ///
    /// `SQLite` 读取失败或状态损坏时返回 [`StorageError`]。
    pub async fn get_execution(
        &self,
        execution_id: Uuid,
    ) -> Result<Option<ExecutionRecord>, StorageError> {
        self.read(move |conn, _| get_execution_sync(conn, execution_id))
            .await
    }

    /// 为 live execution 解密其固定 source-version credential snapshot。
    ///
    /// 返回值不实现序列化或 `Debug`，只能在 live 执行适配器内短暂使用；该方法绝不会读取当前
    /// source 配置。replay execution 绝不接收静态 source credential，并会被显式拒绝。
    ///
    /// # Errors
    ///
    /// execution/source version 缺失、Secret Artifact 缺失或篡改、主密钥不可用，或调用方尝试为
    /// replay execution 读取 credential 时返回 [`StorageError`]。
    pub async fn load_execution_source_credentials(
        &self,
        execution_id: Uuid,
    ) -> Result<ExecutionSourceCredentials, StorageError> {
        self.read(move |conn, artifacts| {
            load_execution_source_credentials_sync(conn, artifacts, execution_id)
        })
        .await
    }

    /// 读取 execution 固定的 immutable Plan replay pin。
    ///
    /// 成功返回的 [`ExecutionReplayPin`] 已验证 body artifact 的 BLAKE3、Plan artifact ref 与
    /// Plan canonical hash。若 source version 有静态 credential ref，此方法只验证其密文文件、
    /// metadata 与安装级主密钥可用，绝不解密或把 bytes 放入 pin；调用方必须使用其 `plan` 与
    /// `mode`，不得回退到当前安装版本。
    ///
    /// # Errors
    ///
    /// execution 不存在、archive 已 GC、artifact 缺失/篡改、主密钥不可用或 Plan 内容不一致时返回
    /// [`StorageError`]。
    pub async fn load_execution_replay_pin(
        &self,
        execution_id: Uuid,
    ) -> Result<ExecutionReplayPin, StorageError> {
        self.read(move |conn, artifacts| {
            load_execution_replay_pin_sync(conn, artifacts, execution_id)
        })
        .await
    }

    /// 从指定 execution stream version 之后补读持久事件。
    ///
    /// # Errors
    ///
    /// `SQLite` 或 JSON 读取失败时返回 [`StorageError`]。
    pub async fn catch_up_execution(
        &self,
        execution_id: Uuid,
        after_version: u64,
    ) -> Result<Vec<StoredEvent>, StorageError> {
        self.read(move |conn, _| {
            events_after_stream_sync(conn, &execution_stream_id(execution_id), after_version)
        })
        .await
    }

    /// 更新资料库唯一拥有的用户状态，并同步追加 library Event。
    ///
    /// # Errors
    ///
    /// expected version 冲突、SQLite transaction 或 writer 停止时返回 [`StorageError`]。
    pub async fn update_library(
        &self,
        request: LibraryUpdate,
    ) -> Result<CommitReceipt, StorageError> {
        self.dispatch(|reply| WriterCommand::UpdateLibrary { request, reply })
            .await
    }

    /// 在同一只读 `SQLite` transaction 中读取完整资料库投影。
    ///
    /// 结果含全局 revision、按资源稳定 ID 升序的用户拥有条目及每条目的 optimistic revision，
    /// 可直接配合既有 [`Self::update_library`] 写回；不包含媒体 Graph、规则 Plan 或 secret。
    ///
    /// # Errors
    ///
    /// `SQLite`、资料库 progress JSON 或 stream revision 损坏时返回 [`StorageError`]。
    pub async fn get_library_projection(&self) -> Result<LibraryProjection, StorageError> {
        self.read(move |conn, _| library_projection_sync(conn))
            .await
    }

    /// 读取单个资料库用户状态。
    ///
    /// # Errors
    ///
    /// `SQLite` 或 JSON 读取失败时返回 [`StorageError`]。
    pub async fn get_library_entry(
        &self,
        resource_id: MediaResourceId,
    ) -> Result<Option<LibraryEntry>, StorageError> {
        self.read(move |conn, _| get_library_entry_sync(conn, &resource_id.0))
            .await
    }

    /// 按来源读取规范化投影，不会读取或重写整张 JSON graph。
    ///
    /// # Errors
    ///
    /// `SQLite` 或任一规范化资源 JSON 读取失败时返回 [`StorageError`]。
    pub async fn source_projection(
        &self,
        source_identity: impl Into<String>,
    ) -> Result<SourceProjectionView, StorageError> {
        let source_identity = source_identity.into();
        self.read(move |conn, _| source_projection_sync(conn, &source_identity))
            .await
    }

    /// 按稳定 ID 查询媒体主体。
    ///
    /// # Errors
    ///
    /// `SQLite` 或 JSON 读取失败时返回 [`StorageError`]。
    pub async fn get_item(
        &self,
        resource_id: MediaResourceId,
    ) -> Result<Option<MediaItem>, StorageError> {
        self.read(move |conn, _| get_payload_by_id(conn, "projection_items", "id", &resource_id.0))
            .await
    }

    /// 按来源稳定索引查询媒体主体。
    ///
    /// # Errors
    ///
    /// `SQLite` 或 JSON 读取失败时返回 [`StorageError`]。
    pub async fn list_items_by_source(
        &self,
        source_identity: impl Into<String>,
    ) -> Result<Vec<MediaItem>, StorageError> {
        let source_identity = source_identity.into();
        self.read(move |conn, _| {
            payloads_by_column(
                conn,
                "projection_items",
                "source_identity",
                &source_identity,
            )
        })
        .await
    }

    /// 按媒体主体索引查询消费单元。
    ///
    /// # Errors
    ///
    /// `SQLite` 或 JSON 读取失败时返回 [`StorageError`]。
    pub async fn list_units_for_item(
        &self,
        item_id: MediaResourceId,
    ) -> Result<Vec<MediaUnit>, StorageError> {
        self.read(move |conn, _| {
            payloads_by_column(conn, "projection_units", "item_id", &item_id.0)
        })
        .await
    }

    /// 按消费单元索引查询资产。
    ///
    /// # Errors
    ///
    /// `SQLite` 或 JSON 读取失败时返回 [`StorageError`]。
    pub async fn list_assets_for_unit(
        &self,
        unit_id: MediaResourceId,
    ) -> Result<Vec<MediaAsset>, StorageError> {
        self.read(move |conn, _| {
            payloads_by_column(conn, "projection_assets", "unit_id", &unit_id.0)
        })
        .await
    }

    /// 显式扫描并删除 temp/无 metadata 的 artifact orphan。
    ///
    /// # Errors
    ///
    /// `SQLite` 或文件系统扫描失败时返回 [`StorageError`]。
    pub async fn recover_orphans(&self) -> Result<OrphanRecovery, StorageError> {
        self.dispatch(WriterCommand::RecoverOrphans).await
    }

    /// 为单一来源 aggregate 创建并验证压缩 checkpoint；不会复制整个 `SQLite` 文件。
    ///
    /// # Errors
    ///
    /// 来源不存在、artifact/SQLite 写入失败或 snapshot 验证失败时返回 [`StorageError`]。
    pub async fn checkpoint_source(
        &self,
        source_identity: impl Into<String>,
        created_at_ms: i64,
    ) -> Result<CheckpointReceipt, StorageError> {
        self.dispatch(|reply| WriterCommand::CheckpointSource {
            source_identity: source_identity.into(),
            created_at_ms,
            reply,
        })
        .await
    }

    /// 为 library aggregate 创建并验证小型 checkpoint。
    ///
    /// # Errors
    ///
    /// artifact/SQLite 写入失败或 snapshot 验证失败时返回 [`StorageError`]。
    pub async fn checkpoint_library(
        &self,
        created_at_ms: i64,
    ) -> Result<CheckpointReceipt, StorageError> {
        self.dispatch(|reply| WriterCommand::CheckpointLibrary {
            created_at_ms,
            reply,
        })
        .await
    }

    /// 读取最新来源 checkpoint。
    ///
    /// # Errors
    ///
    /// SQLite、artifact 或 JSON 读取失败时返回 [`StorageError`]。
    pub async fn load_source_checkpoint(
        &self,
        source_identity: impl Into<String>,
    ) -> Result<Option<SourceProjectionSnapshot>, StorageError> {
        let source_identity = source_identity.into();
        self.read(move |conn, artifacts| {
            load_source_checkpoint_sync(conn, artifacts, &source_identity)
        })
        .await
    }

    /// 读取最新 library checkpoint。
    ///
    /// # Errors
    ///
    /// SQLite、artifact 或 JSON 读取失败时返回 [`StorageError`]。
    pub async fn load_library_checkpoint(
        &self,
    ) -> Result<Option<LibraryProjectionSnapshot>, StorageError> {
        self.read(load_library_checkpoint_sync).await
    }

    /// 读取某来源 checkpoint 后的持久事件，供外部 catch-up/rebuild 使用。
    ///
    /// # Errors
    ///
    /// `SQLite` 或 JSON 读取失败时返回 [`StorageError`]。
    pub async fn source_events_after(
        &self,
        source_identity: impl Into<String>,
        after_global_seq: u64,
    ) -> Result<Vec<StoredEvent>, StorageError> {
        let source_identity = source_identity.into();
        self.read(move |conn, _| events_after_source_sync(conn, &source_identity, after_global_seq))
            .await
    }

    /// 用 checkpoint 与其后的 source execution Delta 在内存中重建来源当前状态。
    ///
    /// # Errors
    ///
    /// 缺少 checkpoint、事件 payload 损坏或 SQLite/artifact 读取失败时返回 [`StorageError`]。
    pub async fn recover_source_from_checkpoint(
        &self,
        source_identity: impl Into<String>,
    ) -> Result<SourceProjectionSnapshot, StorageError> {
        let source_identity = source_identity.into();
        self.read(move |conn, artifacts| {
            recover_source_snapshot_sync(conn, artifacts, &source_identity)
        })
        .await
    }

    /// 立即清理一个终态 execution archive，并复用 checkpoint 与两阶段 GC 状态机。
    ///
    /// 未 pin archive 可直接清理；pin archive 只有在 `confirm_pinned` 为 `true` 时才允许清理。
    ///
    /// # Errors
    ///
    /// execution 不存在、仍在运行、pin 未确认，或 checkpoint/artifact/SQLite 阶段失败时返回
    /// [`StorageError`]。
    pub async fn clear_execution_archive(
        &self,
        execution_id: Uuid,
        confirm_pinned: bool,
        now_ms: i64,
    ) -> Result<GcReport, StorageError> {
        self.dispatch(|reply| WriterCommand::ClearExecutionArchive {
            execution_id,
            confirm_pinned,
            now_ms,
            reply,
        })
        .await
    }

    /// 执行 candidate staging 与 execution archive 的策略 B 回收。
    ///
    /// 未 pin 的过期 archive 或使用量达到配额 90% 时会依次经历
    /// `marked → external_refs_removed → finalized`；pin archive 不参与自动回收。
    ///
    /// # Errors
    ///
    /// checkpoint、artifact/keyring、SQLite 或文件删除失败时返回 [`StorageError`]；已进入的
    /// 状态会在下次调用幂等继续，而不会静默标记为完成。
    pub async fn run_gc(
        &self,
        policy: RetentionPolicy,
        now_ms: i64,
    ) -> Result<GcReport, StorageError> {
        self.dispatch(|reply| WriterCommand::RunGc {
            policy,
            now_ms,
            reply,
        })
        .await
    }

    /// 有序关闭单 writer，测试在删除 temp `SQLite` 文件前应调用此方法。
    ///
    /// # Errors
    ///
    /// writer 已停止或无法完成已接收命令时返回 [`StorageError`]。
    pub async fn shutdown(&self) -> Result<(), StorageError> {
        self.dispatch(WriterCommand::Shutdown).await
    }

    async fn dispatch<T: Send + 'static>(
        &self,
        build: impl FnOnce(oneshot::Sender<Result<T, StorageError>>) -> WriterCommand,
    ) -> Result<T, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.writer
            .send(build(reply))
            .await
            .map_err(|_| StorageError::WriterClosed)?;
        receiver
            .await
            .map_err(|_| StorageError::WriterUnavailable)?
    }

    async fn read<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut SqliteConnection, &ArtifactStore) -> Result<T, StorageError>
        + Send
        + 'static,
    ) -> Result<T, StorageError> {
        let permit = self
            .read_permits
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| StorageError::WriterClosed)?;
        let database_path = self.database_path.clone();
        let artifacts = self.artifacts.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let mut conn = open_connection(&database_path, false)?;
            operation(&mut conn, &artifacts)
        })
        .await
        .map_err(|_| StorageError::WriterUnavailable)?
    }

    pub(crate) async fn persist_effect_capture(
        &self,
        capture: EffectCapture,
    ) -> Result<DurableCaptureReceipt, StorageError> {
        self.dispatch(|reply| WriterCommand::PersistEffect { capture, reply })
            .await
    }

    pub(crate) async fn replay_capture(
        &self,
        lookup: EffectReplayLookup,
    ) -> Result<Option<EffectCapture>, StorageError> {
        self.read(move |conn, artifacts| {
            crate::execution_archive::load_effect_capture(conn, artifacts, &lookup)
        })
        .await
    }
}
