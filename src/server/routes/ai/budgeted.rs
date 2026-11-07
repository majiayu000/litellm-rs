use std::future::Future;
use std::sync::Arc;

use uuid::Uuid;

use crate::config::models::gateway::GatewayPricingConfig;
use crate::core::budget::{
    BudgetManager, BudgetReservation, UnifiedBudgetLimits, UnifiedBudgetReservation,
};
use crate::core::keys::KeyManager;
use crate::core::models::openai::Usage;
use crate::core::pricing_service::PricingService;
use crate::core::providers::{Provider, ProviderError};
use crate::core::router::UnifiedRouter;
use crate::core::types::model::ProviderCapability;
use crate::utils::error::gateway_error::GatewayError;

use super::execution;
pub(super) use super::execution::StreamingDeploymentLease;
use super::spend;

#[cfg(test)]
#[path = "budgeted_async_tests.rs"]
mod async_tests;

#[derive(Clone, Copy)]
pub(super) enum SettlementMode {
    Metered,
    AvailabilityOnly,
    KeyReservationThenPostSuccessRecord,
}

pub(super) type ApiKeyBudgetPolicy = SettlementMode;

impl SettlementMode {
    #[allow(non_upper_case_globals)]
    pub(super) const FromProviderReservation: Self = Self::Metered;
    #[allow(non_upper_case_globals)]
    pub(super) const RequirePricedReservation: Self = Self::KeyReservationThenPostSuccessRecord;
}

#[derive(Clone)]
pub(crate) struct BudgetedExecutor {
    budget_limits: Arc<UnifiedBudgetLimits>,
    budget_manager: Arc<BudgetManager>,
    pricing: Arc<PricingService>,
    key_manager: KeyManager,
}

impl BudgetedExecutor {
    pub(crate) fn new(
        budget_limits: Arc<UnifiedBudgetLimits>,
        budget_manager: Arc<BudgetManager>,
        pricing: Arc<PricingService>,
        key_manager: KeyManager,
    ) -> Self {
        Self {
            budget_limits,
            budget_manager,
            pricing,
            key_manager,
        }
    }

    pub(super) fn for_selected(
        &self,
        provider: impl Into<String>,
        model: impl Into<String>,
    ) -> BudgetedCall {
        BudgetedCall::new(self.budget_limits.clone(), provider, model)
            .with_budget_manager(self.budget_manager.clone())
    }

    pub(super) fn for_selected_with_api_key_budget(
        &self,
        provider: impl Into<String>,
        model: impl Into<String>,
        api_key_budget_id: Option<Uuid>,
        mode: SettlementMode,
    ) -> BudgetedCall {
        self.for_selected(provider, model).with_api_key_budget(
            self.budget_manager.clone(),
            api_key_budget_id,
            mode,
        )
    }

    pub(super) fn pricing(&self) -> Arc<PricingService> {
        self.pricing.clone()
    }

    pub(super) fn budget_limits(&self) -> Arc<UnifiedBudgetLimits> {
        self.budget_limits.clone()
    }

    pub(super) fn key_manager(&self) -> KeyManager {
        self.key_manager.clone()
    }
}

pub(super) async fn run_unary<T, F, Fut>(
    router: &UnifiedRouter,
    requested_model: &str,
    capability: ProviderCapability,
    estimated_tokens: u64,
    operation: F,
) -> Result<T, GatewayError>
where
    F: Fn(Provider, String, String) -> Fut + Clone,
    Fut: Future<Output = Result<(T, u64), ProviderError>>,
{
    execution::execute_with_selected_deployment(
        router,
        requested_model,
        capability,
        estimated_tokens,
        operation,
    )
    .await
}

pub(super) async fn run_stream<T, F, Fut>(
    router: Arc<UnifiedRouter>,
    requested_model: &str,
    capability: ProviderCapability,
    estimated_tokens: u64,
    operation: F,
) -> Result<(T, StreamingDeploymentLease), GatewayError>
where
    F: Fn(Provider, String, String) -> Fut + Clone,
    Fut: Future<Output = Result<T, ProviderError>>,
{
    execution::execute_stream_with_selected_deployment(
        router,
        requested_model,
        capability,
        estimated_tokens,
        operation,
    )
    .await
}

