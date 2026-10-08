//! Request estimates owned by admission, separate from observed token usage.

use crate::core::models::openai::ChatCompletionRequest;
use crate::core::pricing_service::PricingUsage;
use crate::core::providers::ChatMessageContinuation;
use crate::utils::ai::counter::token_counter::{TokenCounter, TokenizerIdentity};
use crate::utils::error::gateway_error::GatewayError;

#[path = "execution_estimate_media.rs"]
mod media;

pub(in crate::server::routes::ai) fn chat(
    request: &ChatCompletionRequest,
    retained_prompt_tokens: u32,
) -> Result<u64, GatewayError> {
    chat_with_continuation(request, &[], retained_prompt_tokens)
}

pub(in crate::server::routes::ai) fn chat_with_continuation(
    request: &ChatCompletionRequest,
    continuations: &[ChatMessageContinuation],
    retained_prompt_tokens: u32,
) -> Result<u64, GatewayError> {
    chat_with_key_limit(request, continuations, retained_prompt_tokens, None)
}

pub(in crate::server::routes::ai) fn chat_with_key_limit(
    request: &ChatCompletionRequest,
    continuations: &[ChatMessageContinuation],
    retained_prompt_tokens: u32,
    key_output_limit: Option<u32>,
) -> Result<u64, GatewayError> {
    let identity = TokenizerIdentity::approximate("gateway", &request.model);
    // Serialize the forwarding carrier, including opaque signatures and redacted
    // payloads. Its Debug representation and guardrail projection omit that data.
    let mut continuation_tokens = 0_u32;
    for continuation in continuations.iter().filter(|value| !value.is_empty()) {
        continuation_tokens = continuation_tokens
            .saturating_add(u32::try_from(json_input(continuation)?).unwrap_or(u32::MAX));
    }
    let input = super::super::spend::try_estimate_chat_prompt_tokens(
        &identity,
        &request.messages,
        request.tools.as_deref(),
        request.functions.as_deref(),
        request.function_call.as_ref(),
        request.response_format.as_ref(),
    )?
    .saturating_add(retained_prompt_tokens)
    .saturating_add(continuation_tokens);
    // An alias may not have a local context-window entry. Its explicit bound
    // must not be truncated to the fallback tokenizer's smaller model window.
    let output = match super::super::token_policy::requested_chat_output_token_limit(request)
        .or(key_output_limit)
    {
        Some(bound) => bound,
        None => TokenCounter::new().estimate_output_tokens(None, input, &identity)?,
    };
    Ok(u64::from(input)
        .saturating_add(u64::from(output).saturating_mul(u64::from(request.n.unwrap_or(1).max(1)))))
}

pub(in crate::server::routes::ai) fn json_input<T: serde::Serialize>(
    request: &T,
) -> Result<u64, GatewayError> {
    let text = serde_json::to_string(request)
        .map_err(|_| GatewayError::validation("Request cannot be token-estimated"))?;
    let identity = TokenizerIdentity::approximate("gateway", "default");
    let input = TokenCounter::new().count_completion_tokens(&identity, &text)?;
    Ok(u64::from(input.input_tokens))
}

pub(in crate::server::routes::ai) fn json_completion<T: serde::Serialize>(
    request: &T,
    model: &str,
    max_output_tokens: Option<u32>,
) -> Result<u64, GatewayError> {
    json_completion_with_candidates(request, model, max_output_tokens, 1)
}

pub(in crate::server::routes::ai) fn json_completion_with_candidates<T: serde::Serialize>(
    request: &T,
    model: &str,
    max_output_tokens: Option<u32>,
    candidates: u64,
) -> Result<u64, GatewayError> {
    let input = u32::try_from(media::native_input(request, model)?).unwrap_or(u32::MAX);
    let output = match max_output_tokens {
        Some(bound) => bound,
        None => TokenCounter::new().estimate_output_tokens(
            None,
            input,
            &TokenizerIdentity::approximate("gateway", model),
        )?,
    };
    Ok(u64::from(input).saturating_add(u64::from(output).saturating_mul(candidates.max(1))))
}

pub(in crate::server::routes::ai) fn usage(usage: &PricingUsage) -> u64 {
    u64::from(usage.prompt_tokens)
        .saturating_add(u64::from(usage.completion_tokens))
        .saturating_add(u64::from(usage.audio_tokens.unwrap_or_default()))
        .saturating_add(u64::from(usage.image_tokens.unwrap_or_default()))
}

#[cfg(test)]
#[path = "execution_estimate_tests.rs"]
mod tests;
