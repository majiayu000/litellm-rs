//! Content-policy fallback after an output-guardrail Block.

use std::collections::HashSet;
use std::sync::Arc;

use crate::core::providers::{Provider, ProviderError};
use crate::core::router::deployment::Deployment;
use crate::core::router::{FallbackType, UnifiedRouter};
use crate::core::types::model::ProviderCapability;
use crate::server::guardrails::{OUTPUT_BLOCK_MESSAGE, OutputDecisionBinding};
use crate::utils::error::gateway_error::GatewayError;

use super::execution::{
    StreamingDeploymentLease, execute_stream_with_selected_deployment_matching,
};
use super::stream_output_guardrail::StreamGuardrailError;

pub(super) struct SelectedAttempt<T> {
    pub value: T,
    pub provider: String,
    pub deployment_id: String,
}

pub(super) fn is_output_guardrail_block(error: &GatewayError) -> bool {
    matches!(error, GatewayError::Forbidden(message) if message == OUTPUT_BLOCK_MESSAGE)
}

pub(super) fn allow_uncommitted_stream_fallback(
    error: StreamGuardrailError,
    committed: bool,
) -> bool {
    error == StreamGuardrailError::Violation && !committed
}

/// ContentPolicy chain, or General when that list is empty. Capped and deduped.
pub(super) fn content_policy_fallback_models(
    router: &UnifiedRouter,
    requested_model: &str,
) -> Vec<String> {
    let cap = router.config().max_fallbacks as usize;
    let mut seen = HashSet::new();
    router
        .get_fallbacks(requested_model, FallbackType::ContentPolicy)
        .into_iter()
        .filter(|model| seen.insert(model.clone()))
        .take(cap)
        .collect()
}

/// Original model plus content-policy (or general) fallbacks, capped at `1 + max_fallbacks`.
pub(super) fn models_to_try(router: &UnifiedRouter, requested_model: &str) -> Vec<String> {
    let mut models = vec![requested_model.to_string()];
    let mut seen = HashSet::from([requested_model.to_string()]);
    for fallback in content_policy_fallback_models(router, requested_model) {
        if seen.insert(fallback.clone()) {
            models.push(fallback);
        }
    }
    models
}

pub(super) fn output_binding<'a>(
    original_deployment: Option<&'a str>,
    provider: &'a str,
    deployment_id: &'a str,
) -> OutputDecisionBinding<'a> {
    match original_deployment {
        Some(original) => {
            OutputDecisionBinding::fallback(Some(provider), Some(original), Some(deployment_id))
        }
        None => OutputDecisionBinding::primary(Some(provider), Some(deployment_id)),
    }
}

pub(super) async fn next_uncommitted_stream<T, F, Fut>(
    router: Arc<UnifiedRouter>,
    capability: ProviderCapability,
    fallback_models: &mut std::vec::IntoIter<String>,
    excluded: &HashSet<String>,
    operation: F,
) -> Option<(T, StreamingDeploymentLease)>
where
    F: Fn(Provider, String, String) -> Fut + Clone,
    Fut: std::future::Future<Output = Result<T, ProviderError>>,
{
    for model in fallback_models.by_ref() {
        let excluded = excluded.clone();
        match execute_stream_with_selected_deployment_matching(
            router.clone(),
            &model,
            capability.clone(),
            move |deployment: &Deployment| !excluded.contains(deployment.id.as_str()),
            operation.clone(),
        )
        .await
        {
            Ok(next) => return Some(next),
            Err(_) => continue,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::router::{FallbackConfig, RouterConfig, UnifiedRouter};

    #[test]
    fn output_block_is_not_input_block() {
        assert!(is_output_guardrail_block(&GatewayError::Forbidden(
            OUTPUT_BLOCK_MESSAGE.to_string()
        )));
        assert!(!is_output_guardrail_block(&GatewayError::Forbidden(
            "Request blocked by input guardrails".to_string()
        )));
        assert!(!is_output_guardrail_block(&GatewayError::BadRequest(
            OUTPUT_BLOCK_MESSAGE.to_string()
        )));
    }

    #[test]
    fn uncommitted_violation_can_fallback_execution_cannot() {
        assert!(allow_uncommitted_stream_fallback(
            StreamGuardrailError::Violation,
            false
        ));
        assert!(!allow_uncommitted_stream_fallback(
            StreamGuardrailError::Violation,
            true
        ));
        assert!(!allow_uncommitted_stream_fallback(
            StreamGuardrailError::Execution,
            false
        ));
    }

    #[test]
    fn prefers_content_policy_then_general_and_caps_dedup() {
        let router = UnifiedRouter::default().with_fallback_config(
            FallbackConfig::new()
                .add_content_policy(
                    "gpt-4o",
                    vec![
                        "safe-a".to_string(),
                        "safe-a".to_string(),
                        "safe-b".to_string(),
                    ],
                )
                .add_general("gpt-4o", vec!["general".to_string()]),
        );
        assert_eq!(
            content_policy_fallback_models(&router, "gpt-4o"),
            vec!["safe-a".to_string(), "safe-b".to_string()]
        );
        assert_eq!(
            models_to_try(&router, "gpt-4o"),
            vec![
                "gpt-4o".to_string(),
                "safe-a".to_string(),
                "safe-b".to_string()
            ]
        );

        let general_only = UnifiedRouter::default().with_fallback_config(
            FallbackConfig::new().add_general("gpt-4o", vec!["general".to_string()]),
        );
        assert_eq!(
            content_policy_fallback_models(&general_only, "gpt-4o"),
            vec!["general".to_string()]
        );

        let capped = UnifiedRouter::new(RouterConfig {
            max_fallbacks: 1,
            ..RouterConfig::default()
        })
        .with_fallback_config(
            FallbackConfig::new()
                .add_content_policy("gpt-4o", vec!["a".to_string(), "b".to_string()]),
        );
        assert_eq!(
            content_policy_fallback_models(&capped, "gpt-4o"),
            vec!["a".to_string()]
        );
        assert_eq!(
            models_to_try(&capped, "gpt-4o"),
            vec!["gpt-4o".to_string(), "a".to_string()]
        );

        let empty = UnifiedRouter::default();
        assert!(content_policy_fallback_models(&empty, "gpt-4o").is_empty());
        assert_eq!(models_to_try(&empty, "gpt-4o"), vec!["gpt-4o".to_string()]);
    }

    #[test]
    fn does_not_duplicate_the_requested_model_in_the_try_list() {
        let router = UnifiedRouter::default().with_fallback_config(
            FallbackConfig::new().add_content_policy("gpt-4o", vec!["gpt-4o".to_string()]),
        );
        assert_eq!(models_to_try(&router, "gpt-4o"), vec!["gpt-4o".to_string()]);
    }
}
