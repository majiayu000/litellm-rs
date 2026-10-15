//! A complete runtime-backed call to a local OpenAI-compatible server.
use litellm_rs::{
    completion,
    config::models::provider::ProviderConfig,
    core::{
        net::ProviderEndpointAccess,
        router::{RuntimeBinding, UnifiedRouter, install_default_runtime},
    },
    user_message,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let base_url = args
        .next()
        .unwrap_or_else(|| "http://localhost:8000/v1".into());
    let model = args.next().unwrap_or_else(|| "local-model".into());
    let provider = ProviderConfig {
        name: "local".into(),
        provider_type: "vllm".into(),
        base_url: Some(base_url),
        endpoint_access: ProviderEndpointAccess::PrivateNetwork,
        models: vec![model.clone()],
        ..ProviderConfig::default()
    };
    let router = UnifiedRouter::from_gateway_config(&[provider], None).await?;
    install_default_runtime(RuntimeBinding::new(Arc::new(router)))?;

    let response = completion(&model, vec![user_message("Hello!")], None).await?;
    let content = response
        .choices
        .first()
        .and_then(|choice| choice.message.content.as_ref());
    println!("{}", content.ok_or("response has no text content")?);
    Ok(())
}
