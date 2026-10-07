//! Async gateway bridge for the synchronous budget SDK.
//!
//! Admission owns one slot until the reservation is settled, cancelled, or its
//! detached cleanup finishes. Dropping an HTTP future never drops an in-flight
//! Redis result: the worker still owns it and cancels an undelivered reservation.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use tokio::sync::{OwnedSemaphorePermit, Semaphore, oneshot};

use super::{
    ModelBudgetManager, ProviderBudgetManager, ResponseBudgetLeases, UnifiedBudgetLimits,
    UnifiedBudgetReservation,
};
use crate::core::budget::{BudgetReservationError, BudgetStatus};

/// Bounds queued work, active reservations, and their terminal cleanup together.
/// A reservation can own at most two drop-cancel tasks (provider and model).
const MAX_ASYNC_BUDGET_RESERVATIONS: usize = 1024;
const ASYNC_BUDGET_REPLY_TIMEOUT: Duration = Duration::from_secs(30);

async fn receive_with_timeout<T>(
    reply: oneshot::Receiver<T>,
    operation: &'static str,
    timeout: Duration,
) -> Result<T, BudgetReservationError> {
    match tokio::time::timeout(timeout, reply).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(_)) => Err(BudgetReservationError::BackendUnavailable),
        Err(_) => {
            // Closing this receiver does not cancel dispatched accounting.
            // The worker retains its original reservation and capacity slot.
            tracing::warn!(operation, "async budget reply deadline exceeded");
            Err(BudgetReservationError::BackendUnavailable)
        }
    }
}

async fn receive<T>(
    reply: oneshot::Receiver<T>,
    operation: &'static str,
) -> Result<T, BudgetReservationError> {
    receive_with_timeout(reply, operation, ASYNC_BUDGET_REPLY_TIMEOUT).await
}

pub(crate) struct AsyncBudgetPermit {
    _permit: OwnedSemaphorePermit,
}

pub(crate) fn try_budget_permit() -> Result<Arc<AsyncBudgetPermit>, BudgetReservationError> {
    static SLOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();
    let slots = SLOTS.get_or_init(|| Arc::new(Semaphore::new(MAX_ASYNC_BUDGET_RESERVATIONS)));
    Arc::clone(slots)
        .try_acquire_owned()
        .map(|permit| Arc::new(AsyncBudgetPermit { _permit: permit }))
        .map_err(|_| {
            tracing::warn!(
                capacity = MAX_ASYNC_BUDGET_RESERVATIONS,
                "async budget capacity is full; rejecting new work without queueing"
            );
            BudgetReservationError::BackendUnavailable
        })
}

fn budget_worker_handle() -> tokio::runtime::Handle {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .max_blocking_threads(32)
                .enable_all()
                .thread_name("budget-sdk")
                .build()
                .expect("budget SDK worker runtime")
        })
        .handle()
        .clone()
}

fn dispatch<T: Send + 'static>(
    permit: Arc<AsyncBudgetPermit>,
    work: impl FnOnce() -> T + Send + 'static,
) -> oneshot::Receiver<T> {
    let (tx, rx) = oneshot::channel();
    // Separate from the Redis I/O runtime: a full SDK pool must not starve
    // connection setup (including Tokio's blocking DNS resolver) on that runtime.
    budget_worker_handle().spawn_blocking(move || {
        let _permit = permit;
        // Settlement/cancellation must finish even when the caller goes away.
        let result = work();
        let _ = tx.send(result);
    });
    rx
}

impl ProviderBudgetManager {
    /// Reset a distributed budget without blocking a gateway admin handler.
    pub async fn reset_provider_budget_async(
        &self,
        provider: &str,
    ) -> Result<bool, BudgetReservationError> {
        if !self.backend.is_distributed() {
            return Ok(self.reset_provider_budget(provider));
        }
        let permit = try_budget_permit()?;
        let manager = self.clone();
        let provider = provider.to_owned();
        receive(
            dispatch(permit, move || manager.reset_provider_budget(&provider)),
            "reset_provider",
        )
        .await
    }
}

impl ModelBudgetManager {
    /// Reset a distributed budget without blocking a gateway admin handler.
    pub async fn reset_model_budget_async(
        &self,
        model: &str,
    ) -> Result<bool, BudgetReservationError> {
        if !self.backend.is_distributed() {
            return Ok(self.reset_model_budget(model));
        }
        let permit = try_budget_permit()?;
        let manager = self.clone();
        let model = model.to_owned();
        receive(
            dispatch(permit, move || manager.reset_model_budget(&model)),
            "reset_model",
        )
        .await
    }
}

impl UnifiedBudgetLimits {
    /// Reserve provider and model budgets without blocking an async executor.
    ///
    /// Saturation fails before enqueueing. Once dispatched, cancellation of the
    /// caller transfers responsibility to the worker, which cancels a successful
    /// reservation if its receiver has disappeared. The bridge does not retry
    /// ambiguous Redis writes.
    pub async fn reserve_spend_async(
        &self,
        provider: &str,
        model: &str,
        max_amount: f64,
    ) -> Result<UnifiedBudgetReservation, BudgetReservationError> {
        if !self.providers.backend.is_distributed() && !self.models.backend.is_distributed() {
            return self.reserve_spend(provider, model, max_amount);
        }
        let permit = try_budget_permit()?;
        let limits = self.clone();
        let provider = provider.to_owned();
        let model = model.to_owned();
        let (tx, rx) = oneshot::channel();
        budget_worker_handle().spawn_blocking(move || {
            // A queued request which was already cancelled never reaches Redis.
            if tx.is_closed() {
                return;
            }
            let result = limits.reserve_spend_with_permit(
                &provider,
                &model,
                max_amount,
                Some(Arc::clone(&permit)),
            );
            if let Err(Ok(reservation)) = tx.send(result) {
                // Keep the permit while both terminal operations run. A failed
                // cancellation also passes that permit to its existing fallback.
                reservation.cancel();
            }
        });
        receive(rx, "reserve").await?
    }

