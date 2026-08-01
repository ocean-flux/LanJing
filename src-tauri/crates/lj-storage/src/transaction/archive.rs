//! Effect/control archive transaction service。

use lj_runtime::{ControlTraceCapture, ControlTraceReceipt, DurableCaptureReceipt, EffectCapture};

use crate::artifact::ArtifactStore;
use crate::database::DatabaseSession;
use crate::types::StorageError;

pub(crate) async fn persist_effect(
    connection: &DatabaseSession,
    artifacts: &ArtifactStore,
    capture: EffectCapture,
) -> Result<DurableCaptureReceipt, StorageError> {
    let artifacts = artifacts.clone();
    super::run(connection, move |transaction| {
        Box::pin(async move {
            crate::repository::archive::persist_effect_capture(transaction, &artifacts, capture)
                .await
        })
    })
    .await
}

pub(crate) async fn persist_control(
    connection: &DatabaseSession,
    capture: ControlTraceCapture,
) -> Result<ControlTraceReceipt, StorageError> {
    super::run(connection, move |transaction| {
        Box::pin(async move {
            crate::repository::archive::persist_control_trace(transaction, &capture).await
        })
    })
    .await
}
