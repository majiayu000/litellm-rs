//! User operations

use super::roles::UserRole;
use super::settings::UserPreferences;
use super::types::{Team, User};
use crate::storage::database::Database;
use crate::utils::error::gateway_error::{GatewayError, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

/// User management operations
pub struct UserOperations {
    database: Arc<Database>,
}

impl UserOperations {
    /// Create new user operations handler
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    /// Create a new user
    pub async fn create_user(&self, email: String, display_name: Option<String>) -> Result<User> {
        info!("Creating user: {}", email);

        // Check if user already exists
        if self.database.get_user_by_email(&email).await?.is_some() {
            return Err(GatewayError::Conflict("User already exists".to_string()));
        }

        let user = User {
            user_id: Uuid::new_v4().to_string(),
            email,
            display_name,
            first_name: None,
            last_name: None,
            role: UserRole::User,
            teams: vec![],
            permissions: vec![],
            metadata: HashMap::new(),
            max_budget: Some(100.0), // Default budget
            spend: 0.0,
            budget_duration: Some("1m".to_string()),
            budget_reset_at: Some(Utc::now() + chrono::Duration::days(30)),
            is_active: true,
            created_at: Utc::now(),
            last_login_at: None,
            preferences: UserPreferences::default(),
        };

        self.database.um_create_user(&user).await?;
        info!("User created successfully: {}", user.user_id);
        Ok(user)
    }

    /// Get user by ID
    pub async fn get_user(&self, user_id: &str) -> Result<Option<User>> {
        self.database.get_user(user_id).await
    }

    /// Get user by email
    pub async fn get_user_by_email(&self, email: &str) -> Result<Option<User>> {
        self.database.get_user_by_email(email).await
    }

    /// Update user
    pub async fn update_user(&self, user: &User) -> Result<()> {
        self.database.update_user(user).await
    }

    /// Delete user
    pub async fn delete_user(&self, user_id: &str) -> Result<()> {
        info!("Deleting user: {}", user_id);
        self.database.delete_user(user_id).await
    }

    /// Check if user has permission
    pub async fn check_permission(&self, user_id: &str, permission: &str) -> Result<bool> {
        let user = self
            .database
            .get_user(user_id)
            .await?
            .ok_or_else(|| GatewayError::NotFound("User not found".to_string()))?;

        // Super admin has all permissions
        if user.role == UserRole::SuperAdmin {
            return Ok(true);
        }

        // Check direct user permissions
        if user.permissions.contains(&permission.to_string()) {
            return Ok(true);
        }

        // Check team permissions
        for team_id in &user.teams {
            if let Some(team) = self.database.get_team(team_id).await?
                && team.permissions.contains(&permission.to_string())
            {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Update user spend
    pub async fn update_user_spend(&self, user_id: &str, cost: f64) -> Result<()> {
        self.database.update_user_spend(user_id, cost).await
    }

    /// List users with pagination
    pub async fn list_users(&self, offset: u32, limit: u32) -> Result<Vec<User>> {
        self.database.list_users(offset, limit).await
    }

    /// Get user teams
    pub async fn get_user_teams(&self, user_id: &str) -> Result<Vec<Team>> {
        let user = self
            .database
            .get_user(user_id)
            .await?
            .ok_or_else(|| GatewayError::NotFound("User not found".to_string()))?;

        let mut teams = Vec::new();
        for team_id in &user.teams {
            if let Some(team) = self.database.get_team(team_id).await? {
                teams.push(team);
            }
        }

        Ok(teams)
    }
}
