use std::future::Future;

use tokio_util::sync::CancellationToken;

use crate::io::scheduler::{ScheduleError, Scheduler};

/// Multi-threading with Tokio.
///
/// CrowdedWX: every task this scheduler spawns is cancelled when the scheduler is dropped. Each
/// map owns its schedulers, so throwing a map away (Refresh radar rebuilds it) stops its pending
/// tile downloads and tessellation instead of letting hundreds of them finish for nothing.
pub struct TokioScheduler {
    cancel: CancellationToken,
}

impl TokioScheduler {
    pub fn new() -> Self {
        Self {
            cancel: CancellationToken::new(),
        }
    }
}

impl Drop for TokioScheduler {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl Scheduler for TokioScheduler {
    #[cfg(feature = "thread-safe-futures")]
    fn schedule<T>(
        &self,
        future_factory: impl FnOnce() -> T + Send + 'static,
    ) -> Result<(), ScheduleError>
    where
        T: Future<Output = ()> + Send + 'static,
    {
        let cancel = self.cancel.clone();
        if cancel.is_cancelled() {
            return Ok(());
        }
        tokio::task::spawn(async move {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => {}
                _ = (future_factory)() => {}
            }
        });
        Ok(())
    }

    // FIXME: Provide a working implementation
    #[cfg(not(feature = "thread-safe-futures"))]
    fn schedule<T>(
        &self,
        _future_factory: impl FnOnce() -> T + 'static,
    ) -> Result<(), ScheduleError>
    where
        T: Future<Output = ()> + 'static,
    {
        Ok(())
    }
}

impl Default for TokioScheduler {
    fn default() -> Self {
        Self::new()
    }
}
