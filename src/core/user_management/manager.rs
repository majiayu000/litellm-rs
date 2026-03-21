//! User management system - main facade

use super::roles::TeamRole;
use super::team_ops::TeamOperations;
use super::types::{Organization, Team, User};
use super::user_ops::UserOperations;
use crate::storage::database::Database;
use crate::utils::error::gateway_error::Result;
use std::sync::Arc;

/// User management system
pub struct UserManager {
    user_ops: UserOperations,
    team_ops: TeamOperations,
}

impl UserManager {
    /// Create a new user manager
    pub fn new(database: Arc<Database>) -> Self {
        Self {
            user_ops: UserOperations::new(Arc::clone(&database)),
            team_ops: TeamOperations::new(database),
        }
    }

    // User operations

    /// Create a new user
    pub async fn create_user(&self, email: String, display_name: Option<String>) -> Result<User> {
        self.user_ops.create_user(email, display_name).await
    }

    /// Get user by ID
    pub async fn get_user(&self, user_id: &str) -> Result<Option<User>> {
        self.user_ops.get_user(user_id).await
    }

    /// Get user by email
    pub async fn get_user_by_email(&self, email: &str) -> Result<Option<User>> {
        self.user_ops.get_user_by_email(email).await
    }

    /// Update user
    pub async fn update_user(&self, user: &User) -> Result<()> {
        self.user_ops.update_user(user).await
    }

    /// Delete user
    pub async fn delete_user(&self, user_id: &str) -> Result<()> {
        self.user_ops.delete_user(user_id).await
    }

    /// Check if user has permission
    pub async fn check_permission(&self, user_id: &str, permission: &str) -> Result<bool> {
        self.user_ops.check_permission(user_id, permission).await
    }

    /// Update user spend
    pub async fn update_user_spend(&self, user_id: &str, cost: f64) -> Result<()> {
        self.user_ops.update_user_spend(user_id, cost).await
    }

    /// List users with pagination
    pub async fn list_users(&self, offset: u32, limit: u32) -> Result<Vec<User>> {
        self.user_ops.list_users(offset, limit).await
    }

    /// Get user teams
    pub async fn get_user_teams(&self, user_id: &str) -> Result<Vec<Team>> {
        self.user_ops.get_user_teams(user_id).await
    }

    // Team operations

    /// Create a new team
    pub async fn create_team(
        &self,
        team_name: String,
        description: Option<String>,
        organization_id: Option<String>,
        creator_id: String,
    ) -> Result<Team> {
        self.team_ops
            .create_team(team_name, description, organization_id, creator_id)
            .await
    }

    /// Get team by ID
    pub async fn get_team(&self, team_id: &str) -> Result<Option<Team>> {
        self.team_ops.get_team(team_id).await
    }

    /// Add user to team
    pub async fn add_user_to_team(
        &self,
        team_id: &str,
        user_id: &str,
        role: TeamRole,
    ) -> Result<()> {
        self.team_ops.add_user_to_team(team_id, user_id, role).await
    }

    /// Remove user from team
    pub async fn remove_user_from_team(&self, team_id: &str, user_id: &str) -> Result<()> {
        self.team_ops.remove_user_from_team(team_id, user_id).await
    }

    /// Update team spend
    pub async fn update_team_spend(&self, team_id: &str, cost: f64) -> Result<()> {
        self.team_ops.update_team_spend(team_id, cost).await
    }

    /// List teams with pagination
    pub async fn list_teams(&self, offset: u32, limit: u32) -> Result<Vec<Team>> {
        self.team_ops.list_teams(offset, limit).await
    }

    // Organization operations

    /// Create organization
    pub async fn create_organization(
        &self,
        organization_name: String,
        description: Option<String>,
        creator_id: String,
    ) -> Result<Organization> {
        self.team_ops
            .create_organization(organization_name, description, creator_id)
            .await
    }

    /// Get organization by ID
    pub async fn get_organization(&self, organization_id: &str) -> Result<Option<Organization>> {
        self.team_ops.get_organization(organization_id).await
    }
}
