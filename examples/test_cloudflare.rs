//! Example of using Cloudflare Workers AI Provider

use litellm_rs::core::providers::cloudflare::{CloudflareConfig, CloudflareProvider};
use litellm_rs::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use litellm_rs::core::types::{
    chat::ChatMessage, chat::ChatRequest, context::RequestContext, message::MessageContent,
    message::MessageRole,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create Cloudflare configuration
    let config = CloudflareConfig {
        account_id: std::env::var("CLOUDFLARE_ACCOUNT_ID").ok(),
        api_token: std::env::var("CLOUDFLARE_API_TOKEN").ok(),
        ..Default::default()
    };

    // Create provider
    let provider = match CloudflareProvider::new(config).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to create Cloudflare provider: {}", e);
            eprintln!(
                "Make sure CLOUDFLARE_ACCOUNT_ID and CLOUDFLARE_API_TOKEN environment variables are set"
            );
            return Err(e.into());
        }
    };

    println!("Cloudflare Workers AI Provider initialized successfully!");
    println!("Available models:");
    for model in provider.models() {
        println!(
            "  - {} ({}): {} tokens context, streaming: {}",
            model.id, model.name, model.max_context_length, model.supports_streaming
        );
    }

    // Test chat completion with GPT OSS
    println!("\n=== Testing Chat Completion with GPT OSS 120B ===");

    let request = ChatRequest {
        model: "@cf/openai/gpt-oss-120b".to_string(),
        messages: vec![
            ChatMessage {
                role: MessageRole::System,
                content: Some(MessageContent::Text(
                    "You are a helpful assistant running on Cloudflare's global network."
                        .to_string(),
                )),
                ..Default::default()
            },
            ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text(
                    "What are the benefits of edge computing? Keep it brief.".to_string(),
                )),
                ..Default::default()
            },
        ],
        temperature: Some(0.7),
        max_tokens: Some(150),
        stream: false,
        ..Default::default()
    };

    let context = RequestContext::default();

    match provider.chat_completion(request.clone(), context).await {
        Ok(response) => {
            println!("\nResponse:");
            if let Some(choice) = response.choices.first()
                && let Some(ref content) = choice.message.content
            {
                println!("{}", content);
            }
        }
        Err(e) => {
            println!("Chat completion failed: {}", e);
        }
    }

    // Test with Gemma
    println!("\n=== Testing with Gemma 4 ===");

    let gemma_request = ChatRequest {
        model: "@cf/google/gemma-4-26b-a4b-it".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text(
                "Write a haiku about cloud computing.".to_string(),
            )),
            ..Default::default()
        }],
        temperature: Some(0.8),
        max_tokens: Some(100),
        stream: false,
        ..Default::default()
    };

    match provider
        .chat_completion(gemma_request, RequestContext::default())
        .await
    {
        Ok(response) => {
            println!("\nGemma 4 Response:");
            if let Some(choice) = response.choices.first()
                && let Some(ref content) = choice.message.content
            {
                println!("{}", content);
            }
        }
        Err(e) => {
            println!("Gemma 4 request failed: {}", e);
        }
    }

    // Test with Kimi K2.7 Code for code generation
    println!("\n=== Testing Code Generation with Kimi K2.7 Code ===");

    let code_request = ChatRequest {
        model: "@cf/moonshotai/kimi-k2.7-code".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text(
                "Write a Python function that calculates the factorial of a number.".to_string(),
            )),
            ..Default::default()
        }],
        temperature: Some(0.3),
        max_tokens: Some(200),
        stream: false,
        ..Default::default()
    };

    match provider
        .chat_completion(code_request, RequestContext::default())
        .await
    {
        Ok(response) => {
            println!("\nKimi K2.7 Code Response:");
            if let Some(choice) = response.choices.first()
                && let Some(ref content) = choice.message.content
            {
                println!("{}", content);
            }
        }
        Err(e) => {
            println!("Code generation failed: {}", e);
        }
    }

    // Published-rate estimate before account allowances or credits
    println!("\n=== Testing Cost Calculation ===");

    match provider
        .calculate_cost("@cf/openai/gpt-oss-120b", 1000, 500)
        .await
    {
        Ok(cost) => println!(
            "Estimated cost for 1000 input + 500 output tokens: ${:.6} before account allowances",
            cost
        ),
        Err(e) => println!("Cost calculation failed: {}", e),
    };

    Ok(())
}
