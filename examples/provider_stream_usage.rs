//! Single-attempt streaming through the provider layer, with a final usage chunk.
//!
//! `create_provider` + `Provider::chat_completion_stream` skips the router: no
//! deployment selection, router retries, fallbacks or cooldowns. The provider
//! layer sends exactly the request you build, so ask for usage explicitly with
//! `stream_options.include_usage`; some OpenAI-compatible endpoints (for example
//! Azure OpenAI v1) send no usage chunk without it.
//!
//! ```bash
//! OPENAI_COMPAT_API_KEY=... cargo run --example provider_stream_usage -- \
//!     https://<resource>.openai.azure.com/openai/v1 <deployment-or-model>
//! # Local server (loopback / RFC1918) needs the private-network policy:
//! ENDPOINT_ACCESS=private_network cargo run --example provider_stream_usage -- \
//!     http://localhost:8000/v1 your-served-model
//! ```
use futures::StreamExt;
use litellm_rs::{
    config::models::provider::ProviderConfig,
    core::{
        net::ProviderEndpointAccess,
        providers::create_provider,
        types::{
            chat::{ChatRequest, StreamOptions},
            context::RequestContext,
            model::ProviderCapability,
        },
    },
};
use std::io::Write;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(base_url), Some(model)) = (args.next(), args.next()) else {
        return Err("usage: provider_stream_usage <base_url> <model>".into());
    };
    let endpoint_access = match std::env::var("ENDPOINT_ACCESS") {
        Ok(value) => value.parse::<ProviderEndpointAccess>()?,
        Err(_) => ProviderEndpointAccess::PublicOnly,
    };

    let provider = create_provider(ProviderConfig {
        name: "compat".into(),
        provider_type: "openai_compatible".into(),
        api_key: std::env::var("OPENAI_COMPAT_API_KEY").unwrap_or_default(),
        base_url: Some(base_url),
        endpoint_access,
        models: vec![model.clone()],
        ..ProviderConfig::default()
    })
    .await?;
    if !provider
        .capabilities()
        .contains(&ProviderCapability::ChatCompletionStream)
    {
        return Err("provider does not support chat streaming".into());
    }

    let mut request = ChatRequest::new(model)
        .add_user_message("Say hello in five words.")
        .with_streaming();
    // The router path (`completion_stream`) sets this for you; the provider path does not.
    request.stream_options = Some(StreamOptions {
        include_usage: Some(true),
    });

    let mut stream = provider
        .chat_completion_stream(request, RequestContext::default())
        .await?;
    let mut usage = None;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        for choice in &chunk.choices {
            if let Some(text) = &choice.delta.content {
                print!("{text}");
                std::io::stdout().flush()?;
            }
        }
        // With include_usage, OpenAI-compatible servers send usage on a final
        // chunk whose `choices` is empty.
        if chunk.usage.is_some() {
            usage = chunk.usage;
        }
    }
    println!();
    let usage = usage.ok_or("endpoint sent no usage chunk")?;
    println!(
        "usage: prompt={} completion={} total={}",
        usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
    );
    Ok(())
}
