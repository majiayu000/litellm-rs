//! Azure AI Streaming Completion Example
//!
//! Demonstrates streaming responses from Azure AI models
//! Run with: AZURE_AI_API_KEY=xxx AZURE_AI_API_BASE=xxx cargo run --example azure_ai_streaming

use futures::StreamExt;
use litellm_rs::{CompletionOptions, completion_stream};
use litellm_rs::{system_message, user_message};
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔷 Azure AI Streaming Example\n");
    println!("Watch the response appear token by token!\n");

    // Check environment
    if std::env::var("AZURE_AI_API_KEY").is_err() || std::env::var("AZURE_AI_API_BASE").is_err() {
        println!("⚠️  Please set environment variables:");
        println!("   export AZURE_AI_API_KEY='your-api-key'");
        println!("   export AZURE_AI_API_BASE='https://your-resource.cognitiveservices.azure.com'");
        return Ok(());
    }

    // Example 1: Simple streaming
    println!("📤 Example 1: Simple Streaming Response\n");

    let messages = vec![
        system_message("You are a helpful assistant. Respond concisely."),
        user_message("Count from 1 to 10 with a brief description for each number."),
    ];

    print!("💬 Assistant: ");
    io::stdout().flush()?;

    match completion_stream("azure_ai/gpt-4o", messages, None).await {
        Ok(mut stream) => {
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        if let Some(choice) = chunk.choices.first()
                            && let Some(ref content) = choice.delta.content
                        {
                            print!("{}", content);
                            io::stdout().flush()?;
                        }
                    }
                    Err(e) => {
                        println!("\n❌ Stream error: {}", e);
                        break;
                    }
                }
            }
            println!("\n\n✅ Streaming completed!\n");
        }
        Err(e) => println!("❌ Failed to start stream: {}\n", e),
    }

    // Example 2: Streaming with parameters
    println!("📤 Example 2: Creative Writing with Streaming\n");

    let params = CompletionOptions {
        temperature: Some(0.9),
        max_tokens: Some(200),
        top_p: Some(0.95),
        stream: true,
        ..Default::default()
    };

    let story_messages = vec![
        system_message("You are a creative writer. Write vividly and engagingly."),
        user_message(
            "Write the opening paragraph of a sci-fi story set on Azure Cloud Platform as if it were a planet.",
        ),
    ];

    print!("📖 Story: ");
    io::stdout().flush()?;

    match completion_stream("azure_ai/gpt-4o", story_messages, Some(params)).await {
        Ok(mut stream) => {
            let mut full_response = String::new();

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        if let Some(choice) = chunk.choices.first() {
                            if let Some(ref content) = choice.delta.content {
                                print!("{}", content);
                                io::stdout().flush()?;
                                full_response.push_str(content);
                            }

                            // Check for finish reason
                            if let Some(ref reason) = choice.finish_reason {
                                println!("\n\n📊 Finish reason: {:?}", reason);
                            }
                        }
                    }
                    Err(e) => {
                        println!("\n❌ Stream error: {}", e);
                        break;
                    }
                }
            }

            println!("\n📏 Total characters generated: {}\n", full_response.len());
        }
        Err(e) => println!("❌ Failed to start stream: {}\n", e),
    }

    // Example 3: Interactive streaming
    println!("📤 Example 3: Code Generation with Streaming\n");

    let code_messages = vec![
        system_message(
            "You are an expert Rust programmer. Generate clean, idiomatic code with comments.",
        ),
        user_message(
            "Write a Rust function that connects to Azure Blob Storage and lists all containers.",
        ),
    ];

    println!("```rust");

    match completion_stream("azure_ai/gpt-4o", code_messages, None).await {
        Ok(mut stream) => {
            let mut token_count = 0;

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        if let Some(choice) = chunk.choices.first()
                            && let Some(ref content) = choice.delta.content
                        {
                            print!("{}", content);
                            io::stdout().flush()?;
                            token_count += 1;
                        }

                        // Show progress occasionally
                        if token_count % 50 == 0 && token_count > 0 {
                            // Just counting chunks as a simple metric
                        }
                    }
                    Err(e) => {
                        println!("\n❌ Stream error: {}", e);
                        break;
                    }
                }
            }

            println!("\n```\n");
            println!("✅ Generated approximately {} tokens\n", token_count);
        }
        Err(e) => println!("❌ Failed to start stream: {}\n", e),
    }

    // Example 4: Streaming with different models
    println!("📤 Example 4: Fast Response with GPT-3.5-Turbo\n");

    let fast_messages = vec![user_message("List 5 Azure services in a bullet list.")];

    print!("⚡ Fast Response: ");
    io::stdout().flush()?;

    match completion_stream("azure_ai/gpt-35-turbo", fast_messages, None).await {
        Ok(mut stream) => {
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        if let Some(choice) = chunk.choices.first()
                            && let Some(ref content) = choice.delta.content
                        {
                            print!("{}", content);
                            io::stdout().flush()?;
                        }
                    }
                    Err(e) => {
                        println!("\n❌ Stream error: {}", e);
                        break;
                    }
                }
            }
            println!("\n");
        }
        Err(e) => println!("❌ Note: {} (Model might not be deployed)\n", e),
    }

    println!("💡 Streaming Tips:");
    println!("   • Streaming reduces time to first token");
    println!("   • Ideal for chat interfaces and long responses");
    println!("   • Process chunks as they arrive for better UX");
    println!("   • Handle connection interruptions gracefully");
    println!("   • Consider buffering for word-level display\n");

    Ok(())
}
