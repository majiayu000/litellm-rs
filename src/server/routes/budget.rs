//! Budget management HTTP API endpoints
//!
//! Provides REST API endpoints for managing per-provider and per-model budgets.
//!
//! ## Endpoints
//!
//! ### Provider Budgets
//! - `POST   /v1/budget/providers`         - Set provider budget
//! - `GET    /v1/budget/providers`         - List all provider budgets
//! - `GET    /v1/budget/providers/{name}`  - Get provider budget status
//! - `DELETE /v1/budget/providers/{name}`  - Remove provider budget
//!
//! ### Model Budgets
//! - `POST   /v1/budget/models`            - Set model budget
//! - `GET    /v1/budget/models`            - List all model budgets
//! - `GET    /v1/budget/models/{name}`     - Get model budget status
//! - `DELETE /v1/budget/models/{name}`     - Remove model budget
//!
//! All mutation endpoints (POST/DELETE/reset) require the Admin role;
//! read-only endpoints only require authentication.

use crate::core::budget::{
    BudgetStatus, Currency, ModelLimitConfig, ProviderLimitConfig, ResetPeriod, UnifiedBudgetLimits,
};
use crate::core::models::{
    ApiKey,
    user::types::{User, UserRole},
};
use crate::server::routes::ApiResponse;
use actix_web::{HttpMessage, HttpRequest, HttpResponse, Result as ActixResult, web};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, warn};

/// Request body for setting a provider budget
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetProviderBudgetRequest {
    /// Provider name (e.g., "openai", "anthropic")
    pub provider: String,
    /// Maximum budget amount
    pub max_budget: f64,
    /// Reset period
    #[serde(default = "default_reset_period")]
    pub reset_period: ResetPeriod,
    /// Soft limit percentage (0.0 to 1.0)
    #[serde(default = "default_soft_limit_percentage")]
    pub soft_limit_percentage: f64,
    /// Currency
    #[serde(default)]
    pub currency: Currency,
    /// Whether the limit is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_reset_period() -> ResetPeriod {
    ResetPeriod::Monthly
}

fn default_soft_limit_percentage() -> f64 {
    0.8
}

fn default_enabled() -> bool {
    true
}

/// Request body for setting a model budget
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetModelBudgetRequest {
    /// Model name (e.g., "gpt-4", "claude-3-opus")
    pub model: String,
    /// Maximum budget amount
    pub max_budget: f64,
    /// Reset period
    #[serde(default = "default_reset_period")]
    pub reset_period: ResetPeriod,
    /// Soft limit percentage (0.0 to 1.0)
    #[serde(default = "default_soft_limit_percentage")]
    pub soft_limit_percentage: f64,
    /// Currency
    #[serde(default)]
    pub currency: Currency,
    /// Whether the limit is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

/// Response for provider budget operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderBudgetResponse {
    /// Provider name
    pub provider: String,
    /// Maximum budget
    pub max_budget: f64,
    /// Current spend
    pub current_spend: f64,
    /// Remaining budget
    pub remaining: f64,
    /// Usage percentage
    pub usage_percentage: f64,
    /// Budget status
    pub status: BudgetStatus,
    /// Reset period
    pub reset_period: ResetPeriod,
    /// Currency
    pub currency: Currency,
    /// Whether enabled
    pub enabled: bool,
    /// Request count
    pub request_count: u64,
    /// Last reset time
    pub last_reset_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Response for model budget operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelBudgetResponse {
    /// Model name
    pub model: String,
    /// Maximum budget
    pub max_budget: f64,
    /// Current spend
    pub current_spend: f64,
    /// Remaining budget
    pub remaining: f64,
    /// Usage percentage
    pub usage_percentage: f64,
    /// Budget status
    pub status: BudgetStatus,
    /// Reset period
    pub reset_period: ResetPeriod,
    /// Currency
    pub currency: Currency,
    /// Whether enabled
    pub enabled: bool,
    /// Request count
    pub request_count: u64,
    /// Last reset time
    pub last_reset_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Response for listing provider budgets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListProviderBudgetsResponse {
    /// List of provider budgets
    pub providers: Vec<ProviderBudgetResponse>,
    /// Total count
    pub total: usize,
}

/// Response for listing model budgets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListModelBudgetsResponse {
    /// List of model budgets
    pub models: Vec<ModelBudgetResponse>,
    /// Total count
    pub total: usize,
}

/// Response for delete operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteBudgetResponse {
    /// Whether the operation was successful
    pub success: bool,
    /// Message
    pub message: String,
}

