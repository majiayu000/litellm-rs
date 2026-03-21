//! Team and organization operations

use super::roles::TeamRole;
use super::settings::{OrganizationSettings, TeamSettings};
use super::types::{Organization, Team, TeamMember};
use crate::storage::database::Database;
use crate::utils::error::gateway_error::{GatewayError, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

/// Team and organization operations
pub struct TeamOperations {
    database: Arc<Database>,
}

impl TeamOperations {
    /// Create new team operations handler
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    /// Create a new team
    pub async fn create_team(
        &self,
        team_name: String,
        description: Option<String>,
        organization_id: Option<String>,
        creator_id: String,
    ) -> Result<Team> {
        info!("Creating team: {}", team_name);

        let team = Team {
            team_id: Uuid::new_v4().to_string(),
            team_name,
            description,
            organization_id,
            members: vec![TeamMember {
                user_id: creator_id,
                role: TeamRole::Owner,
                joined_at: Utc::now(),
                is_active: true,
            }],
            permissions: vec![],
            models: vec![],
            max_budget: Some(1000.0), // Default team budget
            spend: 0.0,
            budget_duration: Some("1m".to_string()),
            budget_reset_at: Some(Utc::now() + chrono::Duration::days(30)),
            metadata: HashMap::new(),
            is_active: true,
            created_at: Utc::now(),
            settings: TeamSettings::default(),
        };

        self.database.create_team(&team).await?;
        info!("Team created successfully: {}", team.team_id);
        Ok(team)
    }

    /// Get team by ID
    pub async fn get_team(&self, team_id: &str) -> Result<Option<Team>> {
        self.database.get_team(team_id).await
    }

    /// Add user to team
    pub async fn add_user_to_team(
        &self,
        team_id: &str,
        user_id: &str,
        role: TeamRole,
    ) -> Result<()> {
        info!(
            "Adding user {} to team {} with role {:?}",
            user_id, team_id, role
        );

        let mut team = self
            .database
            .get_team(team_id)
            .await?
            .ok_or_else(|| GatewayError::NotFound("Team not found".to_string()))?;

        // Check if user is already a member
        if team.members.iter().any(|m| m.user_id == user_id) {
            return Err(GatewayError::Conflict(
                "User is already a team member".to_string(),
            ));
        }

        // Add member
        team.members.push(TeamMember {
            user_id: user_id.to_string(),
            role,
            joined_at: Utc::now(),
            is_active: true,
        });

        self.database.update_team(&team).await?;

        // Update user's teams list
        if let Some(mut user) = self.database.get_user(user_id).await? {
            user.teams.push(team_id.to_string());
            self.database.update_user(&user).await?;
        }

        Ok(())
    }

    /// Remove user from team
    pub async fn remove_user_from_team(&self, team_id: &str, user_id: &str) -> Result<()> {
        info!("Removing user {} from team {}", user_id, team_id);

        let mut team = self
            .database
            .get_team(team_id)
            .await?
            .ok_or_else(|| GatewayError::NotFound("Team not found".to_string()))?;

        // Remove member
        team.members.retain(|m| m.user_id != user_id);
        self.database.update_team(&team).await?;

        // Update user's teams list
        if let Some(mut user) = self.database.get_user(user_id).await? {
            user.teams.retain(|t| t != team_id);
            self.database.update_user(&user).await?;
        }

        Ok(())
    }

    /// Update team spend
    pub async fn update_team_spend(&self, team_id: &str, cost: f64) -> Result<()> {
        self.database.update_team_spend(team_id, cost).await
    }

    /// List teams with pagination
    pub async fn list_teams(&self, offset: u32, limit: u32) -> Result<Vec<Team>> {
        self.database.list_teams(offset, limit).await
    }

    /// Create organization
    pub async fn create_organization(
        &self,
        organization_name: String,
        description: Option<String>,
        creator_id: String,
    ) -> Result<Organization> {
        info!("Creating organization: {}", organization_name);

        let organization = Organization {
            organization_id: Uuid::new_v4().to_string(),
            organization_name,
            description,
            domain: None,
            teams: vec![],
            admins: vec![creator_id],
            max_budget: Some(10000.0), // Default org budget
            spend: 0.0,
            budget_duration: Some("1m".to_string()),
            budget_reset_at: Some(Utc::now() + chrono::Duration::days(30)),
            metadata: HashMap::new(),
            is_active: true,
            created_at: Utc::now(),
            settings: OrganizationSettings::default(),
        };

        self.database.create_organization(&organization).await?;
        info!(
            "Organization created successfully: {}",
            organization.organization_id
        );
        Ok(organization)
    }

    /// Get organization by ID
    pub async fn get_organization(&self, organization_id: &str) -> Result<Option<Organization>> {
        self.database.get_organization(organization_id).await
    }
}
