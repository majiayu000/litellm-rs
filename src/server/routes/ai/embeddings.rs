//! Embeddings endpoint

use crate::core::models::openai::{EmbeddingRequest, EmbeddingResponse};
use crate::core::pricing_service::PricingUsage;
use crate::core::types::{
    context::RequestContext, embedding::EmbeddingInput,
    embedding::EmbeddingRequest as CoreEmbeddingRequest, model::ProviderCapability,
};
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use tracing::info;

use super::budgeted::{ApiKeyBudgetPolicy, run_unary};
use super::callbacks::CallbackLifecycle;
use super::context::handle_ai_request;

enum EmbeddingAttemptResponse {
    Cached(EmbeddingResponse),
    Provider(crate::core::types::responses::EmbeddingResponse),
}

fn parse_embedding_input(input: &serde_json::Value) -> Result<EmbeddingInput, GatewayError> {
    match input {
        serde_json::Value::String(s) => Ok(EmbeddingInput::Text(s.clone())),
        serde_json::Value::Array(arr) => {
            let mut texts = Vec::with_capacity(arr.len());
            for (index, value) in arr.iter().enumerate() {
                let Some(text) = value.as_str() else {
                    return Err(GatewayError::validation(format!(
                        "Invalid input: array element at index {} must be a string, got {}",
                        index,
                        json_value_type(value)
                    )));
                };
                texts.push(text.to_string());
            }
            Ok(EmbeddingInput::Array(texts))
        }
        _ => Err(GatewayError::validation(
            "Invalid input: expected string or array of strings",
        )),
    }
}

