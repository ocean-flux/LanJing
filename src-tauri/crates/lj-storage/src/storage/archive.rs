//! runtime effect/control durable archive façade。

use lj_runtime::{
    ControlReplayLookup, ControlTraceCapture, ControlTraceReceipt, DurableCaptureReceipt,
    EffectCapture, EffectReplayLookup, ReplayCompletionLookup,
};

use super::EventProjectionStorage;
use crate::types::StorageError;
use crate::writer::WriterCommand;

impl EventProjectionStorage {
    pub(crate) async fn persist_effect_capture(
        &self,
        capture: EffectCapture,
    ) -> Result<DurableCaptureReceipt, StorageError> {
        self.dispatch(|reply| WriterCommand::PersistEffect { capture, reply })
            .await
    }

    pub(crate) async fn persist_control_trace(
        &self,
        capture: ControlTraceCapture,
    ) -> Result<ControlTraceReceipt, StorageError> {
        self.dispatch(|reply| WriterCommand::PersistControlTrace { capture, reply })
            .await
    }

    pub(crate) async fn replay_control_trace(
        &self,
        lookup: ControlReplayLookup,
    ) -> Result<Option<ControlTraceCapture>, StorageError> {
        self.read(move |conn, _| {
            Box::pin(
                async move { crate::repository::archive::load_control_trace(conn, &lookup).await },
            )
        })
        .await
    }

    pub(crate) async fn validate_replay_invocations(
        &self,
        lookup: ReplayCompletionLookup,
    ) -> Result<(), StorageError> {
        self.read(move |conn, _| {
            Box::pin(async move {
                crate::repository::archive::validate_replay_complete(conn, lookup).await
            })
        })
        .await
    }

    pub(crate) async fn replay_capture(
        &self,
        lookup: EffectReplayLookup,
    ) -> Result<Option<EffectCapture>, StorageError> {
        self.read(move |conn, artifacts| {
            Box::pin(async move {
                crate::repository::archive::load_effect_capture(conn, artifacts, &lookup).await
            })
        })
        .await
    }
}
