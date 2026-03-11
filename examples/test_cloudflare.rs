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

    // Test chat completion with Llama 3
    println!("\n=== Testing Chat Completion with Llama 3 8B ===");

    let request = ChatRequest {
        model: "@cf/meta/llama-3-8b-instruct".to_string(),
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

    // Test with Mistral
    println!("\n=== Testing with Mistral 7B ===");

    let mistral_request = ChatRequest {
        model: "@cf/mistral/mistral-7b-instruct-v0.1".to_string(),
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
        .chat_completion(mistral_request, RequestContext::default())
        .await
    {
        Ok(response) => {
            println!("\nMistral Response:");
            if let Some(choice) = response.choices.first()
                && let Some(ref content) = choice.message.content
            {
                println!("{}", content);
            }
        }
        Err(e) => {
            println!("Mistral request failed: {}", e);
        }
    }

    // Test with Code Llama for code generation
    println!("\n=== Testing Code Generation with Code Llama ===");

    let code_request = ChatRequest {
        model: "@cf/meta/codellama-7b-instruct".to_string(),
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
            println!("\nCode Llama Response:");
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

    // Test cost calculation (should be 0 for Cloudflare)
    println!("\n=== Testing Cost Calculation ===");

    match provider
        .calculate_cost("@cf/meta/llama-3-8b-instruct", 1000, 500)
        .await
    {
        Ok(cost) => println!(
            "Cost for 1000 input + 500 output tokens: ${:.4} (Free on Cloudflare Workers!)",
            cost
        ),
        Err(e) => println!("Cost calculation failed: {}", e),
    };

    Ok(())
}
