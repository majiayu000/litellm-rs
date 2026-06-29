//! Rerank provider implementations

mod cohere;
mod jina;

use crate::core::providers::ProviderError;
use crate::utils::error::gateway_error::GatewayError;
use reqwest::StatusCode;

pub use cohere::CohereRerankProvider;
pub use jina::JinaRerankProvider;

pub(crate) fn rerank_upstream_error(
    provider: &'static str,
    status: StatusCode,
    body: String,
) -> GatewayError {
    let message = if body.trim().is_empty() {
        format!("rerank error ({status})")
    } else {
        format!("rerank error ({status}): {body}")
    };

    GatewayError::Provider(ProviderError::api_error(provider, status.as_u16(), message))
}
