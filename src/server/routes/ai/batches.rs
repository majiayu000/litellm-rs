//! Batch API route handlers.
//!
//! These endpoints are mounted to keep OpenAI-compatible batch paths stable.
//! Business logic is not implemented yet, so handlers return explicit 501.

use crate::utils::error::gateway_error::GatewayError;
use actix_web::{HttpResponse, ResponseError, Result as ActixResult, web};
use tracing::warn;

/// Create a batch request.
pub async fn create_batch() -> ActixResult<HttpResponse> {
    warn!("Batch create endpoint is not implemented");
    Ok(
        GatewayError::not_implemented("Batch create endpoint is not implemented yet")
            .error_response(),
    )
}

/// List batches.
pub async fn list_batches() -> ActixResult<HttpResponse> {
    warn!("Batch list endpoint is not implemented");
    Ok(
        GatewayError::not_implemented("Batch list endpoint is not implemented yet")
            .error_response(),
    )
}

/// Get a batch by ID.
pub async fn get_batch(batch_id: web::Path<String>) -> ActixResult<HttpResponse> {
    warn!(
        batch_id = %batch_id.as_str(),
        "Batch retrieve endpoint is not implemented"
    );
    Ok(
        GatewayError::not_implemented("Batch retrieve endpoint is not implemented yet")
            .error_response(),
    )
}

/// Cancel a batch by ID.
pub async fn cancel_batch(batch_id: web::Path<String>) -> ActixResult<HttpResponse> {
    warn!(
        batch_id = %batch_id.as_str(),
        "Batch cancel endpoint is not implemented"
    );
    Ok(
        GatewayError::not_implemented("Batch cancel endpoint is not implemented yet")
            .error_response(),
    )
}
