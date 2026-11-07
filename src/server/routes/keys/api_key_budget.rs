use super::types::UpdateKeyRequest;
use crate::server::state::AppState;
use uuid::Uuid;

pub(super) async fn validate_create_key_budget_fields(
    state: &AppState,
    budget_id: Option<Uuid>,
    max_budget: Option<f64>,
) -> Result<(), String> {
    ensure_persistent_database(state, max_budget)?;
    reject_external_binding(budget_id)
}

pub(super) async fn validate_update_key_budget_fields(
    state: &AppState,
    request: &UpdateKeyRequest,
) -> Result<(), String> {
    ensure_persistent_database(state, request.max_budget)?;
    reject_external_binding(request.budget_id.flatten())?;
    Ok(())
}

fn reject_external_binding(budget_id: Option<Uuid>) -> Result<(), String> {
    if budget_id.is_some() {
        return Err("Use max_budget to configure a key-owned budget; binding another key's budget_id is not supported".into());
    }
    Ok(())
}

fn ensure_persistent_database(state: &AppState, max_budget: Option<f64>) -> Result<(), String> {
    if max_budget.is_some()
        && (!state.config().gateway.storage.database.enabled
            || state.storage.database.is_sqlite_fallback()
            || state
                .config
                .load()
                .gateway
                .storage
                .database
                .url
                .contains(":memory:"))
    {
        return Err("max_budget requires an enabled persistent SQL database; SQLite fallback is not supported".into());
    }
    Ok(())
}

pub(super) async fn budget_usage(
    state: &AppState,
    id: Option<Uuid>,
) -> Result<Option<super::types::KeyBudgetUsage>, String> {
    let Some(id) = id else {
        return Ok(None);
    };
    let budget = state
        .storage
        .database
        .api_key_budget(id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Bound API key budget is missing".to_string())?;
    Ok(Some(super::types::KeyBudgetUsage {
        id,
        max_budget: budget.max_amount as f64 / 1e9,
        spent_and_reserved: budget.used_amount as f64 / 1e9,
        remaining: (budget.max_amount - budget.used_amount).max(0) as f64 / 1e9,
    }))
}
