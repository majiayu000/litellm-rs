//! Webhook integration system
//!
//! This module provides webhook functionality for external system integration.

mod delivery;
pub mod events;
mod manager;
#[cfg(test)]
mod tests;
mod types;

// Re-export public types and structs for backward compatibility
#[deprecated(
    since = "0.6.0",
    note = "core::webhooks is a default-off compatibility surface scheduled for removal in 0.7.0"
)]
pub use manager::WebhookManager;
#[deprecated(
    since = "0.6.0",
    note = "core::webhooks is a default-off compatibility surface scheduled for removal in 0.7.0"
)]
pub use types::{
    WebhookConfig, WebhookDelivery, WebhookDeliveryStatus, WebhookEventType, WebhookPayload,
    WebhookStats,
};
