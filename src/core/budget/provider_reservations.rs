use super::config::BudgetPersistenceEvent;
use super::distributed::{BudgetLeaseScope, budget_period_epoch};
use super::tracker::BudgetReservationError;
use super::types::BudgetStatus;
use super::{
    BudgetAmount, add_budget_spend, budget_can_spend, release_budget_spend, settle_budget_spend,
};
use super::{ModelBudgetManager, ProviderBudgetManager, UnifiedBudgetLimits};
use std::sync::atomic::Ordering;

impl ProviderBudgetManager {
    pub fn reserve_provider_spend(
        &self,
        provider: &str,
        max_amount: f64,
    ) -> Result<ProviderBudgetReservation, BudgetReservationError> {
        let reserved = BudgetAmount::from_f64(max_amount)?;
        if !self.is_enabled() {
            return Ok(ProviderBudgetReservation::untracked(
                self.clone(),
                provider.to_string(),
                reserved,
            ));
        }
        let Some(mut budget) = self.budgets.get_mut(provider) else {
            return Ok(ProviderBudgetReservation::untracked(
                self.clone(),
                provider.to_string(),
                reserved,
            ));
        };
        if !budget.enabled {
            return Ok(ProviderBudgetReservation::untracked(
                self.clone(),
                provider.to_string(),
                reserved,
            ));
        }
        if self.backend.is_distributed() {
            let max = BudgetAmount::from_f64(budget.max_budget)?;
            let period_epoch = budget_period_epoch(budget.reset_period, chrono::Utc::now());
            let outstanding = self
                .reserved_spend
                .get(provider)
                .map(|value| *value)
                .unwrap_or_else(BudgetAmount::zero);
            let seed_committed = BudgetAmount::from_f64(budget.current_spend)
                .unwrap_or_else(|_| BudgetAmount::zero())
                .saturating_sub(outstanding);
            let reservation_reset_at = budget.last_reset_at;
            drop(budget);
            let lease = self.backend.reserve(
                BudgetLeaseScope::Provider,
                provider,
                reserved,
                max,
                seed_committed,
                period_epoch,
            )?;
            self.sync_lease_snapshot(provider, lease.snapshot);
            return Ok(ProviderBudgetReservation::tracked(
                self.clone(),
                provider.to_string(),
                reserved,
                reservation_reset_at,
            )
            .with_lease(lease.lease_id, lease.period_epoch));
        }
        if !budget_can_spend(budget.current_spend, budget.max_budget, true, max_amount)? {
            return Err(BudgetReservationError::ProviderBudgetExceeded);
        }
        let reservation_reset_at = budget.last_reset_at;
        let current_spend = add_budget_spend(budget.current_spend, max_amount)?;
        self.add_provider_outstanding_reservation(provider, reserved)?;
        budget.current_spend = current_spend.as_f64();
        budget.updated_at = chrono::Utc::now();
        drop(budget);
        Ok(ProviderBudgetReservation::tracked(
            self.clone(),
            provider.to_string(),
            reserved,
            reservation_reset_at,
        ))
    }

    pub(crate) fn release_provider_reservation(
        &self,
        provider: &str,
        reserved: BudgetAmount,
        reservation_reset_at: Option<chrono::DateTime<chrono::Utc>>,
    ) {
        let snapshot = if let Some(mut budget) = self.budgets.get_mut(provider) {
            let same_reset_epoch = budget.last_reset_at == reservation_reset_at;
            if let Ok(current_spend) =
                release_budget_spend(budget.current_spend, reserved, same_reset_epoch)
            {
                if same_reset_epoch {
                    self.release_provider_outstanding_reservation(provider, reserved);
                }
                budget.current_spend = current_spend.as_f64();
                budget.updated_at = chrono::Utc::now();
                Some(self.snapshot_for(&budget))
            } else {
                None
            }
        } else {
            None
        };
        if let Some(snapshot) = snapshot {
            self.send_persistence_event(BudgetPersistenceEvent::Upsert(snapshot));
        }
    }

