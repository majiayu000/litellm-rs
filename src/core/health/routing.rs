//! Health-based routing methods
//!
//! This module provides methods for selecting and routing to healthy providers
//! based on their health status and routing weights.

use super::monitor::HealthMonitor;

impl HealthMonitor {
    /// Get healthy providers sorted by routing weight
    pub async fn get_healthy_providers(&self) -> Vec<(String, f64)> {
        let health_map = self.provider_health.read().await;
        let mut providers: Vec<_> = health_map
            .iter()
            .filter(|(_, health)| health.is_available())
            .map(|(id, health)| (id.clone(), health.routing_weight()))
            .collect();

        providers.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        providers
    }
}