/// Budget summary response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetSummaryResponse {
    /// Total provider budgets
    pub total_provider_budgets: usize,
    /// Total model budgets
    pub total_model_budgets: usize,
    /// Providers with exceeded budgets
    pub exceeded_providers: Vec<String>,
    /// Providers with warning status
    pub warning_providers: Vec<String>,
    /// Total allocated for providers
    pub total_provider_allocated: f64,
    /// Total spent for providers
    pub total_provider_spent: f64,
    /// Total allocated for models
    pub total_model_allocated: f64,
    /// Total spent for models
    pub total_model_spent: f64,
}

// ========== Authorization ==========

/// Require the Admin role for budget mutation endpoints.
///
/// The global `AuthMiddleware` authenticates every non-public request and
/// inserts the resolved `User` (and/or `ApiKey`) into the request extensions.
/// Budget mutations affect global provider/model limits, so only admins
/// (SuperAdmin passes via the `has_role` hierarchy) may perform them.
///
/// When authentication is disabled the middleware bypasses all credential
/// checks and inserts no identity, so the role check is skipped to preserve
/// no-auth mode (same behaviour as the keys routes).
///
/// Returns `Some(403 response)` when the caller must be rejected.
fn require_admin(req: &HttpRequest, action: &str) -> Option<HttpResponse> {
    let extensions = req.extensions();

    if let Some(user) = extensions.get::<User>() {
        if user.has_role(&UserRole::Admin) {
            return None;
        }
        warn!(
            "User '{}' attempted to {} without admin role",
            user.username, action
        );
    } else if extensions.get::<ApiKey>().is_some() {
        // Team-only API key: authenticated but carries no admin user.
        warn!("Team-scoped API key attempted to {}", action);
    } else {
        // No identity in the extensions: the auth middleware ran in no-auth
        // mode, where all credential checks are intentionally bypassed.
        return None;
    }

    Some(HttpResponse::Forbidden().json(ApiResponse::<()>::error(
        "Admin role required for budget mutations".to_string(),
    )))
}

// ========== Provider Budget Handlers ==========

/// POST /v1/budget/providers - Set a provider budget
pub async fn set_provider_budget(
    req: HttpRequest,
    budget_limits: web::Data<Arc<UnifiedBudgetLimits>>,
    request: web::Json<SetProviderBudgetRequest>,
) -> ActixResult<HttpResponse> {
    if let Some(forbidden) = require_admin(&req, "set provider budget") {
        return Ok(forbidden);
    }

    // Validate request
    if request.provider.trim().is_empty() {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "Provider name cannot be empty".to_string(),
        )));
    }

    if request.max_budget <= 0.0 {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "max_budget must be greater than 0".to_string(),
        )));
    }

    if request.soft_limit_percentage < 0.0 || request.soft_limit_percentage > 1.0 {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "soft_limit_percentage must be between 0.0 and 1.0".to_string(),
        )));
    }

    let config = ProviderLimitConfig {
        max_budget: request.max_budget,
        reset_period: request.reset_period,
        soft_limit_percentage: request.soft_limit_percentage,
        currency: request.currency,
        enabled: request.enabled,
    };

    budget_limits
        .providers
        .set_provider_limit(&request.provider, config);

    info!(
        "Set provider budget for '{}': ${:.2} ({})",
        request.provider, request.max_budget, request.reset_period
    );

    // Return the created/updated budget
    match budget_limits
        .providers
        .get_provider_usage(&request.provider)
    {
        Some(usage) => {
            let response = ProviderBudgetResponse {
                provider: usage.provider_name,
                max_budget: usage.max_budget,
                current_spend: usage.current_spend,
                remaining: usage.remaining,
                usage_percentage: usage.usage_percentage,
                status: usage.status,
                reset_period: usage.reset_period,
                currency: request.currency,
                enabled: request.enabled,
                request_count: usage.request_count,
                last_reset_at: usage.last_reset_at,
            };
            Ok(HttpResponse::Created().json(ApiResponse::success(response)))
        }
        None => {
            error!("Failed to retrieve provider budget after creation");
            Ok(
                HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                    "Failed to retrieve budget".to_string(),
                )),
            )
        }
    }
}

