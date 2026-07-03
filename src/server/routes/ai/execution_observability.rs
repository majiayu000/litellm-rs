use crate::core::providers::ProviderError;
use crate::core::router::deployment::Deployment;
use crate::core::router::execution::retryable_budget_scope;

pub(super) fn is_budget_or_unpriced_fallback(
    deployment: &Deployment,
    error: &ProviderError,
    streaming: bool,
) -> bool {
    let model_not_priced = super::super::spend::is_model_not_priced_error(error);
    if model_not_priced {
        record_candidate_exclusion(deployment, error, streaming);
    }

    retryable_budget_scope(error).is_some() || model_not_priced
}

pub(in crate::server::routes::ai) fn record_candidate_exclusion(
    deployment: &Deployment,
    error: &ProviderError,
    streaming: bool,
) {
    let provider = deployment.provider.name();
    let model = deployment.model.as_str();
    crate::server::middleware::record_unpriced_event(
        provider,
        model,
        "reject",
        "candidate_excluded",
    );
    tracing::warn!(
        provider = %provider,
        model,
        model_bucket = crate::server::middleware::unpriced_model_bucket(model),
        policy = "reject",
        outcome = "candidate_excluded",
        streaming,
        error = %error,
        "unpriced deployment excluded from router selection"
    );
}
