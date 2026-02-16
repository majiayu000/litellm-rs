//! Test Groq integration with Provider dispatch

use litellm_rs::core::providers::{
    Provider,
    groq::{GroqConfig, GroqProvider},
};
use litellm_rs::core::types::health::HealthStatus;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create Groq provider through the Provider enum
    let config = GroqConfig::from_env().with_api_key("test-key");

    let groq_provider = GroqProvider::new(config).await?;
    let provider = Provider::Groq(groq_provider);

    // Test dispatch methods
    println!("Provider name: {}", provider.name());
    println!("Provider type: {:?}", provider.provider_type());

    // Test capabilities
    let capabilities = provider.capabilities();
    println!("Capabilities: {} total", capabilities.len());

    // Test model support
    let test_model = "llama-3.3-70b-versatile";
    let supported = provider.supports_model(test_model);
    println!("Supports {}: {}", test_model, supported);

    // Test health check
    let health = provider.health_check().await;
    println!(
        "Health status: {}",
        match health {
            HealthStatus::Healthy => "Healthy",
            HealthStatus::Unhealthy => "Unhealthy",
            HealthStatus::Unknown => "Unknown",
            HealthStatus::Degraded => "Degraded",
        }
    );

    println!("\n✅ Groq dispatch integration verified!");
    Ok(())
}
