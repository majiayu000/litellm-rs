//! Tests for the event publish-subscribe system

use super::broker::{EventBroker, EventBrokerConfig};
use super::types::{Event, EventType, Subscriber, SubscriptionHandle};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::sync::Barrier;
use tokio::time::timeout;

#[derive(Debug, Clone, PartialEq)]
struct TestData {
    id: u64,
    value: String,
}

impl TestData {
    fn new(id: u64, value: &str) -> Self {
        Self {
            id,
            value: value.to_string(),
        }
    }
}

mod broker_concurrency_tests;
mod broker_config_tests;
mod broker_creation_tests;
mod broker_edge_case_tests;
mod broker_publish_tests;
mod broker_subscription_tests;
mod event_tests;
mod event_type_tests;
mod subscriber_trait_tests;
mod subscription_handle_tests;
