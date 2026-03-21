use crate::utils::error::gateway_error::Result;
use tracing::warn;

use super::types::{DatabaseStats, SeaOrmDatabase};

impl SeaOrmDatabase {
    /// Get user usage statistics
    pub async fn get_user_usage(
        &self,
        _user_id: &str,
        _start: chrono::DateTime<chrono::Utc>,
        _end: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<serde_json::Value>> {
        // NOTE: user usage retrieval not yet implemented
        warn!("get_user_usage not implemented yet");
        Ok(vec![])
    }

    /// Store request metrics
    pub async fn store_metrics(
        &self,
        _metrics: &crate::core::models::metrics::request::RequestMetrics,
    ) -> Result<()> {
        // NOTE: metrics storage not yet implemented
        warn!("store_metrics not implemented yet");
        Ok(())
    }

    /// Get database statistics
    pub fn stats(&self) -> DatabaseStats {
        // NOTE: database stats not yet implemented
        warn!("stats not implemented yet");
        DatabaseStats {
            total_users: 0,
            size: 0,
            idle: 0,
        }
    }
}