    pub(crate) fn settle_provider_reservation(
        &self,
        provider: &str,
        reserved: BudgetAmount,
        actual: BudgetAmount,
        reservation_reset_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Option<BudgetStatus>, BudgetReservationError> {
        let status_snapshot = if let Some(mut budget) = self.budgets.get_mut(provider) {
            let same_reset_epoch = budget.last_reset_at == reservation_reset_at;
            let current_spend =
                settle_budget_spend(budget.current_spend, reserved, actual, same_reset_epoch)?;
            if same_reset_epoch {
                self.release_provider_outstanding_reservation(provider, reserved);
            }
            budget.current_spend = current_spend.as_f64();
            budget.updated_at = chrono::Utc::now();
            if let Some(counter) = self.request_counts.get(provider) {
                counter.count.fetch_add(1, Ordering::Relaxed);
            }
            let status = budget.status();
            Some((status, self.snapshot_for(&budget)))
        } else {
            None
        };
        if let Some((status, snapshot)) = status_snapshot {
            self.send_persistence_event(BudgetPersistenceEvent::Upsert(snapshot));
            Ok(Some(status))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn can_settle_provider_reservation(
        &self,
        provider: &str,
        reserved: BudgetAmount,
        actual: BudgetAmount,
        reservation_reset_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<bool, BudgetReservationError> {
        if let Some(budget) = self.budgets.get(provider) {
            settle_budget_spend(
                budget.current_spend,
                reserved,
                actual,
                budget.last_reset_at == reservation_reset_at,
            )?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn finish_distributed_settle(&self, provider: &str) -> Option<BudgetStatus> {
        if let Some(counter) = self.request_counts.get(provider) {
            counter.count.fetch_add(1, Ordering::Relaxed);
        }
        let budget = self.budgets.get(provider)?;
        let status = budget.status();
        let snapshot = self.snapshot_for(&budget);
        drop(budget);
        self.send_persistence_event(BudgetPersistenceEvent::Upsert(snapshot));
        Some(status)
    }

    fn add_provider_outstanding_reservation(
        &self,
        provider: &str,
        reserved: BudgetAmount,
    ) -> Result<(), BudgetReservationError> {
        match self.reserved_spend.entry(provider.to_string()) {
            dashmap::mapref::entry::Entry::Occupied(mut entry) => {
                let next = entry
                    .get()
                    .checked_add(reserved)
                    .map_err(BudgetReservationError::InvalidAmount)?;
                *entry.get_mut() = next;
            }
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                entry.insert(reserved);
            }
        }
        Ok(())
    }

    fn release_provider_outstanding_reservation(&self, provider: &str, reserved: BudgetAmount) {
        if let dashmap::mapref::entry::Entry::Occupied(mut entry) =
            self.reserved_spend.entry(provider.to_string())
        {
            let next = entry.get().saturating_sub(reserved);
            if next == BudgetAmount::zero() {
                entry.remove();
            } else {
                *entry.get_mut() = next;
            }
        }
    }
}

impl ModelBudgetManager {
    pub fn reserve_model_spend(
        &self,
        model: &str,
        max_amount: f64,
    ) -> Result<ModelBudgetReservation, BudgetReservationError> {
        let reserved = BudgetAmount::from_f64(max_amount)?;
        if !self.is_enabled() {
            return Ok(ModelBudgetReservation::untracked(
                self.clone(),
                model.to_string(),
                reserved,
            ));
        }
        let Some(mut budget) = self.budgets.get_mut(model) else {
            return Ok(ModelBudgetReservation::untracked(
                self.clone(),
                model.to_string(),
                reserved,
            ));
        };
        if !budget.enabled {
            return Ok(ModelBudgetReservation::untracked(
                self.clone(),
                model.to_string(),
                reserved,
            ));
        }
        if self.backend.is_distributed() {
            let max = BudgetAmount::from_f64(budget.max_budget)?;
            let period_epoch = budget_period_epoch(budget.reset_period, chrono::Utc::now());
            let outstanding = self
                .reserved_spend
                .get(model)
                .map(|value| *value)
                .unwrap_or_else(BudgetAmount::zero);
            let seed_committed = BudgetAmount::from_f64(budget.current_spend)
                .unwrap_or_else(|_| BudgetAmount::zero())
                .saturating_sub(outstanding);
            let reservation_reset_at = budget.last_reset_at;
            drop(budget);
            let lease = self.backend.reserve(
                BudgetLeaseScope::Model,
                model,
                reserved,
                max,
                seed_committed,
                period_epoch,
            )?;
            self.sync_lease_snapshot(model, lease.snapshot);
            return Ok(ModelBudgetReservation::tracked(
                self.clone(),
                model.to_string(),
                reserved,
                reservation_reset_at,
            )
            .with_lease(lease.lease_id, lease.period_epoch));
        }
        if !budget_can_spend(budget.current_spend, budget.max_budget, true, max_amount)? {
            return Err(BudgetReservationError::ModelBudgetExceeded);
        }
        let reservation_reset_at = budget.last_reset_at;
        let current_spend = add_budget_spend(budget.current_spend, max_amount)?;
        self.add_model_outstanding_reservation(model, reserved)?;
        budget.current_spend = current_spend.as_f64();
        budget.updated_at = chrono::Utc::now();
        drop(budget);
        Ok(ModelBudgetReservation::tracked(
            self.clone(),
            model.to_string(),
            reserved,
            reservation_reset_at,
        ))
    }

    pub(crate) fn release_model_reservation(
        &self,
        model: &str,
        reserved: BudgetAmount,
        reservation_reset_at: Option<chrono::DateTime<chrono::Utc>>,
    ) {
        let snapshot = if let Some(mut budget) = self.budgets.get_mut(model) {
            let same_reset_epoch = budget.last_reset_at == reservation_reset_at;
            if let Ok(current_spend) =
                release_budget_spend(budget.current_spend, reserved, same_reset_epoch)
            {
                if same_reset_epoch {
                    self.release_model_outstanding_reservation(model, reserved);
                }
                budget.current_spend = current_spend.as_f64();
                budget.updated_at = chrono::Utc::now();
                Some(self.snapshot_for(&budget))
            } else {
                None
            }
        } else {
            None
        };
        if let Some(snapshot) = snapshot {
            self.send_persistence_event(BudgetPersistenceEvent::Upsert(snapshot));
        }
    }

    pub(crate) fn settle_model_reservation(
        &self,
        model: &str,
        reserved: BudgetAmount,
        actual: BudgetAmount,
        reservation_reset_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Option<BudgetStatus>, BudgetReservationError> {
        let status_snapshot = if let Some(mut budget) = self.budgets.get_mut(model) {
            let same_reset_epoch = budget.last_reset_at == reservation_reset_at;
            let current_spend =
                settle_budget_spend(budget.current_spend, reserved, actual, same_reset_epoch)?;
            if same_reset_epoch {
                self.release_model_outstanding_reservation(model, reserved);
            }
            budget.current_spend = current_spend.as_f64();
            budget.updated_at = chrono::Utc::now();
            if let Some(counter) = self.request_counts.get(model) {
                counter.count.fetch_add(1, Ordering::Relaxed);
            }
            let status = budget.status();
            Some((status, self.snapshot_for(&budget)))
        } else {
            None
        };
        if let Some((status, snapshot)) = status_snapshot {
            self.send_persistence_event(BudgetPersistenceEvent::Upsert(snapshot));
            Ok(Some(status))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn can_settle_model_reservation(
        &self,
        model: &str,
        reserved: BudgetAmount,
        actual: BudgetAmount,
        reservation_reset_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<bool, BudgetReservationError> {
        if let Some(budget) = self.budgets.get(model) {
            settle_budget_spend(
                budget.current_spend,
                reserved,
                actual,
                budget.last_reset_at == reservation_reset_at,
            )?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn finish_distributed_settle(&self, model: &str) -> Option<BudgetStatus> {
        if let Some(counter) = self.request_counts.get(model) {
            counter.count.fetch_add(1, Ordering::Relaxed);
        }
        let budget = self.budgets.get(model)?;
        let status = budget.status();
        let snapshot = self.snapshot_for(&budget);
        drop(budget);
        self.send_persistence_event(BudgetPersistenceEvent::Upsert(snapshot));
        Some(status)
    }

    fn add_model_outstanding_reservation(
        &self,
        model: &str,
        reserved: BudgetAmount,
    ) -> Result<(), BudgetReservationError> {
        match self.reserved_spend.entry(model.to_string()) {
            dashmap::mapref::entry::Entry::Occupied(mut entry) => {
                let next = entry
                    .get()
                    .checked_add(reserved)
                    .map_err(BudgetReservationError::InvalidAmount)?;
                *entry.get_mut() = next;
            }
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                entry.insert(reserved);
            }
        }
        Ok(())
    }

    fn release_model_outstanding_reservation(&self, model: &str, reserved: BudgetAmount) {
        if let dashmap::mapref::entry::Entry::Occupied(mut entry) =
            self.reserved_spend.entry(model.to_string())
        {
            let next = entry.get().saturating_sub(reserved);
            if next == BudgetAmount::zero() {
                entry.remove();
            } else {
                *entry.get_mut() = next;
            }
        }
    }
}

impl UnifiedBudgetLimits {
    pub fn reserve_spend(
        &self,
        provider: &str,
        model: &str,
        max_amount: f64,
    ) -> Result<UnifiedBudgetReservation, BudgetReservationError> {
        let provider_reservation = self
            .providers
            .reserve_provider_spend(provider, max_amount)?;
        match self.models.reserve_model_spend(model, max_amount) {
            Ok(model_reservation) => Ok(UnifiedBudgetReservation::new(
                provider_reservation,
                model_reservation,
            )),
            Err(error) => {
                provider_reservation.cancel();
                Err(error)
            }
        }
    }
}

pub struct ProviderBudgetReservation {
    manager: ProviderBudgetManager,
    provider: String,
    reserved: BudgetAmount,
    reservation_reset_at: Option<chrono::DateTime<chrono::Utc>>,
    tracked: bool,
    settled: bool,
    lease_id: Option<String>,
    period_epoch: i64,
}

impl ProviderBudgetReservation {
    pub(crate) fn tracked(
        manager: ProviderBudgetManager,
        provider: String,
        reserved: BudgetAmount,
        reservation_reset_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Self {
        Self {
            manager,
            provider,
            reserved,
            reservation_reset_at,
            tracked: true,
            settled: false,
            lease_id: None,
            period_epoch: 0,
        }
    }

    pub(crate) fn untracked(
        manager: ProviderBudgetManager,
        provider: String,
        reserved: BudgetAmount,
    ) -> Self {
        Self {
            manager,
            provider,
            reserved,
            reservation_reset_at: None,
            tracked: false,
            settled: false,
            lease_id: None,
            period_epoch: 0,
        }
    }

    fn with_lease(mut self, lease_id: String, period_epoch: i64) -> Self {
        self.lease_id = Some(lease_id);
        self.period_epoch = period_epoch;
        self
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }

    pub fn reserved_amount(&self) -> f64 {
        self.reserved.as_f64()
    }

    pub(crate) fn reserved(&self) -> BudgetAmount {
        self.reserved
    }

    pub fn settle(
        mut self,
        actual_amount: f64,
    ) -> Result<Option<BudgetStatus>, BudgetReservationError> {
        let actual = BudgetAmount::from_f64(actual_amount)?;
        if let Some(lease_id) = self.lease_id.clone() {
            let snapshot = self.manager.backend.settle(
                BudgetLeaseScope::Provider,
                &self.provider,
                &lease_id,
                self.reserved,
                actual,
                self.period_epoch,
            )?;
            self.manager.sync_lease_snapshot(&self.provider, snapshot);
            let status = self.manager.finish_distributed_settle(&self.provider);
            self.settled = true;
            return Ok(status);
        }
        let status = if self.tracked {
            self.manager.settle_provider_reservation(
                &self.provider,
                self.reserved,
                actual,
                self.reservation_reset_at,
            )?
        } else {
            self.manager
                .record_provider_spend(&self.provider, actual.as_f64())
        };
        self.settled = true;
        Ok(status)
    }

    pub fn cancel(mut self) {
        if let Some(lease_id) = self.lease_id.clone() {
            match self.manager.backend.cancel(
                BudgetLeaseScope::Provider,
                &self.provider,
                &lease_id,
                self.reserved,
                self.period_epoch,
            ) {
                Ok(snapshot) => self.manager.sync_lease_snapshot(&self.provider, snapshot),
                Err(_) => self.manager.backend.spawn_cancel(
                    BudgetLeaseScope::Provider,
                    &self.provider,
                    lease_id,
                    self.reserved,
                    self.period_epoch,
                ),
            }
            self.settled = true;
            return;
        }
        if self.tracked {
            self.manager.release_provider_reservation(
                &self.provider,
                self.reserved,
                self.reservation_reset_at,
            );
        }
        self.settled = true;
    }
}

impl Drop for ProviderBudgetReservation {
    fn drop(&mut self) {
        if self.tracked && !self.settled {
            if let Some(lease_id) = self.lease_id.clone() {
                self.manager.backend.spawn_cancel(
                    BudgetLeaseScope::Provider,
                    &self.provider,
                    lease_id,
                    self.reserved,
                    self.period_epoch,
                );
            } else {
                self.manager.release_provider_reservation(
                    &self.provider,
                    self.reserved,
                    self.reservation_reset_at,
                );
            }
        }
    }
}

pub struct ModelBudgetReservation {
    manager: ModelBudgetManager,
    model: String,
    reserved: BudgetAmount,
    reservation_reset_at: Option<chrono::DateTime<chrono::Utc>>,
    tracked: bool,
    settled: bool,
    lease_id: Option<String>,
    period_epoch: i64,
}

impl ModelBudgetReservation {
    pub(crate) fn tracked(
        manager: ModelBudgetManager,
        model: String,
        reserved: BudgetAmount,
        reservation_reset_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Self {
        Self {
            manager,
            model,
            reserved,
            reservation_reset_at,
            tracked: true,
            settled: false,
            lease_id: None,
            period_epoch: 0,
        }
    }

    pub(crate) fn untracked(
        manager: ModelBudgetManager,
        model: String,
        reserved: BudgetAmount,
    ) -> Self {
        Self {
            manager,
            model,
            reserved,
            reservation_reset_at: None,
            tracked: false,
            settled: false,
            lease_id: None,
            period_epoch: 0,
        }
    }

    fn with_lease(mut self, lease_id: String, period_epoch: i64) -> Self {
        self.lease_id = Some(lease_id);
        self.period_epoch = period_epoch;
        self
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn reserved_amount(&self) -> f64 {
        self.reserved.as_f64()
    }

    pub(crate) fn reserved(&self) -> BudgetAmount {
        self.reserved
    }

    pub fn settle(
        mut self,
        actual_amount: f64,
    ) -> Result<Option<BudgetStatus>, BudgetReservationError> {
        let actual = BudgetAmount::from_f64(actual_amount)?;
        if let Some(lease_id) = self.lease_id.clone() {
            let snapshot = self.manager.backend.settle(
                BudgetLeaseScope::Model,
                &self.model,
                &lease_id,
                self.reserved,
                actual,
                self.period_epoch,
            )?;
            self.manager.sync_lease_snapshot(&self.model, snapshot);
            let status = self.manager.finish_distributed_settle(&self.model);
            self.settled = true;
            return Ok(status);
        }
        let status = if self.tracked {
            self.manager.settle_model_reservation(
                &self.model,
                self.reserved,
                actual,
                self.reservation_reset_at,
            )?
        } else {
            self.manager
                .record_model_spend(&self.model, actual.as_f64())
        };
        self.settled = true;
        Ok(status)
    }

    pub fn cancel(mut self) {
        if let Some(lease_id) = self.lease_id.clone() {
            match self.manager.backend.cancel(
                BudgetLeaseScope::Model,
                &self.model,
                &lease_id,
                self.reserved,
                self.period_epoch,
            ) {
                Ok(snapshot) => self.manager.sync_lease_snapshot(&self.model, snapshot),
                Err(_) => self.manager.backend.spawn_cancel(
                    BudgetLeaseScope::Model,
                    &self.model,
                    lease_id,
                    self.reserved,
                    self.period_epoch,
                ),
            }
            self.settled = true;
            return;
        }
        if self.tracked {
            self.manager.release_model_reservation(
                &self.model,
                self.reserved,
                self.reservation_reset_at,
            );
        }
        self.settled = true;
    }
}

impl Drop for ModelBudgetReservation {
    fn drop(&mut self) {
        if self.tracked && !self.settled {
            if let Some(lease_id) = self.lease_id.clone() {
                self.manager.backend.spawn_cancel(
                    BudgetLeaseScope::Model,
                    &self.model,
                    lease_id,
                    self.reserved,
                    self.period_epoch,
                );
            } else {
                self.manager.release_model_reservation(
                    &self.model,
                    self.reserved,
                    self.reservation_reset_at,
                );
            }
        }
    }
}

pub struct UnifiedBudgetReservation {
    provider: ProviderBudgetReservation,
    model: ModelBudgetReservation,
}

impl UnifiedBudgetReservation {
    pub(crate) fn new(provider: ProviderBudgetReservation, model: ModelBudgetReservation) -> Self {
        Self { provider, model }
    }

    pub fn provider(&self) -> &str {
        self.provider.provider()
    }

    pub fn model(&self) -> &str {
        self.model.model()
    }

    pub fn reserved_amount(&self) -> f64 {
        self.provider
            .reserved_amount()
            .min(self.model.reserved_amount())
    }

    pub fn settle(
        self,
        actual_amount: f64,
    ) -> Result<(Option<BudgetStatus>, Option<BudgetStatus>), BudgetReservationError> {
        let actual = BudgetAmount::from_f64(actual_amount)?;
        let Self { provider, model } = self;
        if provider.tracked {
            provider.manager.can_settle_provider_reservation(
                provider.provider(),
                provider.reserved(),
                actual,
                provider.reservation_reset_at,
            )?;
        }
        if model.tracked {
            model.manager.can_settle_model_reservation(
                model.model(),
                model.reserved(),
                actual,
                model.reservation_reset_at,
            )?;
        }
        let provider_status = provider.settle(actual_amount)?;
        let model_status = model.settle(actual_amount)?;
        Ok((provider_status, model_status))
    }

    pub fn cancel(self) {
        let Self { provider, model } = self;
        provider.cancel();
        model.cancel();
    }
}
