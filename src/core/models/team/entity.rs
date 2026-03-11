//! Team core model

use super::billing::TeamBilling;
use super::settings::TeamSettings;
use crate::core::models::user::types::UserRateLimits;
use crate::core::models::{Metadata, UsageStats};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Team/Organization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    /// Team metadata
    #[serde(flatten)]
    pub metadata: Metadata,
    /// Team name (unique)
    pub name: String,
    /// Team display name
    pub display_name: Option<String>,
    /// Team description
    pub description: Option<String>,
    /// Team status
    pub status: TeamStatus,
    /// Team settings
    pub settings: TeamSettings,
    /// Usage statistics
    pub usage_stats: UsageStats,
    /// Team rate limits
    pub rate_limits: Option<UserRateLimits>,
    /// Billing information
    pub billing: Option<TeamBilling>,
    /// Team metadata
    pub team_metadata: HashMap<String, serde_json::Value>,
}

/// Team status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeamStatus {
    /// Active team
    Active,
    /// Inactive team
    Inactive,
    /// Suspended team
    Suspended,
    /// Deleted team (soft delete)
    Deleted,
}

/// Team visibility
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeamVisibility {
    /// Public team
    Public,
    /// Private team
    #[default]
    Private,
    /// Internal team
    Internal,
}

impl Team {
    /// Create a new team
    pub fn new(name: String, display_name: Option<String>) -> Self {
        Self {
            metadata: Metadata::new(),
            name,
            display_name,
            description: None,
            status: TeamStatus::Active,
            settings: TeamSettings::default(),
            usage_stats: UsageStats::default(),
            rate_limits: None,
            billing: None,
            team_metadata: HashMap::new(),
        }
    }

    /// Get team ID
    pub fn id(&self) -> Uuid {
        self.metadata.id
    }

    /// Check if team is active
    pub fn is_active(&self) -> bool {
        matches!(self.status, TeamStatus::Active)
    }

    /// Update usage statistics
    pub fn update_usage(&mut self, requests: u64, tokens: u64, cost: f64) {
        self.usage_stats.total_requests += requests;
        self.usage_stats.total_tokens += tokens;
        self.usage_stats.total_cost += cost;

        // Update daily stats
        let today = chrono::Utc::now().date_naive();
        let last_reset = self.usage_stats.last_reset.date_naive();

        if today != last_reset {
            self.usage_stats.requests_today = 0;
            self.usage_stats.tokens_today = 0;
            self.usage_stats.cost_today = 0.0;
            self.usage_stats.last_reset = chrono::Utc::now();
        }

        self.usage_stats.requests_today += requests as u32;
        self.usage_stats.tokens_today += tokens as u32;
        self.usage_stats.cost_today += cost;

        // Update billing usage if applicable
        if let Some(billing) = &mut self.billing {
            billing.current_usage += cost;
        }

        self.metadata.touch();
    }

    /// Check if team is over budget
    pub fn is_over_budget(&self) -> bool {
        if let Some(billing) = &self.billing
            && let Some(budget) = billing.monthly_budget
        {
            return billing.current_usage >= budget;
        }
        false
    }

    /// Get remaining budget
    pub fn remaining_budget(&self) -> Option<f64> {
        if let Some(billing) = &self.billing
            && let Some(budget) = billing.monthly_budget
        {
            return Some((budget - billing.current_usage).max(0.0));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== TeamStatus Tests ====================

    #[test]
    fn test_team_status_active() {
        let status = TeamStatus::Active;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"active\"");
    }

    #[test]
    fn test_team_status_inactive() {
        let status = TeamStatus::Inactive;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"inactive\"");
    }