pub(super) struct BudgetedCall {
    budget_limits: Arc<UnifiedBudgetLimits>,
    budget_manager: Option<Arc<BudgetManager>>,
    api_key_budget_id: Option<Uuid>,
    api_key_estimated_cost: Option<f64>,
    provider: String,
    model: String,
    settlement_mode: SettlementMode,
}

#[derive(Clone)]
pub(super) struct BudgetContext {
    budget_limits: Arc<UnifiedBudgetLimits>,
    provider: String,
    model: String,
}

impl BudgetContext {
    pub(super) fn budget_limits(&self) -> &UnifiedBudgetLimits {
        self.budget_limits.as_ref()
    }

    pub(super) fn provider(&self) -> &str {
        &self.provider
    }

    pub(super) fn model(&self) -> &str {
        &self.model
    }

    pub(super) async fn reserve_spend(
        &self,
        amount: f64,
    ) -> Result<UnifiedBudgetReservation, ProviderError> {
        self.budget_limits
            .reserve_spend_async(&self.provider, &self.model, amount)
            .await
            .map_err(|error| {
                spend::reservation_error_to_provider_error(error, &self.provider, &self.model)
            })
    }
}

impl BudgetedCall {
    pub(super) fn new(
        budget_limits: Arc<UnifiedBudgetLimits>,
        provider: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            budget_limits,
            budget_manager: None,
            api_key_budget_id: None,
            api_key_estimated_cost: None,
            provider: provider.into(),
            model: model.into(),
            settlement_mode: SettlementMode::AvailabilityOnly,
        }
    }

    pub(super) fn with_settlement_mode(mut self, mode: SettlementMode) -> Self {
        self.settlement_mode = mode;
        self
    }

    pub(super) fn with_budget_manager(mut self, budget_manager: Arc<BudgetManager>) -> Self {
        self.budget_manager = Some(budget_manager);
        self
    }

    pub(super) fn with_api_key_budget(
        mut self,
        budget_manager: Arc<BudgetManager>,
        api_key_budget_id: Option<Uuid>,
        mode: SettlementMode,
    ) -> Self {
        self.budget_manager = Some(budget_manager);
        self.api_key_budget_id = api_key_budget_id;
        self.settlement_mode = mode;
        self
    }

    pub(super) fn with_precomputed_api_key_budget_cost(
        mut self,
        estimated_cost: Option<f64>,
    ) -> Self {
        self.api_key_estimated_cost = estimated_cost;
        self
    }

    pub(super) fn ensure_available(self) -> Result<(), ProviderError> {
        if !matches!(self.settlement_mode, SettlementMode::AvailabilityOnly) {
            return Err(ProviderError::invalid_request(
                "budget",
                "availability check requires AvailabilityOnly settlement mode",
            ));
        }
        let context = self.context();
        spend::ensure_budget_available(context.budget_limits(), context.provider(), context.model())
    }

    pub(super) async fn reserve_call<T, Reserve, Call, CallFuture>(
        self,
        reserve: Reserve,
        call: Call,
    ) -> Result<(T, BudgetReservations), ProviderError>
    where
        Reserve:
            AsyncFnOnce(&BudgetContext) -> Result<Option<UnifiedBudgetReservation>, ProviderError>,
        Call: FnOnce() -> CallFuture,
        CallFuture: Future<Output = Result<T, ProviderError>>,
    {
        let mut reservations = self.reserve(reserve).await?;
        match call().await {
            Ok(value) => {
                execution::completion::provider_succeeded();
                Ok((value, reservations))
            }
            Err(error) => {
                reservations.on_error(&error).await;
                Err(error)
            }
        }
    }

    pub(super) async fn reserve_for_call<Reserve>(
        self,
        reserve: Reserve,
    ) -> Result<(BudgetReservations, BudgetContext), ProviderError>
    where
        Reserve:
            AsyncFnOnce(&BudgetContext) -> Result<Option<UnifiedBudgetReservation>, ProviderError>,
    {
        let context = self.context();
        let reservations = self.reserve(reserve).await?;
        Ok((reservations, context))
    }

    pub(super) async fn reserve_call_settle<T, Reserve, Call, CallFuture, Settle, SettleFuture>(
        self,
        reserve: Reserve,
        call: Call,
        settle: Settle,
    ) -> Result<(T, u64), ProviderError>
    where
        Reserve:
            AsyncFnOnce(&BudgetContext) -> Result<Option<UnifiedBudgetReservation>, ProviderError>,
        Call: FnOnce() -> CallFuture,
        CallFuture: Future<Output = Result<T, ProviderError>>,
        Settle: FnOnce(T, BudgetReservations, BudgetContext) -> SettleFuture,
        SettleFuture: Future<Output = (T, u64)>,
    {
        let context = self.context();
        let (value, reservations) = self.reserve_call(reserve, call).await?;
        Ok(settle(value, reservations, context).await)
    }

    async fn reserve<Reserve>(&self, reserve: Reserve) -> Result<BudgetReservations, ProviderError>
    where
        Reserve:
            AsyncFnOnce(&BudgetContext) -> Result<Option<UnifiedBudgetReservation>, ProviderError>,
    {
        let context = self.context();
        spend::ensure_budget_available(
            context.budget_limits(),
            context.provider(),
            context.model(),
        )?;
        let budget = reserve(&context).await?;
        let key = match self.settlement_mode {
            SettlementMode::AvailabilityOnly => None,
            SettlementMode::Metered => {
                let budget_manager = self.budget_manager.as_ref().ok_or_else(|| {
                    ProviderError::invalid_request(
                        "budget",
                        "API key budget manager is required for budget reservation",
                    )
                })?;
                spend::reserve_api_key_budget_for_reservation(
                    budget_manager.as_ref(),
                    self.api_key_budget_id,
                    budget.as_ref(),
                )
                .await?
            }
            SettlementMode::KeyReservationThenPostSuccessRecord => {
                let budget_manager = self.budget_manager.as_ref().ok_or_else(|| {
                    ProviderError::invalid_request(
                        "budget",
                        "API key budget manager is required for budget reservation",
                    )
                })?;
                let estimated_cost = self.api_key_estimated_cost.or_else(|| {
                    budget
                        .as_ref()
                        .map(UnifiedBudgetReservation::reserved_amount)
                });
                spend::reserve_api_key_budget(
                    budget_manager.as_ref(),
                    self.api_key_budget_id,
                    estimated_cost,
                )
                .await?
            }
        };

        Ok(BudgetReservations { budget, key })
    }

    fn context(&self) -> BudgetContext {
        BudgetContext {
            budget_limits: self.budget_limits.clone(),
            provider: self.provider.clone(),
            model: self.model.clone(),
        }
    }
}

