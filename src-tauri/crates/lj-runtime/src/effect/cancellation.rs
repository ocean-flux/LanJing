//! execution 与 effect 的协作式取消原语。
//!
//! 取消首先阻止新 effect 调度；已经进入 archive 事务的 capture 必须自行完成 commit 或
//! rollback，不能在中间丢弃已发生外部副作用。token 只传递状态，不携带 HTTP client 或
//! `QuickJS` 非 `Send` 句柄，因此可安全跨 blocking lane 使用。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::watch;

/// 可由执行会话持有的幂等取消句柄。
#[derive(Clone)]
pub struct CancellationHandle {
    cancellation: EffectCancellation,
}

impl CancellationHandle {
    /// 创建尚未取消的句柄。
    #[must_use]
    pub fn new() -> Self {
        let (sender, _receiver) = watch::channel(false);
        Self {
            cancellation: EffectCancellation {
                state: Arc::new(CancellationState {
                    cancelled: AtomicBool::new(false),
                    sender,
                }),
            },
        }
    }

    /// 请求取消，并返回本次调用是否首次改变取消状态。
    #[must_use]
    pub fn cancel(&self) -> bool {
        if self
            .cancellation
            .state
            .cancelled
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            self.cancellation.state.sender.send_replace(true);
            true
        } else {
            false
        }
    }

    /// 返回 effect 可传递的只读取消 token。
    #[must_use]
    pub fn token(&self) -> EffectCancellation {
        self.cancellation.clone()
    }

    /// 返回当前是否已经请求取消。
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }
}

impl Default for CancellationHandle {
    fn default() -> Self {
        Self::new()
    }
}

struct CancellationState {
    cancelled: AtomicBool,
    sender: watch::Sender<bool>,
}

/// 传给实际 effect handler 的取消 token。
#[derive(Clone)]
pub struct EffectCancellation {
    state: Arc<CancellationState>,
}

impl EffectCancellation {
    /// 返回当前是否已经请求取消。
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.state.cancelled.load(Ordering::Acquire)
    }

    /// 等待取消请求。
    pub async fn cancelled(&self) {
        let mut receiver = self.state.sender.subscribe();
        while !*receiver.borrow() {
            if receiver.changed().await.is_err() {
                return;
            }
        }
    }
}