    #[test]
    fn test_team_status_suspended() {
        let status = TeamStatus::Suspended;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"suspended\"");
    }

    #[test]
    fn test_team_status_deleted() {
        let status = TeamStatus::Deleted;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"deleted\"");
    }

    #[test]
    fn test_team_status_deserialize() {
        let status: TeamStatus = serde_json::from_str("\"active\"").unwrap();
        assert!(matches!(status, TeamStatus::Active));

        let status: TeamStatus = serde_json::from_str("\"suspended\"").unwrap();
        assert!(matches!(status, TeamStatus::Suspended));
    }

    #[test]
    fn test_team_status_clone() {
        let original = TeamStatus::Active;
        let cloned = original.clone();
        let json1 = serde_json::to_string(&original).unwrap();
        let json2 = serde_json::to_string(&cloned).unwrap();
        assert_eq!(json1, json2);
    }

    // ==================== TeamVisibility Tests ====================

    #[test]
    fn test_team_visibility_public() {
        let visibility = TeamVisibility::Public;
        let json = serde_json::to_string(&visibility).unwrap();
        assert_eq!(json, "\"public\"");
    }

    #[test]
    fn test_team_visibility_private() {
        let visibility = TeamVisibility::Private;
        let json = serde_json::to_string(&visibility).unwrap();
        assert_eq!(json, "\"private\"");
    }

    #[test]
    fn test_team_visibility_internal() {
        let visibility = TeamVisibility::Internal;
        let json = serde_json::to_string(&visibility).unwrap();
        assert_eq!(json, "\"internal\"");
    }

    #[test]
    fn test_team_visibility_default() {
        let visibility = TeamVisibility::default();
        assert!(matches!(visibility, TeamVisibility::Private));
    }

    #[test]
    fn test_team_visibility_deserialize() {
        let visibility: TeamVisibility = serde_json::from_str("\"public\"").unwrap();
        assert!(matches!(visibility, TeamVisibility::Public));
    }

    // ==================== Team Creation Tests ====================

    #[test]
    fn test_team_new() {
        let team = Team::new("test-team".to_string(), Some("Test Team".to_string()));

        assert_eq!(team.name, "test-team");
        assert_eq!(team.display_name, Some("Test Team".to_string()));
        assert!(team.description.is_none());
        assert!(matches!(team.status, TeamStatus::Active));
        assert!(team.rate_limits.is_none());
        assert!(team.billing.is_none());
        assert!(team.team_metadata.is_empty());
    }

    #[test]
    fn test_team_new_minimal() {
        let team = Team::new("minimal".to_string(), None);

        assert_eq!(team.name, "minimal");
        assert!(team.display_name.is_none());
        assert!(team.is_active());
    }

    #[test]
    fn test_team_id() {
        let team = Team::new("test".to_string(), None);
        let id = team.id();

        // ID should be a valid UUID
        assert!(!id.is_nil());
    }

    // ==================== Team is_active Tests ====================

    #[test]
    fn test_team_is_active_when_active() {
        let team = Team::new("active-team".to_string(), None);
        assert!(team.is_active());
    }

    #[test]
    fn test_team_is_active_when_inactive() {
        let mut team = Team::new("inactive-team".to_string(), None);
        team.status = TeamStatus::Inactive;
        assert!(!team.is_active());
    }

    #[test]
    fn test_team_is_active_when_suspended() {
        let mut team = Team::new("suspended-team".to_string(), None);
        team.status = TeamStatus::Suspended;
        assert!(!team.is_active());
    }

    #[test]
    fn test_team_is_active_when_deleted() {
        let mut team = Team::new("deleted-team".to_string(), None);
        team.status = TeamStatus::Deleted;
        assert!(!team.is_active());
    }

    // ==================== Team update_usage Tests ====================

    #[test]
    fn test_team_update_usage() {
        let mut team = Team::new("usage-test".to_string(), None);

        team.update_usage(10, 1000, 0.05);

        assert_eq!(team.usage_stats.total_requests, 10);
        assert_eq!(team.usage_stats.total_tokens, 1000);
        assert!((team.usage_stats.total_cost - 0.05).abs() < f64::EPSILON);
    }

    #[test]
    fn test_team_update_usage_cumulative() {
        let mut team = Team::new("cumulative-test".to_string(), None);

        team.update_usage(10, 1000, 0.05);
        team.update_usage(20, 2000, 0.10);
        team.update_usage(30, 3000, 0.15);

        assert_eq!(team.usage_stats.total_requests, 60);
        assert_eq!(team.usage_stats.total_tokens, 6000);
        assert!((team.usage_stats.total_cost - 0.30).abs() < 0.001);
    }

    #[test]
    fn test_team_update_usage_daily_stats() {
        let mut team = Team::new("daily-test".to_string(), None);

        team.update_usage(5, 500, 0.02);

        assert_eq!(team.usage_stats.requests_today, 5);
        assert_eq!(team.usage_stats.tokens_today, 500);
    }

    #[test]
    fn test_team_update_usage_with_billing() {
        use super::super::billing::{BillingPlan, BillingStatus, TeamBilling};

        let mut team = Team::new("billing-test".to_string(), None);
        team.billing = Some(TeamBilling {
            plan: BillingPlan::Professional,
            status: BillingStatus::Active,
            monthly_budget: Some(100.0),
            current_usage: 0.0,
            cycle_start: chrono::Utc::now(),
            cycle_end: chrono::Utc::now() + chrono::Duration::days(30),
            payment_method: None,
            billing_address: None,
        });

        team.update_usage(10, 1000, 5.0);

        assert_eq!(team.billing.as_ref().unwrap().current_usage, 5.0);
    }

    // ==================== Team Budget Tests ====================

    #[test]
    fn test_team_is_over_budget_no_billing() {
        let team = Team::new("no-billing".to_string(), None);
        assert!(!team.is_over_budget());
    }

    #[test]
    fn test_team_is_over_budget_no_budget_limit() {
        use super::super::billing::{BillingPlan, BillingStatus, TeamBilling};

        let mut team = Team::new("no-limit".to_string(), None);
        team.billing = Some(TeamBilling {
            plan: BillingPlan::Enterprise,
            status: BillingStatus::Active,
            monthly_budget: None,
            current_usage: 1000.0,
            cycle_start: chrono::Utc::now(),
            cycle_end: chrono::Utc::now() + chrono::Duration::days(30),
            payment_method: None,
            billing_address: None,
        });

        assert!(!team.is_over_budget());
    }

    #[test]
    fn test_team_is_over_budget_under() {
        use super::super::billing::{BillingPlan, BillingStatus, TeamBilling};

        let mut team = Team::new("under-budget".to_string(), None);
        team.billing = Some(TeamBilling {
            plan: BillingPlan::Starter,
            status: BillingStatus::Active,
            monthly_budget: Some(100.0),
            current_usage: 50.0,
            cycle_start: chrono::Utc::now(),
            cycle_end: chrono::Utc::now() + chrono::Duration::days(30),
            payment_method: None,
            billing_address: None,
        });

        assert!(!team.is_over_budget());
    }

    #[test]
    fn test_team_is_over_budget_at_limit() {
        use super::super::billing::{BillingPlan, BillingStatus, TeamBilling};

        let mut team = Team::new("at-limit".to_string(), None);
        team.billing = Some(TeamBilling {
            plan: BillingPlan::Professional,
            status: BillingStatus::Active,
            monthly_budget: Some(100.0),
            current_usage: 100.0,
            cycle_start: chrono::Utc::now(),
            cycle_end: chrono::Utc::now() + chrono::Duration::days(30),
            payment_method: None,
            billing_address: None,
        });

        assert!(team.is_over_budget());
    }

    #[test]
    fn test_team_is_over_budget_exceeded() {
        use super::super::billing::{BillingPlan, BillingStatus, TeamBilling};

        let mut team = Team::new("over-budget".to_string(), None);
        team.billing = Some(TeamBilling {
            plan: BillingPlan::Starter,
            status: BillingStatus::Active,
            monthly_budget: Some(100.0),
            current_usage: 150.0,
            cycle_start: chrono::Utc::now(),
            cycle_end: chrono::Utc::now() + chrono::Duration::days(30),
            payment_method: None,
            billing_address: None,
        });

        assert!(team.is_over_budget());
    }

    // ==================== Team remaining_budget Tests ====================

    #[test]
    fn test_team_remaining_budget_no_billing() {
        let team = Team::new("no-billing".to_string(), None);
        assert!(team.remaining_budget().is_none());
    }

    #[test]
    fn test_team_remaining_budget_no_limit() {
        use super::super::billing::{BillingPlan, BillingStatus, TeamBilling};

        let mut team = Team::new("no-limit".to_string(), None);
        team.billing = Some(TeamBilling {
            plan: BillingPlan::Enterprise,
            status: BillingStatus::Active,
            monthly_budget: None,
            current_usage: 500.0,
            cycle_start: chrono::Utc::now(),
            cycle_end: chrono::Utc::now() + chrono::Duration::days(30),
            payment_method: None,
            billing_address: None,
        });

        assert!(team.remaining_budget().is_none());
    }

    #[test]
    fn test_team_remaining_budget_positive() {
        use super::super::billing::{BillingPlan, BillingStatus, TeamBilling};

        let mut team = Team::new("positive-budget".to_string(), None);
        team.billing = Some(TeamBilling {
            plan: BillingPlan::Professional,
            status: BillingStatus::Active,
            monthly_budget: Some(100.0),
            current_usage: 30.0,
            cycle_start: chrono::Utc::now(),
            cycle_end: chrono::Utc::now() + chrono::Duration::days(30),
            payment_method: None,
            billing_address: None,
        });

        let remaining = team.remaining_budget().unwrap();
        assert!((remaining - 70.0).abs() < 0.001);
    }

    #[test]
    fn test_team_remaining_budget_zero() {
        use super::super::billing::{BillingPlan, BillingStatus, TeamBilling};

        let mut team = Team::new("zero-budget".to_string(), None);
        team.billing = Some(TeamBilling {
            plan: BillingPlan::Starter,
            status: BillingStatus::Active,
            monthly_budget: Some(100.0),
            current_usage: 100.0,
            cycle_start: chrono::Utc::now(),
            cycle_end: chrono::Utc::now() + chrono::Duration::days(30),
            payment_method: None,
            billing_address: None,
        });

        let remaining = team.remaining_budget().unwrap();
        assert!((remaining - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_team_remaining_budget_exceeded_returns_zero() {
        use super::super::billing::{BillingPlan, BillingStatus, TeamBilling};

        let mut team = Team::new("exceeded-budget".to_string(), None);
        team.billing = Some(TeamBilling {
            plan: BillingPlan::Starter,
            status: BillingStatus::Active,
            monthly_budget: Some(100.0),
            current_usage: 150.0,
            cycle_start: chrono::Utc::now(),
            cycle_end: chrono::Utc::now() + chrono::Duration::days(30),
            payment_method: None,
            billing_address: None,
        });

        let remaining = team.remaining_budget().unwrap();
        assert!((remaining - 0.0).abs() < 0.001);
    }

    // ==================== Team Serialization Tests ====================

    #[test]
    fn test_team_serialize() {
        let team = Team::new(
            "serialize-test".to_string(),
            Some("Serialize Test".to_string()),
        );

        let json = serde_json::to_string(&team).unwrap();

        assert!(json.contains("serialize-test"));
        assert!(json.contains("Serialize Test"));
        assert!(json.contains("\"status\":\"active\""));
    }

    #[test]
    fn test_team_clone() {
        let team = Team::new("clone-test".to_string(), Some("Clone Test".to_string()));
        let cloned = team.clone();

        assert_eq!(team.name, cloned.name);
        assert_eq!(team.display_name, cloned.display_name);
        assert_eq!(team.id(), cloned.id());
    }

    #[test]
    fn test_team_debug() {
        let team = Team::new("debug-test".to_string(), None);
        let debug_str = format!("{:?}", team);

        assert!(debug_str.contains("Team"));
        assert!(debug_str.contains("debug-test"));
    }

    // ==================== Team Metadata Tests ====================

    #[test]
    fn test_team_metadata_empty() {
        let team = Team::new("metadata-test".to_string(), None);
        assert!(team.team_metadata.is_empty());
    }

    #[test]
    fn test_team_metadata_with_data() {
        let mut team = Team::new("metadata-data".to_string(), None);
        team.team_metadata
            .insert("key1".to_string(), serde_json::json!("value1"));
        team.team_metadata
            .insert("key2".to_string(), serde_json::json!(123));

        assert_eq!(team.team_metadata.len(), 2);
        assert_eq!(team.team_metadata.get("key1").unwrap(), "value1");
    }
}
