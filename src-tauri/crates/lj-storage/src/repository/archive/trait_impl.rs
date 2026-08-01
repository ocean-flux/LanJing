#[async_trait]
impl EffectArchive for EventProjectionStorage {
    async fn persist_durable(
        &self,
        capture: EffectCapture,
    ) -> Result<DurableCaptureReceipt, EffectArchiveError> {
        self.persist_effect_capture(capture)
            .await
            .map_err(|error| effect_archive_error(&error))
    }

    async fn load_replay(
        &self,
        lookup: EffectReplayLookup,
    ) -> Result<Option<EffectCapture>, EffectArchiveError> {
        self.replay_capture(lookup)
            .await
            .map_err(|error| effect_archive_error(&error))
    }

    async fn persist_control_trace(
        &self,
        capture: ControlTraceCapture,
    ) -> Result<ControlTraceReceipt, EffectArchiveError> {
        EventProjectionStorage::persist_control_trace(self, capture)
            .await
            .map_err(|error| effect_archive_error(&error))
    }

    async fn load_control_trace(
        &self,
        lookup: ControlReplayLookup,
    ) -> Result<Option<ControlTraceCapture>, EffectArchiveError> {
        self.replay_control_trace(lookup)
            .await
            .map_err(|error| effect_archive_error(&error))
    }

    async fn validate_replay_complete(
        &self,
        lookup: ReplayCompletionLookup,
    ) -> Result<(), EffectArchiveError> {
        self.validate_replay_invocations(lookup)
            .await
            .map_err(|error| effect_archive_error(&error))
    }
}

fn effect_archive_error(error: &StorageError) -> EffectArchiveError {
    match error {
        StorageError::ReplayUnavailable(_)
        | StorageError::IdempotencyMismatch
        | StorageError::InvalidInput(_)
        | StorageError::ArtifactCorrupt
        | StorageError::Serialization => EffectArchiveError::with_code(
            EffectArchiveErrorCode::Integrity,
            "execution archive 完整性校验失败",
        ),
        StorageError::SecretUnavailable
        | StorageError::KeyringLocked
        | StorageError::KeyringUnavailable
        | StorageError::KeyLost => {
            EffectArchiveError::new("secret artifact 不可用，历史 execution 不能 replay")
        }
        StorageError::ArtifactUnavailable(_) => {
            EffectArchiveError::new("body artifact 缺失，历史 execution 不能 replay")
        }
        _ => EffectArchiveError::new("effect archive 持久化或读取失败"),
    }
}
