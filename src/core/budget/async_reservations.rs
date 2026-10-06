//! Async gateway bridge for the synchronous budget SDK.
//!
//! Admission owns one slot until the reservation is settled, cancelled, or its
//! detached cleanup finishes. Dropping an HTTP future never drops an in-flight
//! Redis result: the worker still owns it and cancels an undelivered reservation.

use std::sync::{Arc, OnceLock};

use tokio::sync::{OwnedSemaphorePermit, Semaphore, oneshot};

use super::{
    ModelBudgetManager, ProviderBudgetManager, ResponseBudgetLeases, UnifiedBudgetLimits,
    UnifiedBudgetReservation,
};
use crate::core::budget::{BudgetReservationError, BudgetStatus};

/// Bounds queued work, active reservations, and their terminal cleanup together.
/// A reservation can own at most two drop-cancel tasks (provider and model).
const MAX_ASYNC_BUDGET_RESERVATIONS: usize = 1024;

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
        dispatch(permit, move || manager.reset_provider_budget(&provider))
            .await
            .map_err(|_| BudgetReservationError::BackendUnavailable)
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
        dispatch(permit, move || manager.reset_model_budget(&model))
            .await
            .map_err(|_| BudgetReservationError::BackendUnavailable)
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
        rx.await
            .map_err(|_| BudgetReservationError::BackendUnavailable)?
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
        dispatch(permit, move || {
            limits.settle_response_leases(&provider, &model, &leases, cost)
        })
        .await
        .map_err(|_| BudgetReservationError::BackendUnavailable)?
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
        dispatch(permit, move || self.settle(actual_amount))
            .await
            .map_err(|_| BudgetReservationError::BackendUnavailable)?
    }

    /// Cancel without blocking the async executor, even if this future is dropped.
    pub async fn cancel_async(mut self) -> Result<(), BudgetReservationError> {
        if !self.has_distributed_lease() {
            self.cancel();
            return Ok(());
        }
        let permit = self.async_permit()?;
        dispatch(permit, move || self.cancel())
            .await
            .map_err(|_| BudgetReservationError::BackendUnavailable)
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
