//! Pricing management API endpoints
//!
//! This module provides HTTP endpoints for managing pricing data

use crate::server::state::AppState;
use crate::utils::error::gateway_error::Result;
use actix_web::{HttpResponse, web};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// Pricing refresh request payload
#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    /// Optional: Specific source URL to refresh from
    pub source_url: Option<String>,
    /// Optional: Force refresh even if cache is still valid
    pub force: Option<bool>,
}

/// Pricing refresh response
#[derive(Debug, Serialize)]
pub struct RefreshResponse {
    /// Whether the refresh was successful
    pub success: bool,
    /// Human-readable message about the refresh operation
    pub message: String,
    /// Number of models that were updated
    pub updated_models: usize,
    /// Timestamp of the refresh operation
    pub timestamp: String,
}

/// Pricing statistics response
#[derive(Debug, Serialize)]
pub struct PricingStatsResponse {
    /// Total number of models with pricing data
    pub total_models: usize,
    /// List of available providers
    pub providers: Vec<String>,
    /// When the pricing data was last updated
    pub last_updated: String,
    /// Current cache status (fresh/stale)
    pub cache_status: String,
}

/// Refresh pricing data endpoint
/// POST /v1/pricing/refresh
pub async fn refresh_pricing(
    data: web::Data<AppState>,
    payload: web::Json<RefreshRequest>,
) -> Result<HttpResponse> {
    info!("Pricing refresh requested: {:?}", payload);

    let pricing_service = &data.pricing;

    // Check if force refresh is needed
    let needs_refresh = payload.force.unwrap_or(false) || pricing_service.needs_refresh();

    if !needs_refresh {
        return Ok(HttpResponse::Ok().json(RefreshResponse {
            success: true,
            message: "Pricing data is already up to date".to_string(),
            updated_models: 0,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }));
    }

    // Perform refresh
    match pricing_service.force_refresh().await {
        Ok(_) => {
            let stats = pricing_service.get_statistics();

            info!(
                "Pricing data refreshed successfully, {} models updated",
                stats.total_models
            );

            Ok(HttpResponse::Ok().json(RefreshResponse {
                success: true,
                message: "Pricing data refreshed successfully".to_string(),
                updated_models: stats.total_models,
                timestamp: chrono::Utc::now().to_rfc3339(),
            }))
        }
        Err(e) => {
            warn!("Failed to refresh pricing data: {}", e);

            Ok(HttpResponse::InternalServerError().json(RefreshResponse {
                success: false,
                message: format!("Failed to refresh pricing data: {}", e),
                updated_models: 0,
                timestamp: chrono::Utc::now().to_rfc3339(),
            }))
        }
    }
}

/// Get pricing statistics endpoint
/// GET /v1/pricing/stats
pub async fn get_pricing_stats(data: web::Data<AppState>) -> Result<HttpResponse> {
    let pricing_service = &data.pricing;
    let stats = pricing_service.get_statistics();

    let providers = pricing_service.get_providers();
    let last_updated = chrono::DateTime::<chrono::Utc>::from(stats.last_updated);

    let cache_status = if pricing_service.needs_refresh() {
        "stale".to_string()
    } else {
        "fresh".to_string()
    };

    Ok(HttpResponse::Ok().json(PricingStatsResponse {
        total_models: stats.total_models,
        providers,
        last_updated: last_updated.to_rfc3339(),
        cache_status,
    }))
}

/// Get pricing for a specific model
/// GET /v1/pricing/model/{model_name}
pub async fn get_model_pricing(
    data: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let model_name = path.into_inner();
    let pricing_service = &data.pricing;

    match pricing_service.get_model_info(&model_name) {
        Some(model_info) => Ok(HttpResponse::Ok().json(model_info)),
        None => Ok(HttpResponse::NotFound().json(serde_json::json!({
            "error": "Model not found",
            "model": model_name
        }))),
    }
}

/// Calculate cost for a completion
/// POST /v1/pricing/calculate
#[derive(Debug, Deserialize)]
pub struct CostCalculationRequest {
    /// Model name to calculate cost for
    pub model: String,
    /// Number of input tokens
    pub input_tokens: u32,
    /// Number of output tokens
    pub output_tokens: u32,
    /// Optional prompt text for character-based pricing
    pub prompt: Option<String>,
    /// Optional completion text for character-based pricing
    pub completion: Option<String>,
    /// Optional duration in seconds for time-based pricing
    pub duration_seconds: Option<f64>,
}

/// Calculate the cost for a specific model usage
pub async fn calculate_cost(
    data: web::Data<AppState>,
    payload: web::Json<CostCalculationRequest>,
) -> Result<HttpResponse> {
    let pricing_service = &data.pricing;

    match pricing_service
        .calculate_completion_cost(
            &payload.model,
            payload.input_tokens,
            payload.output_tokens,
            payload.prompt.as_deref(),
            payload.completion.as_deref(),
            payload.duration_seconds,
        )
        .await
    {
        Ok(cost_result) => Ok(HttpResponse::Ok().json(cost_result)),
        Err(e) => Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Cost calculation failed",
            "message": e.to_string()
        }))),
    }
}

/// Configure pricing endpoints
pub fn configure_pricing_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1/pricing")
            .route("/refresh", web::post().to(refresh_pricing))
            .route("/stats", web::get().to(get_pricing_stats))
            .route("/model/{model_name}", web::get().to(get_model_pricing))
            .route("/calculate", web::post().to(calculate_cost)),
    );
}