fn json_value_type(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

/// Embeddings endpoint
///
/// OpenAI-compatible embeddings API for generating text embeddings.
pub async fn embeddings(
    state: web::Data<AppState>,
    req: HttpRequest,
    request: web::Json<EmbeddingRequest>,
) -> ActixResult<HttpResponse> {
    info!("Embedding request for model: {}", request.model);
    let request = request.into_inner();
    if let Err(error) =
        super::context::enforce_api_key_model_and_token_limits(&req, &request.model, None)
    {
        return Ok(super::openai_errors::gateway_error_response(&error));
    }

    handle_ai_request(&req, request, "Embedding", |request, context| {
        handle_embedding_with_state(state.get_ref(), request, context)
    })
    .await
}

/// Handle embedding with app state (UnifiedRouter only)
pub async fn handle_embedding_with_state(
    state: &AppState,
    request: EmbeddingRequest,
    context: RequestContext,
) -> Result<EmbeddingResponse, GatewayError> {
    handle_embedding_internal(state, request, context).await
}

async fn handle_embedding_internal(
    state: &AppState,
    request: EmbeddingRequest,
    context: RequestContext,
) -> Result<EmbeddingResponse, GatewayError> {
    // Convert OpenAI format request to core format.
    let input = parse_embedding_input(&request.input)?;
    let input_count = match &input {
        EmbeddingInput::Text(_) => 1,
        EmbeddingInput::Array(inputs) => inputs.len(),
    };

    if request.model.trim().is_empty() {
        return Err(GatewayError::validation("Model is required"));
    }
    let cached_response =
        super::response_cache::lookup_embedding(state, &request, &context).await?;
    let request_for_cache = request.clone();

    let requested_model = request.model.clone();
    let core_request = CoreEmbeddingRequest {
        model: requested_model,
        input,
        user: request.user,
        encoding_format: None,
        dimensions: None,
        task_type: None,
    };

    let requested_model = core_request.model.clone();
    let callback = CallbackLifecycle::new_embedding(
        &state.callbacks,
        state.budgeted.pricing(),
        &requested_model,
        input_count,
        &context,
    );
    let context_for_execution = context.clone();
    let api_key_id = context.api_key_id();
    let api_key_budget_id = context.api_key_budget_id();
    let budgeted = state.budgeted.clone();
    let key_manager = budgeted.key_manager();
    let pricing_service = budgeted.pricing();
    let pricing_config = state.config().gateway.pricing.clone();
    let callback_for_execution = callback.clone();
    let core_response = match run_unary(
        &state.unified_router,
        &requested_model,
        ProviderCapability::Embeddings,
        move |provider, selected_model, _deployment_id| {
            let core_request = core_request.clone();
            let context = context_for_execution.clone();
            let budgeted = budgeted.clone();
            let key_manager = key_manager.clone();
            let pricing_service = pricing_service.clone();
            let pricing_config = pricing_config.clone();
            let callback = callback_for_execution.clone();
            let cached_response = cached_response.clone();
            async move {
                let budget_provider = provider.name().to_string();
                let request_pricing = super::spend::request_pricing_for_provider(
                    &pricing_service,
                    &provider,
                    &selected_model,
                    ProviderCapability::Embeddings,
                )?;
                if let Some(cached) = cached_response {
                    super::response_cache::ensure_embedding_cache_pricing_for_attempt(
                        &request_pricing,
                        &core_request.input,
                        &budget_provider,
                        &selected_model,
                    )?;
                    return Ok((EmbeddingAttemptResponse::Cached(cached), 0));
                }
                let mut request_for_provider = core_request.clone();
                request_for_provider.model = selected_model.clone();
                let settle_pricing_service = pricing_service.clone();
                let reserve_pricing_config = pricing_config.clone();
                let settle_pricing_config = pricing_config;
                let reserve_request_pricing = request_pricing.clone();
                let settle_request_pricing = request_pricing.clone();
                let settle_key_manager = key_manager.clone();
                let callback_provider = budget_provider.clone();
                let callback_model = selected_model.clone();
                let callback_request_pricing = request_pricing;
                budgeted
                    .for_selected_with_api_key_budget(
                        budget_provider.clone(),
                        selected_model.clone(),
                        api_key_budget_id,
                        ApiKeyBudgetPolicy::RequirePricedReservation,
                    )
                    .reserve_call_settle(
                        |budget| {
                            super::spend::reserve_embedding_budget_with_request_pricing(
                                &reserve_request_pricing,
                                &reserve_pricing_config,
                                budget.budget_limits(),
                                budget.provider(),
                                budget.model(),
                                &core_request.input,
                            )
                        },
                        || {
                            callback.begin_provider_execution_with_pricing(
                                callback_provider,
                                callback_model,
                                callback_request_pricing,
                            );
                            provider.create_embeddings(request_for_provider, context)
                        },
                        |response, reservations, budget| {
                            let (budget_reservation, key_budget_reservation) =
                                reservations.into_parts();
                            async move {
                                let tokens = response
                                    .usage
                                    .as_ref()
                                    .map(|usage| u64::from(usage.total_tokens))
                                    .unwrap_or_default();
                                if let Some(usage) = response.usage.as_ref() {
                                    let usage = PricingUsage::from(usage);
                                    super::spend::record_pricing_usage_spend_with_request_pricing(
                                        &settle_request_pricing,
                                        &settle_pricing_config,
                                        budget.budget_limits(),
                                        &settle_key_manager,
                                        api_key_id,
                                        budget.provider(),
                                        budget.model(),
                                        &usage,
                                        budget_reservation,
                                        key_budget_reservation,
                                    )
                                    .await;
                                } else {
                                    super::spend::record_completion_spend_with_reservation_with_policy(
                                        settle_pricing_service.as_ref(),
                                        &settle_pricing_config,
                                        super::spend::usage_spend_settlement(
                                            (
                                                budget.budget_limits(),
                                                &settle_key_manager,
                                                api_key_id,
                                            ),
                                            (budget.provider(), budget.model(), None),
                                            budget_reservation,
                                            key_budget_reservation,
                                        ),
                                    )
                                    .await;
                                }
                                (response, tokens)
                            }
                        },
                    )
                    .await
                    .map(|(response, tokens)| {
                        (EmbeddingAttemptResponse::Provider(response), tokens)
                    })
            }
        },
    )
    .await
    {
        Ok(response) => response,
        Err(error) => {
            callback.fail(error.to_string(), "provider_error");
            return Err(error);
        }
    };

    let core_response = match core_response {
        EmbeddingAttemptResponse::Cached(cached) => return Ok(cached),
        EmbeddingAttemptResponse::Provider(response) => response,
    };

    // Convert core response to OpenAI format
    let callback_usage = core_response.usage.clone();
    let response = EmbeddingResponse {
        object: core_response.object,
        data: core_response
            .data
            .into_iter()
            .map(|d| crate::core::models::openai::EmbeddingObject {
                object: d.object,
                embedding: d.embedding.into_iter().map(|f| f as f64).collect(),
                index: d.index,
            })
            .collect(),
        model: core_response.model,
        usage: crate::core::models::openai::EmbeddingUsage {
            prompt_tokens: core_response
                .usage
                .as_ref()
                .map(|u| u.prompt_tokens)
                .unwrap_or(0),
            total_tokens: core_response
                .usage
                .as_ref()
                .map(|u| u.total_tokens)
                .unwrap_or(0),
        },
    };

    if let Err(error) =
        super::response_cache::store_embedding(state, &request_for_cache, &response, &context).await
    {
        callback.fail(error.to_string(), "cache_error");
        return Err(error);
    }
    callback.complete_usage(callback_usage.as_ref(), "success");
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_embedding_input_accepts_string() {
        let input = parse_embedding_input(&serde_json::json!("hello")).unwrap();

        match input {
            EmbeddingInput::Text(text) => assert_eq!(text, "hello"),
            EmbeddingInput::Array(_) => panic!("expected text embedding input"),
        }
    }

    #[test]
    fn parse_embedding_input_preserves_string_array() {
        let input = parse_embedding_input(&serde_json::json!(["a", "b"])).unwrap();

        match input {
            EmbeddingInput::Array(texts) => assert_eq!(texts, vec!["a", "b"]),
            EmbeddingInput::Text(_) => panic!("expected array embedding input"),
        }
    }

    #[test]
    fn parse_embedding_input_rejects_non_string_array_item() {
        let error = parse_embedding_input(&serde_json::json!(["a", 123])).unwrap_err();

        match error {
            GatewayError::Validation(message) => {
                assert!(message.contains("index 1"));
                assert!(message.contains("number"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn parse_embedding_input_rejects_object() {
        let error = parse_embedding_input(&serde_json::json!({ "text": "hello" })).unwrap_err();

        match error {
            GatewayError::Validation(message) => {
                assert_eq!(
                    message,
                    "Invalid input: expected string or array of strings"
                );
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }
}
