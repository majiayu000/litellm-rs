use uuid::Uuid;

use crate::server::state::AppState;

use super::types::UpdateKeyRequest;

pub(super) async fn validate_create_key_budget_fields(
    state: &AppState,
    budget_id: Option<Uuid>,
    max_budget: Option<f64>,
) -> Result<(), String> {
    reject_unsupported_max_budget(max_budget)?;
    validate_referenced_budget(state, budget_id).await
}

pub(super) async fn validate_update_key_budget_fields(
    state: &AppState,
    request: &UpdateKeyRequest,
) -> Result<(), String> {
    reject_unsupported_max_budget(request.max_budget)?;
    if let Some(Some(budget_id)) = request.budget_id {
        validate_referenced_budget(state, Some(budget_id)).await?;
    }
    Ok(())
}

pub(super) async fn validate_referenced_budget(
    state: &AppState,
    budget_id: Option<Uuid>,
) -> Result<(), String> {
    let Some(budget_id) = budget_id else {
        return Ok(());
    };
    if state
        .budget_manager
        .get_budget_by_id(&budget_id.to_string())
        .is_some()
    {
        Ok(())
    } else {
        Err(format!("API key budget '{budget_id}' is not configured"))
    }
}

fn reject_unsupported_max_budget(max_budget: Option<f64>) -> Result<(), String> {
    if max_budget.is_some() {
        return Err(
            "max_budget is not supported for persisted API keys; create a budget and pass budget_id"
                .to_string(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_budget_is_rejected_until_api_key_budgets_are_persisted() {
        assert!(reject_unsupported_max_budget(Some(1.0)).is_err());
        assert!(reject_unsupported_max_budget(None).is_ok());
    }
}
