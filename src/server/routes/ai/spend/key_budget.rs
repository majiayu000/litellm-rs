use uuid::Uuid;

use crate::core::budget::{
    BudgetManager, BudgetReservation, BudgetReservationError, BudgetScope, UnifiedBudgetReservation,
};
use crate::core::providers::unified_provider::ProviderError;

pub(in crate::server::routes::ai) fn reserve_api_key_budget(
    budget_manager: &BudgetManager,
    api_key_budget_id: Option<Uuid>,
    estimated_cost: Option<f64>,
) -> Result<Option<BudgetReservation>, ProviderError> {
    let Some(budget_id) = api_key_budget_id else {
        return Ok(None);
    };
    let Some(estimated_cost) = estimated_cost else {
        return Err(ProviderError::invalid_request(
            "pricing",
            format!("pricing is required for API key budget '{budget_id}'"),
        ));
    };
    if estimated_cost <= 0.0 {
        return Ok(None);
    }

    let scope = key_budget_scope(budget_manager, budget_id)?;
    budget_manager
        .tracker()
        .reserve_spend(&scope, estimated_cost)
        .map(Some)
        .map_err(|error| key_reservation_error_to_provider_error(error, budget_id))
}

pub(in crate::server::routes::ai) fn reserve_api_key_budget_for_reservation(
    budget_manager: &BudgetManager,
    api_key_budget_id: Option<Uuid>,
    budget_reservation: Option<&UnifiedBudgetReservation>,
) -> Result<Option<BudgetReservation>, ProviderError> {
    let Some(budget_reservation) = budget_reservation else {
        return Ok(None);
    };
    reserve_api_key_budget(
        budget_manager,
        api_key_budget_id,
        Some(budget_reservation.reserved_amount()),
    )
}

pub(in crate::server::routes::ai) fn settle_api_key_budget_reservation(
    reservation: Option<BudgetReservation>,
    actual_cost: f64,
    context: &str,
) {
    let Some(reservation) = reservation else {
        return;
    };

    if let Err(error) = reservation.settle(actual_cost) {
        tracing::error!("failed to settle API key budget for {context}: {error:?}");
    }
}

fn key_budget_scope(
    budget_manager: &BudgetManager,
    budget_id: Uuid,
) -> Result<BudgetScope, ProviderError> {
    budget_manager
        .get_budget_by_id(&budget_id.to_string())
        .map(|budget| budget.scope)
        .ok_or_else(|| {
            ProviderError::quota_exceeded(
                "budget",
                format!("API key budget '{budget_id}' is not configured"),
            )
        })
}

fn key_reservation_error_to_provider_error(
    error: BudgetReservationError,
    budget_id: Uuid,
) -> ProviderError {
    match error {
        BudgetReservationError::BudgetExceeded => ProviderError::quota_exceeded(
            "budget",
            format!("API key budget '{budget_id}' exceeded"),
        ),
        BudgetReservationError::InvalidAmount(error) => ProviderError::invalid_request(
            "budget",
            format!("invalid API key budget reservation amount for '{budget_id}': {error}"),
        ),
        BudgetReservationError::ActualExceedsReservation => ProviderError::invalid_request(
            "budget",
            format!("actual spend exceeded reserved API key budget '{budget_id}'"),
        ),
        BudgetReservationError::ProviderBudgetExceeded
        | BudgetReservationError::ModelBudgetExceeded => ProviderError::quota_exceeded(
            "budget",
            format!("API key budget '{budget_id}' exceeded"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::budget::BudgetConfig;

    #[tokio::test]
    async fn reserve_api_key_budget_rejects_unknown_budget_id() {
        let manager = BudgetManager::new();
        let budget_id = Uuid::new_v4();

        let error = match reserve_api_key_budget(&manager, Some(budget_id), Some(0.01)) {
            Ok(_) => panic!("unknown API key budget should fail closed"),
            Err(error) => error,
        };

        assert!(matches!(error, ProviderError::QuotaExceeded { .. }));
    }

    #[tokio::test]
    async fn reserve_api_key_budget_uses_configured_budget_scope() {
        let manager = BudgetManager::new();
        let scope = BudgetScope::ApiKey("key-budget-scope".to_string());
        let budget = match manager
            .create_budget(scope.clone(), BudgetConfig::new("key budget", 1.0))
            .await
        {
            Ok(budget) => budget,
            Err(error) => panic!("key budget should be created: {error}"),
        };
        let budget_id = match Uuid::parse_str(&budget.id) {
            Ok(budget_id) => budget_id,
            Err(error) => panic!("budget id should be a UUID string: {error}"),
        };

        let reservation = match reserve_api_key_budget(&manager, Some(budget_id), Some(0.25)) {
            Ok(Some(reservation)) => reservation,
            Ok(None) => panic!("configured API key budget should reserve spend"),
            Err(error) => panic!("configured API key budget should reserve: {error}"),
        };
        assert_eq!(manager.get_current_spend(&scope), 0.25);

        settle_api_key_budget_reservation(Some(reservation), 0.10, "test");

        assert_eq!(manager.get_current_spend(&scope), 0.10);
    }

    #[tokio::test]
    async fn reserve_api_key_budget_allows_zero_cost_priced_request() {
        let manager = BudgetManager::new();
        let scope = BudgetScope::ApiKey("free-key-budget-scope".to_string());
        let budget = match manager
            .create_budget(scope.clone(), BudgetConfig::new("free key budget", 1.0))
            .await
        {
            Ok(budget) => budget,
            Err(error) => panic!("key budget should be created: {error}"),
        };
        let budget_id = match Uuid::parse_str(&budget.id) {
            Ok(budget_id) => budget_id,
            Err(error) => panic!("budget id should be a UUID string: {error}"),
        };

        let reservation = match reserve_api_key_budget(&manager, Some(budget_id), Some(0.0)) {
            Ok(reservation) => reservation,
            Err(error) => panic!("zero-cost priced request should not fail: {error}"),
        };

        assert!(reservation.is_none());
        assert_eq!(manager.get_current_spend(&scope), 0.0);
    }

    #[tokio::test]
    async fn reserve_api_key_budget_for_absent_reservation_is_noop() {
        let manager = BudgetManager::new();
        let budget_id = Uuid::new_v4();

        let reservation =
            match reserve_api_key_budget_for_reservation(&manager, Some(budget_id), None) {
                Ok(reservation) => reservation,
                Err(error) => panic!("absent upstream reservation should not fail: {error}"),
            };

        assert!(reservation.is_none());
    }
}