pub(super) struct BudgetReservations {
    budget: Option<UnifiedBudgetReservation>,
    key: Option<BudgetReservation>,
}

impl BudgetReservations {
    pub(super) fn from_parts(
        budget: Option<UnifiedBudgetReservation>,
        key: Option<BudgetReservation>,
    ) -> Self {
        Self { budget, key }
    }

    pub(super) fn into_parts(
        self,
    ) -> (Option<UnifiedBudgetReservation>, Option<BudgetReservation>) {
        (self.budget, self.key)
    }

    pub(super) async fn on_error(&mut self, error: &ProviderError) {
        // Transport, parse and 5xx errors cannot prove that upstream did no work.
        let rejected = matches!(
            error,
            ProviderError::Authentication { .. }
                | ProviderError::RateLimit { .. }
                | ProviderError::QuotaExceeded { .. }
                | ProviderError::ModelNotFound { .. }
                | ProviderError::InvalidRequest { .. }
                | ProviderError::NotSupported { .. }
                | ProviderError::ContextLengthExceeded { .. }
                | ProviderError::TokenLimitExceeded { .. }
        ) || matches!(error, ProviderError::ApiError { status, .. } if (400..500).contains(status) && *status != 408);
        if !rejected
            && self
                .key
                .as_ref()
                .is_some_and(BudgetReservation::is_persistent)
        {
            drop(self.key.take());
            crate::core::request_ledger::update_billing(None, |billing| {
                billing.key_settlement = Some("settlement_unconfirmed".into());
                billing.unknown_reason = Some("upstream_error_usage_unknown".into());
            });
        }
        self.cancel().await;
    }