/// GET /v1/budget/providers - List all provider budgets
pub async fn list_provider_budgets(
    budget_limits: web::Data<Arc<UnifiedBudgetLimits>>,
) -> ActixResult<HttpResponse> {
    let usage_list = budget_limits.providers.list_provider_usage();

    let providers: Vec<ProviderBudgetResponse> = usage_list
        .into_iter()
        .map(|usage| {
            // Get the budget to get currency and enabled status
            let budgets = budget_limits.providers.list_provider_budgets();
            let budget = budgets
                .iter()
                .find(|b| b.provider_name == usage.provider_name);

            ProviderBudgetResponse {
                provider: usage.provider_name,
                max_budget: usage.max_budget,
                current_spend: usage.current_spend,
                remaining: usage.remaining,
                usage_percentage: usage.usage_percentage,
                status: usage.status,
                reset_period: usage.reset_period,
                currency: budget.map(|b| b.currency).unwrap_or_default(),
                enabled: budget.map(|b| b.enabled).unwrap_or(true),
                request_count: usage.request_count,
                last_reset_at: usage.last_reset_at,
            }
        })
        .collect();

    let response = ListProviderBudgetsResponse {
        total: providers.len(),
        providers,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// GET /v1/budget/providers/{name} - Get provider budget status
pub async fn get_provider_budget(
    budget_limits: web::Data<Arc<UnifiedBudgetLimits>>,
    path: web::Path<String>,
) -> ActixResult<HttpResponse> {
    let provider_name = path.into_inner();

    match budget_limits.providers.get_provider_usage(&provider_name) {
        Some(usage) => {
            let budgets = budget_limits.providers.list_provider_budgets();
            let budget = budgets.iter().find(|b| b.provider_name == provider_name);

            let response = ProviderBudgetResponse {
                provider: usage.provider_name,
                max_budget: usage.max_budget,
                current_spend: usage.current_spend,
                remaining: usage.remaining,
                usage_percentage: usage.usage_percentage,
                status: usage.status,
                reset_period: usage.reset_period,
                currency: budget.map(|b| b.currency).unwrap_or_default(),
                enabled: budget.map(|b| b.enabled).unwrap_or(true),
                request_count: usage.request_count,
                last_reset_at: usage.last_reset_at,
            };
            Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
        }
        None => {
            warn!("Provider budget not found: {}", provider_name);
            Ok(
                HttpResponse::NotFound().json(ApiResponse::<()>::error(format!(
                    "Provider budget not found: {}",
                    provider_name
                ))),
            )
        }
    }
}

/// DELETE /v1/budget/providers/{name} - Remove provider budget
pub async fn delete_provider_budget(
    req: HttpRequest,
    budget_limits: web::Data<Arc<UnifiedBudgetLimits>>,
    path: web::Path<String>,
) -> ActixResult<HttpResponse> {
    if let Some(forbidden) = require_admin(&req, "delete provider budget") {
        return Ok(forbidden);
    }

    let provider_name = path.into_inner();

    if budget_limits
        .providers
        .remove_provider_limit(&provider_name)
    {
        info!("Removed provider budget for '{}'", provider_name);
        let response = DeleteBudgetResponse {
            success: true,
            message: format!("Provider budget '{}' removed successfully", provider_name),
        };
        Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
    } else {
        warn!("Provider budget not found for deletion: {}", provider_name);
        Ok(
            HttpResponse::NotFound().json(ApiResponse::<()>::error(format!(
                "Provider budget not found: {}",
                provider_name
            ))),
        )
    }
}

/// POST /v1/budget/providers/{name}/reset - Reset provider budget
pub async fn reset_provider_budget(
    req: HttpRequest,
    budget_limits: web::Data<Arc<UnifiedBudgetLimits>>,
    path: web::Path<String>,
) -> ActixResult<HttpResponse> {
    if let Some(forbidden) = require_admin(&req, "reset provider budget") {
        return Ok(forbidden);
    }

    let provider_name = path.into_inner();

    if budget_limits
        .providers
        .reset_provider_budget(&provider_name)
    {
        info!("Reset provider budget for '{}'", provider_name);

        match budget_limits.providers.get_provider_usage(&provider_name) {
            Some(usage) => {
                let budgets = budget_limits.providers.list_provider_budgets();
                let budget = budgets.iter().find(|b| b.provider_name == provider_name);

                let response = ProviderBudgetResponse {
                    provider: usage.provider_name,
                    max_budget: usage.max_budget,
                    current_spend: usage.current_spend,
                    remaining: usage.remaining,
                    usage_percentage: usage.usage_percentage,
                    status: usage.status,
                    reset_period: usage.reset_period,
                    currency: budget.map(|b| b.currency).unwrap_or_default(),
                    enabled: budget.map(|b| b.enabled).unwrap_or(true),
                    request_count: usage.request_count,
                    last_reset_at: usage.last_reset_at,
                };
                Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
            }
            None => Ok(
                HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                    "Failed to retrieve budget after reset".to_string(),
                )),
            ),
        }
    } else {
        warn!("Provider budget not found for reset: {}", provider_name);
        Ok(
            HttpResponse::NotFound().json(ApiResponse::<()>::error(format!(
                "Provider budget not found: {}",
                provider_name
            ))),
        )
    }
}

