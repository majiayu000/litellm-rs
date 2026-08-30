//! Test thinking/reasoning extraction from various providers
//!
//! Run with: cargo run --example test_thinking --all-features

use litellm_rs::core::providers::openai_like::OpenAILikeProvider;
use litellm_rs::core::providers::thinking::{
    deepseek_thinking, openai_thinking, openrouter_thinking,
};
use litellm_rs::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use litellm_rs::core::types::context::RequestContext;
use litellm_rs::core::types::thinking::{ThinkingConfig, ThinkingContent, ThinkingEffort};
use litellm_rs::core::types::{
    chat::ChatMessage, chat::ChatRequest, message::MessageContent, message::MessageRole,
};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for debug output
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_target(false)
        .init();

    println!("=== Thinking/Reasoning Extraction Test ===\n");

    // Get API key from environment
    let api_key = env::var("OPENROUTER_API_KEY")
        .expect("OPENROUTER_API_KEY environment variable must be set");

    // Test model detection
    test_model_detection();

    // Create OpenRouter provider via catalog
    let def = litellm_rs::core::providers::registry::get_definition("openrouter")
        .expect("openrouter should be in catalog");
    let config = def.to_openai_like_config(Some(&api_key), None);
    let provider = OpenAILikeProvider::new(config).await?;

    // Test with DeepSeek R1 (thinking model)
    println!("\n--- Testing DeepSeek R1 (Reasoning Model) ---");
    test_deepseek_r1(&provider).await?;

    println!("\n=== All Tests Complete ===");
    Ok(())
}

fn test_model_detection() {
    println!("--- Model Detection Tests ---");

    // OpenAI models
    println!(
        "OpenAI o1-preview: {}",
        openai_thinking::supports_thinking("o1-preview")
    );
    println!(
        "OpenAI o3-mini: {}",
        openai_thinking::supports_thinking("o3-mini")
    );
    println!(
        "OpenAI gpt-4: {}",
        openai_thinking::supports_thinking("gpt-4")
    );

    // DeepSeek models
    println!(
        "DeepSeek R1: {}",
        deepseek_thinking::supports_thinking("deepseek-r1")
    );
    println!(
        "DeepSeek Chat: {}",
        deepseek_thinking::supports_thinking("deepseek-chat")
    );

    // OpenRouter detection
    println!(
        "OpenRouter deepseek/deepseek-r1: {}",
        openrouter_thinking::supports_thinking("deepseek/deepseek-r1")
    );
    println!(
        "OpenRouter provider detection: {}",
        openrouter_thinking::detect_provider("deepseek/deepseek-r1")
    );
}

async fn test_deepseek_r1(provider: &OpenAILikeProvider) -> Result<(), Box<dyn std::error::Error>> {
    // Create a reasoning question
    let request = ChatRequest {
        model: "deepseek/deepseek-r1".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text(
                "What is 15% of 240? Think through this step by step.".to_string(),
            )),
            ..Default::default()
        }],
        thinking: Some(ThinkingConfig {
            enabled: true,
            budget_tokens: Some(5000),
            effort: Some(ThinkingEffort::Medium),
            include_thinking: true,
            ..Default::default()
        }),
        max_tokens: Some(2000),
        ..Default::default()
    };

    // Also print the request being sent
    println!("Request: {:?}", request);

    println!("\nSending request to DeepSeek R1...");

    let context = RequestContext::default();
    let response = provider.chat_completion(request.clone(), context).await?;

    println!("\n--- Response ---");
    println!("Model: {}", response.model);
    println!("ID: {}", response.id);

    // Check for thinking content in response
    if let Some(choice) = response.choices.first() {
        println!("\n--- Message Content ---");
        if let Some(content) = &choice.message.content {
            // Truncate content for display
            let content_str = content.to_string();
            let display_content = if content_str.len() > 500 {
                format!(
                    "{}... [truncated, {} chars total]",
                    &content_str[..500],
                    content_str.len()
                )
            } else {
                content_str
            };
            println!("Content: {}", display_content);
        }

        // Check for thinking in the message - more detailed debug
        println!("\n--- Thinking Field Debug ---");
        println!(
            "message.thinking is_some: {}",
            choice.message.thinking.is_some()
        );
        println!("message.thinking: {:?}", choice.message.thinking);

        if let Some(thinking) = &choice.message.thinking {
            println!("\n--- Thinking Content Found! ---");
            match thinking {
                ThinkingContent::Text { text, signature } => {
                    println!("Type: Text");
                    let display_text = if text.len() > 500 {
                        format!(
                            "{}... [truncated, {} chars total]",
                            &text[..500],
                            text.len()
                        )
                    } else {
                        text.clone()
                    };
                    println!("Thinking:\n{}", display_text);
                    if let Some(sig) = signature {
                        println!("Signature: {}", sig);
                    }
                }
                ThinkingContent::Block {
                    thinking,
                    block_type,
                } => {
                    println!("Type: Block");
                    println!("Block Type: {:?}", block_type);
                    println!("Thinking:\n{}", thinking);
                }
                ThinkingContent::Redacted { token_count } => {
                    println!("Type: Redacted");
                    println!("Token Count: {:?}", token_count);
                }
            }
        } else {
            println!("\n[Thinking field is None]");
        }

        println!("\nFinish Reason: {:?}", choice.finish_reason);
    }

    // Check usage
    if let Some(usage) = &response.usage {
        println!("\n--- Usage ---");
        println!("Prompt tokens: {}", usage.prompt_tokens);
        println!("Completion tokens: {}", usage.completion_tokens);
        println!("Total tokens: {}", usage.total_tokens);

        if let Some(thinking_usage) = &usage.thinking_usage {
            println!("\n--- Thinking Usage ---");
            println!("Thinking tokens: {:?}", thinking_usage.thinking_tokens);
            println!("Budget tokens: {:?}", thinking_usage.budget_tokens);
            println!("Thinking cost: {:?}", thinking_usage.thinking_cost);
            println!("Provider: {:?}", thinking_usage.provider);
        } else {
            println!("\n[No thinking_usage in response]");
        }
    }

    // Also try to extract thinking from raw response
    println!("\n--- Raw Response Analysis ---");
    let raw_json = serde_json::to_value(&response)?;
    println!(
        "Response JSON keys: {:?}",
        raw_json.as_object().map(|o| o.keys().collect::<Vec<_>>())
    );

    // Check choices structure
    if let Some(choices) = raw_json.get("choices").and_then(|c| c.as_array()) {
        for (i, choice) in choices.iter().enumerate() {
            println!(
                "\nChoice {} message keys: {:?}",
                i,
                choice
                    .get("message")
                    .and_then(|m| m.as_object())
                    .map(|o| o.keys().collect::<Vec<_>>())
            );
        }
    }

    // Try OpenRouter thinking extraction
    if let Some(extracted) = openrouter_thinking::extract_thinking(&raw_json) {
        println!(
            "Extracted thinking via openrouter_thinking::extract_thinking: {:?}",
            extracted
        );
    } else {
        println!("No thinking extracted from serialized response");
    }

    Ok(())
}
