//! Tauri command 共享状态。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use lj_rule_system::{ExecutionCancellation, ExecutionId, RuleSystem};

pub(super) type CancellationRegistry = Arc<Mutex<HashMap<ExecutionId, ExecutionCancellation>>>;

/// Tauri 共享状态；业务编排全部封装在 `RuleSystem` 内。
pub(crate) struct AppState {
    pub(super) system: Arc<RuleSystem>,
    pub(super) cancellations: CancellationRegistry,
}

impl AppState {
    #[must_use]
    pub(crate) fn new(system: Arc<RuleSystem>) -> Self {
        Self {
            system,
            cancellations: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}