// ========== Model Budget Handlers ==========

/// POST /v1/budget/models - Set a model budget
pub async fn set_model_budget(
    req: HttpRequest,
    budget_limits: web::Data<Arc<UnifiedBudgetLimits>>,
    request: web::Json<SetModelBudgetRequest>,
) -> ActixResult<HttpResponse> {
    if let Some(forbidden) = require_admin(&req, "set model budget") {
        return Ok(forbidden);
    }

    // Validate request
    if request.model.trim().is_empty() {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "Model name cannot be empty".to_string(),
        )));
    }

    if request.max_budget <= 0.0 {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "max_budget must be greater than 0".to_string(),
        )));
    }

    if request.soft_limit_percentage < 0.0 || request.soft_limit_percentage > 1.0 {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "soft_limit_percentage must be between 0.0 and 1.0".to_string(),
        )));
    }

    let config = ModelLimitConfig {
        max_budget: request.max_budget,
        reset_period: request.reset_period,
        soft_limit_percentage: request.soft_limit_percentage,
        currency: request.currency,
        enabled: request.enabled,
    };

    budget_limits.models.set_model_limit(&request.model, config);

    info!(
        "Set model budget for '{}': ${:.2} ({})",
        request.model, request.max_budget, request.reset_period
    );

    // Return the created/updated budget
    match budget_limits.models.get_model_usage(&request.model) {
        Some(usage) => {
            let response = ModelBudgetResponse {
                model: usage.model_name,
                max_budget: usage.max_budget,
                current_spend: usage.current_spend,
                remaining: usage.remaining,
                usage_percentage: usage.usage_percentage,
                status: usage.status,
                reset_period: usage.reset_period,
                currency: request.currency,
                enabled: request.enabled,
                request_count: usage.request_count,
                last_reset_at: usage.last_reset_at,
            };
            Ok(HttpResponse::Created().json(ApiResponse::success(response)))
        }
        None => {
            error!("Failed to retrieve model budget after creation");
            Ok(
                HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                    "Failed to retrieve budget".to_string(),
                )),
            )
        }
    }
}

