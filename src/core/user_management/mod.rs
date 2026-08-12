//! User and Team management system
//!
//! This module provides comprehensive user and team management for enterprise features.

#[cfg(feature = "user-management")]
mod manager;
mod roles;
mod settings;
#[cfg(feature = "user-management")]
mod team_ops;
#[cfg(all(test, feature = "user-management"))]
mod tests;
mod types;
#[cfg(feature = "user-management")]
mod user_ops;

// Re-export all public types for backward compatibility
#[deprecated(
    since = "0.6.0",
    note = "UserManager is a default-off compatibility surface scheduled for removal in 0.7.0"
)]
#[cfg(feature = "user-management")]
pub use manager::UserManager;
pub use roles::{TeamRole, UserRole};
pub use settings::{
    OrganizationSettings, PasswordPolicy, SSOConfig, SSOProvider, TeamSettings, UserPreferences,
};
pub use types::{Organization, Team, TeamMember, User};