    pub(crate) async fn settle_response_leases_async(
        &self,
        provider: &str,
        model: &str,
        leases: &ResponseBudgetLeases,
        cost: f64,
    ) -> Result<(), BudgetReservationError> {
        let permit = try_budget_permit()?;
        let limits = self.clone();
        let provider = provider.to_owned();
        let model = model.to_owned();
        let leases = leases.clone();
        // SQL retains the receipt until acknowledgement. If the caller aborts
        // after dispatch, this worker finishes; the existing durable retry is
        // safe because settle_response is receipt-idempotent.
        receive(
            dispatch(permit, move || {
                limits.settle_response_leases(&provider, &model, &leases, cost)
            }),
            "settle_response",
        )
        .await?
    }
}

impl UnifiedBudgetReservation {
    fn has_distributed_lease(&self) -> bool {
        self.provider.lease_id.is_some() || self.model.lease_id.is_some()
    }

    fn async_permit(&mut self) -> Result<Arc<AsyncBudgetPermit>, BudgetReservationError> {
        let permit = self
            .provider
            .async_permit
            .as_ref()
            .or(self.model.async_permit.as_ref())
            .cloned()
            .map(Ok)
            .unwrap_or_else(try_budget_permit)?;
        // A synchronous SDK reservation entering an async terminal operation
        // must pass the newly acquired slot to any fallback Drop cleanup too.
        if self.provider.lease_id.is_some() {
            self.provider.async_permit = Some(Arc::clone(&permit));
        }
        if self.model.lease_id.is_some() {
            self.model.async_permit = Some(Arc::clone(&permit));
        }
        Ok(permit)
    }

    /// Settle on the budget worker, retaining ownership if the caller aborts.
    pub async fn settle_async(
        mut self,
        actual_amount: f64,
    ) -> Result<(Option<BudgetStatus>, Option<BudgetStatus>), BudgetReservationError> {
        if !self.has_distributed_lease() {
            return self.settle(actual_amount);
        }
        let permit = match self.async_permit() {
            Ok(permit) => permit,
            Err(error) => {
                // Only a reservation created with the synchronous SDK can lack
                // its own slot. Do not refund potentially billable work when
                // that legacy caller attempts async settlement under overload.
                self.provider.settled = true;
                self.model.settled = true;
                return Err(error);
            }
        };
        receive(
            dispatch(permit, move || self.settle(actual_amount)),
            "settle",
        )
        .await?
    }

    /// Cancel without blocking the async executor, even if this future is dropped.
    pub async fn cancel_async(mut self) -> Result<(), BudgetReservationError> {
        if !self.has_distributed_lease() {
            self.cancel();
            return Ok(());
        }
        let permit = self.async_permit()?;
        receive(dispatch(permit, move || self.cancel()), "cancel").await
    }

    /// Used by Realtime transport Drop: output may already have been consumed,
    /// so complete the conservative settlement rather than cancelling the hold.
    #[cfg(any(feature = "websockets", test))]
    pub(crate) fn settle_detached(mut self, actual_amount: f64) {
        if !self.has_distributed_lease() {
            if let Err(error) = self.settle(actual_amount) {
                tracing::error!(?error, "detached budget settlement failed");
            }
            return;
        }
        let permit = match self.async_permit() {
            Ok(permit) => permit,
            Err(error) => {
                self.provider.settled = true;
                self.model.settled = true;
                tracing::error!(?error, "detached budget settlement could not be admitted");
                return;
            }
        };
        drop(dispatch(permit, move || {
            if let Err(error) = self.settle(actual_amount) {
                tracing::error!(?error, "detached budget settlement failed");
            }
        }));
    }
}

#[cfg(test)]
mod deadline_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test(flavor = "current_thread")]
    async fn terminal_reply_timeout_retains_worker_capacity_until_accounting_finishes() {
        let slots = Arc::new(Semaphore::new(1));
        let permit = Arc::new(AsyncBudgetPermit {
            _permit: Arc::clone(&slots).try_acquire_owned().unwrap(),
        });
        let completed = Arc::new(AtomicUsize::new(0));
        let worker_completed = Arc::clone(&completed);
        let (entered, started) = oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let reply = dispatch(permit, move || {
            let _ = entered.send(());
            wait.recv_timeout(Duration::from_secs(5)).unwrap();
            worker_completed.fetch_add(1, Ordering::SeqCst);
        });
        started.await.unwrap();
        assert!(
            receive_with_timeout(reply, "settle", Duration::from_millis(25))
                .await
                .is_err()
        );
        assert_eq!(completed.load(Ordering::SeqCst), 0);
        assert_eq!(slots.available_permits(), 0);
        release.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while slots.available_permits() != 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(completed.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn expired_admission_receiver_prevents_a_queued_reserve() {
        let (queued, reply) = oneshot::channel::<()>();
        assert!(
            receive_with_timeout(reply, "reserve", Duration::from_millis(25))
                .await
                .is_err()
        );
        assert!(
            queued.is_closed(),
            "queued admission must observe expiration"
        );
    }
}
