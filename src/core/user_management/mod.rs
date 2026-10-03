//! User and Team management system
//!
//! This module provides comprehensive user and team management for enterprise features.

mod roles;
mod settings;
#[cfg(test)]
mod tests;
mod types;

// Domain records used by the existing auth and storage paths.
pub use roles::{TeamRole, UserRole};
pub use settings::{
    OrganizationSettings, PasswordPolicy, SSOConfig, SSOProvider, TeamSettings, UserPreferences,
};
pub use types::{Organization, Team, TeamMember, User};
