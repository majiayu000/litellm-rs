//! Opt-in live AWS Bedrock smoke tests.
//!
//! These tests are ignored by default and only call AWS when
//! `LITELLM_RS_LIVE_BEDROCK=1` is present.
//!
//! Run with:
//! `LITELLM_RS_LIVE_BEDROCK=1 cargo test --test live_bedrock -- --ignored`.

use std::env;
use std::error::Error;
use std::time::Duration;

use futures::StreamExt;
use litellm_rs::core::providers::bedrock::{BedrockConfig, BedrockProvider};
use litellm_rs::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use litellm_rs::core::types::chat::ChatRequest;
use litellm_rs::core::types::context::RequestContext;

fn live_enabled() -> bool {
    env::var("LITELLM_RS_LIVE_BEDROCK")
        .map(|value| value == "1")
        .unwrap_or(false)
}

fn required_bedrock_live_env(name: &str) -> Result<String, Box<dyn Error>> {
    let value = env::var(name)?;
    if value.trim().is_empty() {
        return Err(format!("{name} must not be empty").into());
    }
    Ok(value)
}

fn optional_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

async fn provider_from_env() -> Result<BedrockProvider, Box<dyn Error>> {
    let config = BedrockConfig {
        aws_access_key_id: required_bedrock_live_env("AWS_ACCESS_KEY_ID")?,
        aws_secret_access_key: required_bedrock_live_env("AWS_SECRET_ACCESS_KEY")?,
        aws_session_token: optional_env("AWS_SESSION_TOKEN"),
        aws_region: required_bedrock_live_env("AWS_REGION")?,
        timeout_seconds: 60,
        max_retries: 2,
    };

    Ok(BedrockProvider::new(config).await?)
}

fn smoke_request(model: String) -> ChatRequest {
    let mut request = ChatRequest::new(model)
        .add_system_message("Answer with one short lowercase word.")
        .add_user_message("Reply with: pong");
    request.max_tokens = Some(16);
    request.temperature = Some(0.0);
    request
}

#[tokio::test]
#[ignore = "requires live AWS Bedrock credentials and LITELLM_RS_LIVE_BEDROCK=1"]
async fn bedrock_converse_chat_smoke() -> Result<(), Box<dyn Error>> {
    if !live_enabled() {
        eprintln!("skipping live Bedrock smoke: LITELLM_RS_LIVE_BEDROCK is not 1");
        return Ok(());
    }

    let model = required_bedrock_live_env("BEDROCK_CONVERSE_MODEL_ID")?;
    let provider = provider_from_env().await?;
    let response = provider
        .chat_completion(smoke_request(model), RequestContext::new())
        .await?;

    let content = response
        .first_content()
        .ok_or("Bedrock response did not include assistant text")?;
    assert!(
        content.to_lowercase().contains("pong"),
        "expected Bedrock response to contain pong, got {content:?}"
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires live AWS Bedrock credentials and LITELLM_RS_LIVE_BEDROCK=1"]
async fn bedrock_converse_stream_smoke() -> Result<(), Box<dyn Error>> {
    if !live_enabled() {
        eprintln!("skipping live Bedrock stream smoke: LITELLM_RS_LIVE_BEDROCK is not 1");
        return Ok(());
    }

    let model = required_bedrock_live_env("BEDROCK_CONVERSE_STREAM_MODEL_ID")?;
    let provider = provider_from_env().await?;
    let mut request = smoke_request(model);
    request.stream = true;

    let mut stream = provider
        .chat_completion_stream(request, RequestContext::new())
        .await?;
    let mut text = String::new();

    tokio::time::timeout(Duration::from_secs(90), async {
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            for choice in chunk.choices {
                if let Some(content) = choice.delta.content {
                    text.push_str(&content);
                }
                if choice.finish_reason.is_some() {
                    return Ok::<(), Box<dyn Error>>(());
                }
            }
        }
        Ok(())
    })
    .await??;

    assert!(
        text.to_lowercase().contains("pong"),
        "expected Bedrock stream to contain pong, got {text:?}"
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires a live Bedrock inference profile model ID"]
async fn bedrock_inference_profile_chat_smoke() -> Result<(), Box<dyn Error>> {
    if !live_enabled() {
        eprintln!("skipping live Bedrock profile smoke: LITELLM_RS_LIVE_BEDROCK is not 1");
        return Ok(());
    }

    let Some(model) = optional_env("BEDROCK_INFERENCE_PROFILE_MODEL_ID") else {
        eprintln!(
            "skipping live Bedrock profile smoke: BEDROCK_INFERENCE_PROFILE_MODEL_ID is unset"
        );
        return Ok(());
    };

    let provider = provider_from_env().await?;
    let response = provider
        .chat_completion(smoke_request(model), RequestContext::new())
        .await?;

    let content = response
        .first_content()
        .ok_or("Bedrock profile response did not include assistant text")?;
    assert!(
        content.to_lowercase().contains("pong"),
        "expected Bedrock profile response to contain pong, got {content:?}"
    );
    Ok(())
}