    pub(super) async fn cancel(&mut self) {
        if let Some(reservation) = self.budget.take()
            && let Err(error) = reservation.cancel_async().await
        {
            tracing::error!(?error, "provider/model budget cancellation failed");
        }
        if let Some(reservation) = self.key.take() {
            let tracked = reservation.is_tracked();
            let released = reservation.cancel_async().await.is_ok();
            if tracked && released {
                crate::core::request_ledger::update_billing(None, |billing| {
                    billing.key_settlement = Some("released".into());
                    billing.key_charge_amount = Some(0.0);
                    billing.settlement_updated_at = Some(chrono::Utc::now());
                });
            }
        }
    }
}

pub(super) struct SettledStream {
    pub(super) pricing_service: Arc<PricingService>,
    pub(super) pricing_config: GatewayPricingConfig,
    pub(super) budget_limits: Arc<UnifiedBudgetLimits>,
    pub(super) key_manager: KeyManager,
    pub(super) api_key_id: Option<Uuid>,
    pub(super) provider: String,
    pub(super) model: String,
    pub(super) request_pricing: spend::RequestPricing,
    pub(super) budget_reservation: Option<UnifiedBudgetReservation>,
    pub(super) key_budget_reservation: Option<BudgetReservation>,
    pub(super) ledger_facts: Option<crate::core::request_ledger::SharedRequestLedgerFacts>,
}

impl SettledStream {
    pub(super) async fn record_completion(
        mut self,
        usage: Option<&Usage>,
        saw_upstream_output: bool,
    ) {
        let facts = self.ledger_facts.clone();
        let fut = spend::record_finished_stream_spend_with_reservation_with_policy(
            self.pricing_service.as_ref(),
            &self.pricing_config,
            spend::StreamSpendSettlement {
                budget_limits: self.budget_limits.as_ref(),
                key_manager: &self.key_manager,
                api_key_id: self.api_key_id,
                provider: &self.provider,
                model: &self.model,
                request_pricing: self.request_pricing.clone(),
                usage,
                saw_upstream_output,
                budget_reservation: self.budget_reservation.take(),
                key_budget_reservation: self.key_budget_reservation.take(),
            },
        );
        match facts {
            Some(facts) => crate::core::request_ledger::scope_facts(facts, fut).await,
            None => fut.await,
        }
    }

    pub(super) async fn record_disconnect(&mut self, usage: Option<&Usage>) {
        let facts = self.ledger_facts.clone();
        let fut = spend::record_stream_disconnect_spend_with_reservation_with_policy(
            self.pricing_service.as_ref(),
            &self.pricing_config,
            spend::usage_spend_settlement_with_request_pricing(
                (
                    self.budget_limits.as_ref(),
                    &self.key_manager,
                    self.api_key_id,
                ),
                (&self.provider, &self.model, usage),
                self.request_pricing.clone(),
                self.budget_reservation.take(),
                self.key_budget_reservation.take(),
            ),
        );
        match facts {
            Some(facts) => crate::core::request_ledger::scope_facts(facts, fut).await,
            None => fut.await,
        }
    }
}

#[cfg(test)]
#[path = "budgeted_tests.rs"]
mod tests;