/// GET /v1/budget/models - List all model budgets
pub async fn list_model_budgets(
    budget_limits: web::Data<Arc<UnifiedBudgetLimits>>,
) -> ActixResult<HttpResponse> {
    let usage_list = budget_limits.models.list_model_usage();

    let models: Vec<ModelBudgetResponse> = usage_list
        .into_iter()
        .map(|usage| {
            let budgets = budget_limits.models.list_model_budgets();
            let budget = budgets.iter().find(|b| b.model_name == usage.model_name);

            ModelBudgetResponse {
                model: usage.model_name,
                max_budget: usage.max_budget,
                current_spend: usage.current_spend,
                remaining: usage.remaining,
                usage_percentage: usage.usage_percentage,
                status: usage.status,
                reset_period: usage.reset_period,
                currency: budget.map(|b| b.currency).unwrap_or_default(),
                enabled: budget.map(|b| b.enabled).unwrap_or(true),
                request_count: usage.request_count,
                last_reset_at: usage.last_reset_at,
            }
        })
        .collect();

    let response = ListModelBudgetsResponse {
        total: models.len(),
        models,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// GET /v1/budget/models/{name} - Get model budget status
pub async fn get_model_budget(
    budget_limits: web::Data<Arc<UnifiedBudgetLimits>>,
    path: web::Path<String>,
) -> ActixResult<HttpResponse> {
    let model_name = path.into_inner();

    match budget_limits.models.get_model_usage(&model_name) {
        Some(usage) => {
            let budgets = budget_limits.models.list_model_budgets();
            let budget = budgets.iter().find(|b| b.model_name == model_name);

            let response = ModelBudgetResponse {
                model: usage.model_name,
                max_budget: usage.max_budget,
                current_spend: usage.current_spend,
                remaining: usage.remaining,
                usage_percentage: usage.usage_percentage,
                status: usage.status,
                reset_period: usage.reset_period,
                currency: budget.map(|b| b.currency).unwrap_or_default(),
                enabled: budget.map(|b| b.enabled).unwrap_or(true),
                request_count: usage.request_count,
                last_reset_at: usage.last_reset_at,
            };
            Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
        }
        None => {
            warn!("Model budget not found: {}", model_name);
            Ok(
                HttpResponse::NotFound().json(ApiResponse::<()>::error(format!(
                    "Model budget not found: {}",
                    model_name
                ))),
            )
        }
    }
}

/// DELETE /v1/budget/models/{name} - Remove model budget
pub async fn delete_model_budget(
    req: HttpRequest,
    budget_limits: web::Data<Arc<UnifiedBudgetLimits>>,
    path: web::Path<String>,
) -> ActixResult<HttpResponse> {
    if let Some(forbidden) = require_admin(&req, "delete model budget") {
        return Ok(forbidden);
    }

    let model_name = path.into_inner();

    if budget_limits.models.remove_model_limit(&model_name) {
        info!("Removed model budget for '{}'", model_name);
        let response = DeleteBudgetResponse {
            success: true,
            message: format!("Model budget '{}' removed successfully", model_name),
        };
        Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
    } else {
        warn!("Model budget not found for deletion: {}", model_name);
        Ok(
            HttpResponse::NotFound().json(ApiResponse::<()>::error(format!(
                "Model budget not found: {}",
                model_name
            ))),
        )
    }
}

/// POST /v1/budget/models/{name}/reset - Reset model budget
pub async fn reset_model_budget(
    req: HttpRequest,
    budget_limits: web::Data<Arc<UnifiedBudgetLimits>>,
    path: web::Path<String>,
) -> ActixResult<HttpResponse> {
    if let Some(forbidden) = require_admin(&req, "reset model budget") {
        return Ok(forbidden);
    }

    let model_name = path.into_inner();

    if budget_limits.models.reset_model_budget(&model_name) {
        info!("Reset model budget for '{}'", model_name);

        match budget_limits.models.get_model_usage(&model_name) {
            Some(usage) => {
                let budgets = budget_limits.models.list_model_budgets();
                let budget = budgets.iter().find(|b| b.model_name == model_name);

                let response = ModelBudgetResponse {
                    model: usage.model_name,
                    max_budget: usage.max_budget,
                    current_spend: usage.current_spend,
                    remaining: usage.remaining,
                    usage_percentage: usage.usage_percentage,
                    status: usage.status,
                    reset_period: usage.reset_period,
                    currency: budget.map(|b| b.currency).unwrap_or_default(),
                    enabled: budget.map(|b| b.enabled).unwrap_or(true),
                    request_count: usage.request_count,
                    last_reset_at: usage.last_reset_at,
                };
                Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
            }
            None => Ok(
                HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                    "Failed to retrieve budget after reset".to_string(),
                )),
            ),
        }
    } else {
        warn!("Model budget not found for reset: {}", model_name);
        Ok(
            HttpResponse::NotFound().json(ApiResponse::<()>::error(format!(
                "Model budget not found: {}",
                model_name
            ))),
        )
    }
}

// ========== Summary and Utility Handlers ==========

