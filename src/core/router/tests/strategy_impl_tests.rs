//! Tests for routing strategy implementations (extracted from strategy_impl.rs)

use crate::core::providers::Provider;
use crate::core::providers::openai::OpenAIProvider;
use crate::core::router::deployment::{Deployment, DeploymentConfig};
use crate::core::router::strategy_impl::*;
use dashmap::DashMap;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::Relaxed;

// Helper to create a test provider
async fn create_test_provider() -> Provider {
    let openai = OpenAIProvider::with_api_key("sk-test-key-for-unit-testing-only")
        .await
        .expect("Failed to create OpenAI provider");
    Provider::OpenAI(openai)
}

// Helper to create a test deployment
async fn create_test_deployment(id: &str, config: DeploymentConfig) -> Deployment {
    Deployment::new(
        id.to_string(),
        create_test_provider().await,
        "gpt-4".to_string(),
        "gpt-4".to_string(),
    )
    .with_config(config)
}

mod context_tests;
mod integration_tests;
mod least_busy_tests;
mod lowest_latency_tests;
mod lowest_priority_tests;
mod lowest_usage_tests;
mod rate_limit_aware_tests;
mod round_robin_tests;
mod weighted_random_tests;
