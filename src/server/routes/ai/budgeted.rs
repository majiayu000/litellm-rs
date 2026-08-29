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
    operation: F,
) -> Result<T, GatewayError>
where
    F: Fn(Provider, String, String) -> Fut + Clone,
    Fut: Future<Output = Result<(T, u64), ProviderError>>,
{
    execution::execute_with_selected_deployment(router, requested_model, capability, operation)
        .await
}

pub(super) async fn run_stream<T, F, Fut>(
    router: Arc<UnifiedRouter>,
    requested_model: &str,
    capability: ProviderCapability,
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

    pub(super) fn reserve_spend(
        &self,
        amount: f64,
    ) -> Result<UnifiedBudgetReservation, ProviderError> {
        self.budget_limits
            .reserve_spend(&self.provider, &self.model, amount)
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
        Reserve: FnOnce(&BudgetContext) -> Result<Option<UnifiedBudgetReservation>, ProviderError>,
        Call: FnOnce() -> CallFuture,
        CallFuture: Future<Output = Result<T, ProviderError>>,
    {
        let mut reservations = self.reserve(reserve)?;
        match call().await {
            Ok(value) => Ok((value, reservations)),
            Err(error) => {
                reservations.cancel();
                Err(error)
            }
        }
    }

    pub(super) async fn reserve_call_settle<T, Reserve, Call, CallFuture, Settle, SettleFuture>(
        self,
        reserve: Reserve,
        call: Call,
        settle: Settle,
    ) -> Result<(T, u64), ProviderError>
    where
        Reserve: FnOnce(&BudgetContext) -> Result<Option<UnifiedBudgetReservation>, ProviderError>,
        Call: FnOnce() -> CallFuture,
        CallFuture: Future<Output = Result<T, ProviderError>>,
        Settle: FnOnce(T, BudgetReservations, BudgetContext) -> SettleFuture,
        SettleFuture: Future<Output = (T, u64)>,
    {
        let context = self.context();
        let (value, reservations) = self.reserve_call(reserve, call).await?;
        Ok(settle(value, reservations, context).await)
    }

    fn reserve<Reserve>(&self, reserve: Reserve) -> Result<BudgetReservations, ProviderError>
    where
        Reserve: FnOnce(&BudgetContext) -> Result<Option<UnifiedBudgetReservation>, ProviderError>,
    {
        let context = self.context();
        spend::ensure_budget_available(
            context.budget_limits(),
            context.provider(),
            context.model(),
        )?;
        let budget = reserve(&context)?;
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
                )?
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
                )?
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
    pub(super) fn into_parts(
        self,
    ) -> (Option<UnifiedBudgetReservation>, Option<BudgetReservation>) {
        (self.budget, self.key)
    }

    fn cancel(&mut self) {
        if let Some(reservation) = self.budget.take() {
            reservation.cancel();
        }
        if let Some(reservation) = self.key.take() {
            reservation.cancel();
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
}

impl SettledStream {
    pub(super) async fn record_completion(
        mut self,
        usage: Option<&Usage>,
        saw_upstream_output: bool,
    ) {
        spend::record_finished_stream_spend_with_reservation_with_policy(
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
        )
        .await;
    }

    pub(super) async fn record_disconnect(&mut self, usage: Option<&Usage>) {
        spend::record_stream_disconnect_spend_with_reservation_with_policy(
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
        )
        .await;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use crate::config::models::gateway::GatewayPricingConfig;
    use crate::core::budget::{
        BudgetConfig, BudgetManager, BudgetScope, ResetPeriod, UnifiedBudgetReservation,
    };
    use crate::core::budget::{ModelLimitConfig, ProviderLimitConfig, UnifiedBudgetLimits};
    use crate::core::keys::{InMemoryKeyRepository, KeyManager};
    use crate::core::pricing_service::PricingService;

    use super::{ApiKeyBudgetPolicy, BudgetedCall, SettledStream};

    pub(super) fn limited_budget() -> Arc<UnifiedBudgetLimits> {
        let limits = UnifiedBudgetLimits::new();
        limits.providers.set_provider_limit(
            "openai",
            ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
        );
        limits
            .models
            .set_model_limit("gpt-4", ModelLimitConfig::new(1.0, ResetPeriod::Monthly));
        Arc::new(limits)
    }

    #[tokio::test]
    async fn reserve_failure_prevents_provider_call() {
        let limits = limited_budget();
        let called = Arc::new(AtomicBool::new(false));

        let result = BudgetedCall::new(limits.clone(), "openai", "gpt-4")
            .reserve_call_settle(
                |context| {
                    limits
                        .reserve_spend(context.provider(), context.model(), 2.0)
                        .map(Some)
                        .map_err(|error| {
                            super::spend::reservation_error_to_provider_error(
                                error,
                                context.provider(),
                                context.model(),
                            )
                        })
                },
                {
                    let called = called.clone();
                    move || async move {
                        called.store(true, Ordering::Relaxed);
                        Ok::<_, crate::core::providers::ProviderError>("should not run")
                    }
                },
                |value, _reservations, _context| async move { (value, 0) },
            )
            .await;

        assert!(result.is_err());
        assert!(!called.load(Ordering::Relaxed));
        assert_eq!(
            limits
                .providers
                .get_provider_usage("openai")
                .map(|usage| usage.current_spend)
                .unwrap_or_default(),
            0.0
        );
    }

    #[tokio::test]
    async fn provider_failure_cancels_provider_model_and_key_reservations() {
        let limits = limited_budget();
        let budget_manager = Arc::new(BudgetManager::new());
        let scope = BudgetScope::ApiKey("key-budget".to_string());
        let budget = budget_manager
            .create_budget(scope.clone(), BudgetConfig::new("key budget", 1.0))
            .await
            .expect("key budget should be created");
        let budget_id = uuid::Uuid::parse_str(&budget.id).expect("budget id should be UUID");

        let result = BudgetedCall::new(limits.clone(), "openai", "gpt-4")
            .with_api_key_budget(
                budget_manager.clone(),
                Some(budget_id),
                ApiKeyBudgetPolicy::FromProviderReservation,
            )
            .reserve_call_settle(
                |context| {
                    limits
                        .reserve_spend(context.provider(), context.model(), 0.25)
                        .map(Some)
                        .map_err(|error| {
                            super::spend::reservation_error_to_provider_error(
                                error,
                                context.provider(),
                                context.model(),
                            )
                        })
                },
                || async {
                    Err::<(), _>(crate::core::providers::ProviderError::timeout(
                        "test",
                        "upstream failed",
                    ))
                },
                |value, _reservations, _context| async move { (value, 0) },
            )
            .await;

        assert!(result.is_err());
        assert_eq!(
            limits
                .providers
                .get_provider_usage("openai")
                .expect("provider usage should exist")
                .current_spend,
            0.0
        );
        assert_eq!(
            limits
                .models
                .get_model_usage("gpt-4")
                .expect("model usage should exist")
                .current_spend,
            0.0
        );
        assert_eq!(budget_manager.get_current_spend(&scope), 0.0);
    }

    #[tokio::test]
    async fn key_reservation_mode_uses_precomputed_cost_without_provider_model_reservation() {
        let limits = limited_budget();
        let budget_manager = Arc::new(BudgetManager::new());
        let scope = BudgetScope::ApiKey("image-proxy-key-budget".to_string());
        let budget = budget_manager
            .create_budget(
                scope.clone(),
                BudgetConfig::new("image proxy key budget", 1.0),
            )
            .await
            .expect("key budget should be created");
        let budget_id = uuid::Uuid::parse_str(&budget.id).expect("budget id should be UUID");

        let ((), reservations) = BudgetedCall::new(limits.clone(), "openai", "gpt-image-1-mini")
            .with_api_key_budget(
                budget_manager.clone(),
                Some(budget_id),
                ApiKeyBudgetPolicy::RequirePricedReservation,
            )
            .with_precomputed_api_key_budget_cost(Some(0.25))
            .reserve_call(|_context| Ok(None), {
                let limits = limits.clone();
                let budget_manager = budget_manager.clone();
                let scope = scope.clone();
                move || async move {
                    assert_eq!(
                        limits
                            .providers
                            .get_provider_usage("openai")
                            .map(|usage| usage.current_spend)
                            .unwrap_or_default(),
                        0.0
                    );
                    assert_eq!(
                        limits
                            .models
                            .get_model_usage("gpt-image-1-mini")
                            .map(|usage| usage.current_spend)
                            .unwrap_or_default(),
                        0.0
                    );
                    assert_eq!(budget_manager.get_current_spend(&scope), 0.25);
                    Ok::<_, crate::core::providers::ProviderError>(())
                }
            })
            .await
            .expect("precomputed key budget should reserve without provider/model reservation");

        let (budget, key) = reservations.into_parts();
        assert!(budget.is_none());
        super::spend::settle_api_key_budget_reservation(key, 0.10, "image proxy test");
        assert_eq!(budget_manager.get_current_spend(&scope), 0.10);
    }

    #[tokio::test]
    async fn provider_model_reservation_blocks_concurrent_budget_overrun() {
        let limits = limited_budget();

        let first = BudgetedCall::new(limits.clone(), "openai", "gpt-4")
            .reserve_call(|context| context.reserve_spend(0.75).map(Some), {
                let limits = limits.clone();
                move || async move {
                    assert!(
                        limits.reserve_spend("openai", "gpt-4", 0.50).is_err(),
                        "in-flight reservation must count against remaining budget"
                    );
                    Ok::<_, crate::core::providers::ProviderError>(())
                }
            })
            .await;
        assert!(first.is_ok(), "first reservation should fit the budget");
        let (_, reservations) = match first {
            Ok(value) => value,
            Err(error) => panic!("first reservation failed unexpectedly: {error}"),
        };

        let (reservation, key_reservation) = reservations.into_parts();
        assert!(key_reservation.is_none());
        let Some(reservation) = reservation else {
            panic!("provider/model reservation should be present");
        };
        assert!(
            reservation.settle(0.75).is_ok(),
            "settlement should fit reservation"
        );
        assert_eq!(
            limits
                .providers
                .get_provider_usage("openai")
                .map(|usage| usage.current_spend),
            Some(0.75)
        );
    }

    #[test]
    fn availability_only_ensure_available_does_not_record_spend() {
        let limits = limited_budget();

        BudgetedCall::new(limits.clone(), "openai", "gpt-4")
            .ensure_available()
            .expect("availability check should pass without recording spend");

        assert_eq!(
            limits
                .providers
                .get_provider_usage("openai")
                .map(|usage| usage.current_spend)
                .unwrap_or_default(),
            0.0
        );
        assert_eq!(
            limits
                .models
                .get_model_usage("gpt-4")
                .map(|usage| usage.current_spend)
                .unwrap_or_default(),
            0.0
        );
    }

    #[test]
    fn ensure_available_rejects_metered_mode() {
        let error = BudgetedCall::new(limited_budget(), "openai", "gpt-4")
            .with_settlement_mode(ApiKeyBudgetPolicy::FromProviderReservation)
            .ensure_available()
            .expect_err("metered budget calls must use reservation settlement");

        assert_eq!(error.http_status(), 400);
    }

    #[tokio::test]
    async fn success_hands_reservations_to_settlement() {
        let limits = limited_budget();
        let settled = Arc::new(AtomicBool::new(false));

        let (value, tokens) = BudgetedCall::new(limits.clone(), "openai", "gpt-4")
            .reserve_call_settle(
                |context| {
                    limits
                        .reserve_spend(context.provider(), context.model(), 0.25)
                        .map(Some)
                        .map_err(|error| {
                            super::spend::reservation_error_to_provider_error(
                                error,
                                context.provider(),
                                context.model(),
                            )
                        })
                },
                || async { Ok::<_, crate::core::providers::ProviderError>("ok") },
                {
                    let settled = settled.clone();
                    move |value, reservations, _context| async move {
                        let (budget, key) = reservations.into_parts();
                        let settlement_failed = budget
                            .expect("provider/model reservation should be present")
                            .settle(f64::NAN)
                            .is_err();
                        assert!(key.is_none());
                        assert!(settlement_failed);
                        settled.store(true, Ordering::Relaxed);
                        (value, 17)
                    }
                },
            )
            .await
            .expect("provider success should return response even when settlement logs internally");

        assert_eq!(value, "ok");
        assert_eq!(tokens, 17);
        assert!(settled.load(Ordering::Relaxed));
    }

    pub(super) fn settled_stream(
        budget_limits: Arc<UnifiedBudgetLimits>,
        reservation: Option<UnifiedBudgetReservation>,
    ) -> SettledStream {
        let pricing_service = Arc::new(
            PricingService::with_embedded_default()
                .expect("embedded pricing should load for settled stream tests"),
        );
        let request_pricing = crate::server::routes::ai::spend::RequestPricing::from_exact(
            pricing_service.as_ref(),
            "openai",
            "gpt-4",
        );
        SettledStream {
            pricing_service,
            pricing_config: GatewayPricingConfig::default(),
            budget_limits,
            key_manager: KeyManager::new(InMemoryKeyRepository::new()),
            api_key_id: None,
            provider: "openai".to_string(),
            model: "gpt-4".to_string(),
            request_pricing,
            budget_reservation: reservation,
            key_budget_reservation: None,
        }
    }

    #[tokio::test]
    async fn settled_stream_completion_without_usage_settles_reserved_output() {
        let limits = limited_budget();
        let reservation = limits
            .reserve_spend("openai", "gpt-4", 0.25)
            .expect("reservation should fit test budget");

        settled_stream(limits.clone(), Some(reservation))
            .record_completion(None, true)
            .await;

        assert_eq!(
            limits
                .providers
                .get_provider_usage("openai")
                .expect("provider usage should be recorded")
                .current_spend,
            0.25
        );
    }

    #[tokio::test]
    async fn settled_stream_completion_without_output_settles_reserved_output() {
        let limits = limited_budget();
        let reservation = limits
            .reserve_spend("openai", "gpt-4", 0.25)
            .expect("reservation should fit test budget");

        settled_stream(limits.clone(), Some(reservation))
            .record_completion(None, false)
            .await;

        assert_eq!(
            limits
                .providers
                .get_provider_usage("openai")
                .expect("provider reservation should exist")
                .current_spend,
            0.25
        );
    }
}