/// GET /v1/budget/summary - Get overall budget summary
pub async fn get_budget_summary(
    budget_limits: web::Data<Arc<UnifiedBudgetLimits>>,
) -> ActixResult<HttpResponse> {
    let provider_usage = budget_limits.providers.list_provider_usage();
    let model_usage = budget_limits.models.list_model_usage();

    let exceeded_providers: Vec<String> = provider_usage
        .iter()
        .filter(|u| u.status == BudgetStatus::Exceeded)
        .map(|u| u.provider_name.clone())
        .collect();

    let warning_providers: Vec<String> = provider_usage
        .iter()
        .filter(|u| u.status == BudgetStatus::Warning)
        .map(|u| u.provider_name.clone())
        .collect();

    let total_provider_allocated: f64 = provider_usage.iter().map(|u| u.max_budget).sum();
    let total_provider_spent: f64 = provider_usage.iter().map(|u| u.current_spend).sum();
    let total_model_allocated: f64 = model_usage.iter().map(|u| u.max_budget).sum();
    let total_model_spent: f64 = model_usage.iter().map(|u| u.current_spend).sum();

    let response = BudgetSummaryResponse {
        total_provider_budgets: provider_usage.len(),
        total_model_budgets: model_usage.len(),
        exceeded_providers,
        warning_providers,
        total_provider_allocated,
        total_provider_spent,
        total_model_allocated,
        total_model_spent,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// Configure budget routes
pub fn configure_budget_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1/budget")
            // Provider budget routes
            .route("/providers", web::post().to(set_provider_budget))
            .route("/providers", web::get().to(list_provider_budgets))
            .route("/providers/{name}", web::get().to(get_provider_budget))
            .route(
                "/providers/{name}",
                web::delete().to(delete_provider_budget),
            )
            .route(
                "/providers/{name}/reset",
                web::post().to(reset_provider_budget),
            )
            // Model budget routes
            .route("/models", web::post().to(set_model_budget))
            .route("/models", web::get().to(list_model_budgets))
            .route("/models/{name}", web::get().to(get_model_budget))
            .route("/models/{name}", web::delete().to(delete_model_budget))
            .route("/models/{name}/reset", web::post().to(reset_model_budget))
            // Summary
            .route("/summary", web::get().to(get_budget_summary)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::dev::Service;
    use actix_web::{App, test};

    #[actix_web::test]
    async fn test_set_provider_budget() {
        let budget_limits = Arc::new(UnifiedBudgetLimits::new());
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(budget_limits))
                .configure(configure_budget_routes),
        )
        .await;

        let request = SetProviderBudgetRequest {
            provider: "openai".to_string(),
            max_budget: 1000.0,
            reset_period: ResetPeriod::Monthly,
            soft_limit_percentage: 0.8,
            currency: Currency::USD,
            enabled: true,
        };

        let req = test::TestRequest::post()
            .uri("/v1/budget/providers")
            .set_json(&request)
            .to_request();

        let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_set_provider_budget_validation() {
        let budget_limits = Arc::new(UnifiedBudgetLimits::new());
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(budget_limits))
                .configure(configure_budget_routes),
        )
        .await;

        // Test empty provider name
        let request = SetProviderBudgetRequest {
            provider: "".to_string(),
            max_budget: 1000.0,
            reset_period: ResetPeriod::Monthly,
            soft_limit_percentage: 0.8,
            currency: Currency::USD,
            enabled: true,
        };

        let req = test::TestRequest::post()
            .uri("/v1/budget/providers")
            .set_json(&request)
            .to_request();

        let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 400);

        // Test negative budget
        let request = SetProviderBudgetRequest {
            provider: "openai".to_string(),
            max_budget: -100.0,
            reset_period: ResetPeriod::Monthly,
            soft_limit_percentage: 0.8,
            currency: Currency::USD,
            enabled: true,
        };

        let req = test::TestRequest::post()
            .uri("/v1/budget/providers")
            .set_json(&request)
            .to_request();

        let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 400);
    }

    #[actix_web::test]
    async fn test_list_provider_budgets() {
        let budget_limits = Arc::new(UnifiedBudgetLimits::new());
        budget_limits.providers.set_provider_limit(
            "openai",
            ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
        );
        budget_limits.providers.set_provider_limit(
            "anthropic",
            ProviderLimitConfig::new(500.0, ResetPeriod::Monthly),
        );

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(budget_limits))
                .configure(configure_budget_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/v1/budget/providers")
            .to_request();

        let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_get_provider_budget() {
        let budget_limits = Arc::new(UnifiedBudgetLimits::new());
        budget_limits.providers.set_provider_limit(
            "openai",
            ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
        );

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(budget_limits))
                .configure(configure_budget_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/v1/budget/providers/openai")
            .to_request();

        let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_get_provider_budget_not_found() {
        let budget_limits = Arc::new(UnifiedBudgetLimits::new());
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(budget_limits))
                .configure(configure_budget_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/v1/budget/providers/nonexistent")
            .to_request();

        let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[actix_web::test]
    async fn test_delete_provider_budget() {
        let budget_limits = Arc::new(UnifiedBudgetLimits::new());
        budget_limits.providers.set_provider_limit(
            "openai",
            ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
        );

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(budget_limits))
                .configure(configure_budget_routes),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri("/v1/budget/providers/openai")
            .to_request();

        let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_set_model_budget() {
        let budget_limits = Arc::new(UnifiedBudgetLimits::new());
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(budget_limits))
                .configure(configure_budget_routes),
        )
        .await;

        let request = SetModelBudgetRequest {
            model: "gpt-4".to_string(),
            max_budget: 500.0,
            reset_period: ResetPeriod::Monthly,
            soft_limit_percentage: 0.8,
            currency: Currency::USD,
            enabled: true,
        };

        let req = test::TestRequest::post()
            .uri("/v1/budget/models")
            .set_json(&request)
            .to_request();

        let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_list_model_budgets() {
        let budget_limits = Arc::new(UnifiedBudgetLimits::new());
        budget_limits
            .models
            .set_model_limit("gpt-4", ModelLimitConfig::new(500.0, ResetPeriod::Monthly));

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(budget_limits))
                .configure(configure_budget_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/v1/budget/models")
            .to_request();

        let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_get_budget_summary() {
        let budget_limits = Arc::new(UnifiedBudgetLimits::new());
        budget_limits.providers.set_provider_limit(
            "openai",
            ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
        );
        budget_limits
            .models
            .set_model_limit("gpt-4", ModelLimitConfig::new(500.0, ResetPeriod::Monthly));

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(budget_limits))
                .configure(configure_budget_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/v1/budget/summary")
            .to_request();

        let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    // ========== RBAC tests ==========

    fn make_test_user(role: UserRole) -> User {
        use crate::core::models::Metadata;
        use crate::core::models::UsageStats;
        use crate::core::models::user::preferences::UserPreferences;
        use crate::core::models::user::types::{UserProfile, UserStatus};
        User {
            metadata: Metadata::new(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            display_name: None,
            password_hash: "hash".to_string(),
            role,
            status: UserStatus::Active,
            team_ids: vec![],
            preferences: UserPreferences::default(),
            usage_stats: UsageStats::default(),
            rate_limits: None,
            last_login_at: None,
            email_verified: true,
            two_factor_enabled: false,
            profile: UserProfile::default(),
        }
    }

    fn make_team_api_key() -> ApiKey {
        use crate::core::models::{Metadata, UsageStats};
        ApiKey {
            metadata: Metadata::new(),
            name: "team-key".to_string(),
            key_hash: "hash".to_string(),
            key_prefix: "gw-test".to_string(),
            user_id: None,
            team_id: Some(uuid::Uuid::new_v4()),
            permissions: vec![],
            rate_limits: None,
            expires_at: None,
            is_active: true,
            last_used_at: None,
            usage_stats: UsageStats::default(),
        }
    }

    #[actix_web::test]
    async fn test_budget_mutations_forbidden_for_non_admin() {
        let budget_limits = Arc::new(UnifiedBudgetLimits::new());
        budget_limits.providers.set_provider_limit(
            "openai",
            ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
        );
        budget_limits
            .models
            .set_model_limit("gpt-4", ModelLimitConfig::new(500.0, ResetPeriod::Monthly));

        let user = make_test_user(UserRole::User);
        let app = test::init_service(
            App::new()
                .wrap_fn(move |req, srv| {
                    req.extensions_mut().insert::<User>(user.clone());
                    srv.call(req)
                })
                .app_data(web::Data::new(Arc::clone(&budget_limits)))
                .configure(configure_budget_routes),
        )
        .await;

        let provider_body = SetProviderBudgetRequest {
            provider: "openai".to_string(),
            max_budget: 1.0,
            reset_period: ResetPeriod::Monthly,
            soft_limit_percentage: 0.8,
            currency: Currency::USD,
            enabled: true,
        };
        let model_body = SetModelBudgetRequest {
            model: "gpt-4".to_string(),
            max_budget: 1.0,
            reset_period: ResetPeriod::Monthly,
            soft_limit_percentage: 0.8,
            currency: Currency::USD,
            enabled: true,
        };

        let requests = vec![
            test::TestRequest::post()
                .uri("/v1/budget/providers")
                .set_json(&provider_body)
                .to_request(),
            test::TestRequest::delete()
                .uri("/v1/budget/providers/openai")
                .to_request(),
            test::TestRequest::post()
                .uri("/v1/budget/providers/openai/reset")
                .to_request(),
            test::TestRequest::post()
                .uri("/v1/budget/models")
                .set_json(&model_body)
                .to_request(),
            test::TestRequest::delete()
                .uri("/v1/budget/models/gpt-4")
                .to_request(),
            test::TestRequest::post()
                .uri("/v1/budget/models/gpt-4/reset")
                .to_request(),
        ];

        for req in requests {
            let path = req.path().to_string();
            let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 403, "expected 403 for {}", path);
        }

        // The original budgets must be untouched.
        assert!(
            budget_limits
                .providers
                .get_provider_usage("openai")
                .is_some()
        );
        assert!(budget_limits.models.get_model_usage("gpt-4").is_some());
    }

    #[actix_web::test]
    async fn test_budget_mutations_allowed_for_admin() {
        let budget_limits = Arc::new(UnifiedBudgetLimits::new());
        let user = make_test_user(UserRole::Admin);
        let app = test::init_service(
            App::new()
                .wrap_fn(move |req, srv| {
                    req.extensions_mut().insert::<User>(user.clone());
                    srv.call(req)
                })
                .app_data(web::Data::new(Arc::clone(&budget_limits)))
                .configure(configure_budget_routes),
        )
        .await;

        // Set provider budget
        let request = SetProviderBudgetRequest {
            provider: "openai".to_string(),
            max_budget: 1000.0,
            reset_period: ResetPeriod::Monthly,
            soft_limit_percentage: 0.8,
            currency: Currency::USD,
            enabled: true,
        };
        let req = test::TestRequest::post()
            .uri("/v1/budget/providers")
            .set_json(&request)
            .to_request();
        let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        // Reset provider budget
        let req = test::TestRequest::post()
            .uri("/v1/budget/providers/openai/reset")
            .to_request();
        let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        // Delete provider budget
        let req = test::TestRequest::delete()
            .uri("/v1/budget/providers/openai")
            .to_request();
        let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_budget_mutations_forbidden_for_team_api_key() {
        let budget_limits = Arc::new(UnifiedBudgetLimits::new());
        let api_key = make_team_api_key();
        let app = test::init_service(
            App::new()
                .wrap_fn(move |req, srv| {
                    req.extensions_mut().insert::<ApiKey>(api_key.clone());
                    srv.call(req)
                })
                .app_data(web::Data::new(Arc::clone(&budget_limits)))
                .configure(configure_budget_routes),
        )
        .await;

        let request = SetProviderBudgetRequest {
            provider: "openai".to_string(),
            max_budget: 1000.0,
            reset_period: ResetPeriod::Monthly,
            soft_limit_percentage: 0.8,
            currency: Currency::USD,
            enabled: true,
        };
        let req = test::TestRequest::post()
            .uri("/v1/budget/providers")
            .set_json(&request)
            .to_request();
        let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);
        assert!(
            budget_limits
                .providers
                .get_provider_usage("openai")
                .is_none()
        );
    }

    #[actix_web::test]
    async fn test_read_endpoints_allowed_for_non_admin() {
        let budget_limits = Arc::new(UnifiedBudgetLimits::new());
        budget_limits.providers.set_provider_limit(
            "openai",
            ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
        );

        let user = make_test_user(UserRole::User);
        let app = test::init_service(
            App::new()
                .wrap_fn(move |req, srv| {
                    req.extensions_mut().insert::<User>(user.clone());
                    srv.call(req)
                })
                .app_data(web::Data::new(Arc::clone(&budget_limits)))
                .configure(configure_budget_routes),
        )
        .await;

        for uri in [
            "/v1/budget/providers",
            "/v1/budget/providers/openai",
            "/v1/budget/summary",
        ] {
            let req = test::TestRequest::get().uri(uri).to_request();
            let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
            assert!(resp.status().is_success(), "expected success for {}", uri);
        }
    }
}
