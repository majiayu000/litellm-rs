//! Team manager for business logic and operations
//!
//! This module provides the TeamManager struct that handles all team-related
//! business logic, coordinating between the repository and other services.

use super::repository::TeamRepository;
use crate::core::models::team::{Team, TeamMember, TeamRole, TeamSettings, TeamStatus};
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;

/// Team usage statistics response
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TeamUsageStats {
    /// Team ID
    pub team_id: Uuid,
    /// Team name
    pub team_name: String,
    /// Total requests made
    pub total_requests: u64,
    /// Total tokens consumed
    pub total_tokens: u64,
    /// Total cost incurred
    pub total_cost: f64,
    /// Requests made today
    pub requests_today: u32,
    /// Tokens consumed today
    pub tokens_today: u32,
    /// Cost today
    pub cost_today: f64,
    /// Number of active members
    pub member_count: usize,
    /// Budget usage percentage (if budget is set)
    pub budget_usage_percent: Option<f64>,
    /// Remaining budget (if budget is set)
    pub remaining_budget: Option<f64>,
}

/// Request to create a new team
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreateTeamRequest {
    /// Team name (unique)
    pub name: String,
    /// Display name
    pub display_name: Option<String>,
    /// Description
    pub description: Option<String>,
    /// Initial settings
    pub settings: Option<TeamSettings>,
}

/// Request to update a team
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UpdateTeamRequest {
    /// New name
    pub name: Option<String>,
    /// New display name
    pub display_name: Option<String>,
    /// New description
    pub description: Option<String>,
    /// Updated settings
    pub settings: Option<TeamSettings>,
    /// New status
    pub status: Option<TeamStatus>,
}

/// Request to add a member to a team
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AddMemberRequest {
    /// User ID to add
    pub user_id: Uuid,
    /// Role to assign
    pub role: TeamRole,
}

/// Request to update a member's role
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UpdateRoleRequest {
    /// New role
    pub role: TeamRole,
}

/// Team manager for handling team operations
pub struct TeamManager {
    repository: Arc<dyn TeamRepository>,
}

impl TeamManager {
    /// Create a new team manager with the given repository
    pub fn new(repository: Arc<dyn TeamRepository>) -> Self {
        Self { repository }
    }

    /// Create a new team
    pub async fn create_team(&self, request: CreateTeamRequest) -> Result<Team> {
        info!("Creating team: {}", request.name);

        // Validate team name
        self.validate_team_name(&request.name)?;

        // Check if team name already exists
        if self.repository.get_by_name(&request.name).await?.is_some() {
            return Err(GatewayError::Conflict(format!(
                "Team with name '{}' already exists",
                request.name
            )));
        }

        // Create the team
        let mut team = Team::new(request.name.clone(), request.display_name);
        team.description = request.description;

        if let Some(settings) = request.settings {
            team.settings = settings;
        }

        let created = self.repository.create(team).await?;
        info!("Team created: {} ({})", created.name, created.id());

        Ok(created)
    }

    /// Get a team by ID
    pub async fn get_team(&self, id: Uuid) -> Result<Team> {
        debug!("Fetching team: {}", id);

        self.repository
            .get(id)
            .await?
            .ok_or_else(|| GatewayError::NotFound(format!("Team {} not found", id)))
    }

    /// Get a team by name
    pub async fn get_team_by_name(&self, name: &str) -> Result<Team> {
        debug!("Fetching team by name: {}", name);

        self.repository
            .get_by_name(name)
            .await?
            .ok_or_else(|| GatewayError::NotFound(format!("Team '{}' not found", name)))
    }

    /// Update a team
    pub async fn update_team(&self, id: Uuid, request: UpdateTeamRequest) -> Result<Team> {
        info!("Updating team: {}", id);

        let mut team = self.get_team(id).await?;

        // Update fields if provided
        if let Some(name) = request.name {
            self.validate_team_name(&name)?;

            // Check if new name conflicts with another team
            if let Some(existing) = self.repository.get_by_name(&name).await?
                && existing.id() != id
            {
                return Err(GatewayError::Conflict(format!(
                    "Team with name '{}' already exists",
                    name
                )));
            }
            team.name = name;
        }

        if let Some(display_name) = request.display_name {
            team.display_name = Some(display_name);
        }

        if let Some(description) = request.description {
            team.description = Some(description);
        }

        if let Some(settings) = request.settings {
            team.settings = settings;
        }

        if let Some(status) = request.status {
            team.status = status;
        }

        let updated = self.repository.update(team).await?;
        info!("Team updated: {} ({})", updated.name, updated.id());

        Ok(updated)
    }

    /// Delete a team
    pub async fn delete_team(&self, id: Uuid) -> Result<()> {
        info!("Deleting team: {}", id);

        // Verify team exists
        let team = self.get_team(id).await?;

        // Perform deletion
        self.repository.delete(id).await?;
        info!("Team deleted: {} ({})", team.name, id);

        Ok(())
    }

    /// List teams with pagination
    pub async fn list_teams(&self, offset: u32, limit: u32) -> Result<(Vec<Team>, u64)> {
        debug!("Listing teams: offset={}, limit={}", offset, limit);
        self.repository.list(offset, limit).await
    }

    /// Add a member to a team
    pub async fn add_member(
        &self,
        team_id: Uuid,
        request: AddMemberRequest,
        invited_by: Option<Uuid>,
    ) -> Result<TeamMember> {
        info!(
            "Adding member {} to team {} with role {:?}",
            request.user_id, team_id, request.role
        );

        // Verify team exists
        let _team = self.get_team(team_id).await?;

        // Check if member already exists
        if self
            .repository
            .get_member(team_id, request.user_id)
            .await?
            .is_some()
        {
            return Err(GatewayError::Conflict(format!(
                "User {} is already a member of team {}",
                request.user_id, team_id
            )));
        }

        // Create and add the member
        let member = TeamMember::new(team_id, request.user_id, request.role, invited_by);
        let created = self.repository.add_member(member).await?;

        info!(
            "Member {} added to team {} with role {:?}",
            created.user_id, team_id, created.role
        );

        Ok(created)
    }

    /// Get a team member
    pub async fn get_member(&self, team_id: Uuid, user_id: Uuid) -> Result<TeamMember> {
        debug!("Fetching member {} from team {}", user_id, team_id);

        self.repository
            .get_member(team_id, user_id)
            .await?
            .ok_or_else(|| {
                GatewayError::NotFound(format!("Member {} not found in team {}", user_id, team_id))
            })
    }

    /// Update a member's role
    pub async fn update_member_role(
        &self,
        team_id: Uuid,
        user_id: Uuid,
        request: UpdateRoleRequest,
    ) -> Result<TeamMember> {
        info!(
            "Updating role for member {} in team {} to {:?}",
            user_id, team_id, request.role
        );

        // Verify member exists
        let _member = self.get_member(team_id, user_id).await?;

        // Prevent removing the last owner
        if matches!(
            request.role,
            TeamRole::Member | TeamRole::Viewer | TeamRole::Manager
        ) {
            let members = self.repository.list_members(team_id).await?;
            let owner_count = members
                .iter()
                .filter(|m| matches!(m.role, TeamRole::Owner))
                .count();

            let current_member = self.get_member(team_id, user_id).await?;
            if matches!(current_member.role, TeamRole::Owner) && owner_count <= 1 {
                return Err(GatewayError::Validation(
                    "Cannot remove the last owner from the team".to_string(),
                ));
            }
        }

        let updated = self
            .repository
            .update_member_role(team_id, user_id, request.role)
            .await?;

        info!(
            "Member {} role updated to {:?} in team {}",
            user_id, updated.role, team_id
        );

        Ok(updated)
    }

    /// Remove a member from a team
    pub async fn remove_member(&self, team_id: Uuid, user_id: Uuid) -> Result<()> {
        info!("Removing member {} from team {}", user_id, team_id);

        // Verify member exists
        let member = self.get_member(team_id, user_id).await?;

        // Prevent removing the last owner
        if matches!(member.role, TeamRole::Owner) {
            let members = self.repository.list_members(team_id).await?;
            let owner_count = members
                .iter()
                .filter(|m| matches!(m.role, TeamRole::Owner))
                .count();

            if owner_count <= 1 {
                return Err(GatewayError::Validation(
                    "Cannot remove the last owner from the team".to_string(),
                ));
            }
        }

        self.repository.remove_member(team_id, user_id).await?;
        info!("Member {} removed from team {}", user_id, team_id);

        Ok(())
    }

    /// List all members of a team
    pub async fn list_members(&self, team_id: Uuid) -> Result<Vec<TeamMember>> {
        debug!("Listing members of team {}", team_id);

        // Verify team exists
        let _team = self.get_team(team_id).await?;

        self.repository.list_members(team_id).await
    }

    /// Get teams for a user
    pub async fn get_user_teams(&self, user_id: Uuid) -> Result<Vec<Team>> {
        debug!("Fetching teams for user {}", user_id);
        self.repository.get_user_teams(user_id).await
    }

    /// Get usage statistics for a team
    pub async fn get_team_usage(&self, team_id: Uuid) -> Result<TeamUsageStats> {
        debug!("Fetching usage stats for team {}", team_id);

        let team = self.get_team(team_id).await?;
        let members = self.repository.list_members(team_id).await?;

        let budget_usage_percent = team.billing.as_ref().and_then(|b| {
            b.monthly_budget
                .map(|budget| (b.current_usage / budget) * 100.0)
        });

        let remaining_budget = team.remaining_budget();

        Ok(TeamUsageStats {
            team_id: team.id(),
            team_name: team.name,
            total_requests: team.usage_stats.total_requests,
            total_tokens: team.usage_stats.total_tokens,
            total_cost: team.usage_stats.total_cost,
            requests_today: team.usage_stats.requests_today,
            tokens_today: team.usage_stats.tokens_today,
            cost_today: team.usage_stats.cost_today,
            member_count: members.len(),
            budget_usage_percent,
            remaining_budget,
        })
    }

    /// Update team settings
    pub async fn update_settings(&self, team_id: Uuid, settings: TeamSettings) -> Result<Team> {
        info!("Updating settings for team {}", team_id);

        let mut team = self.get_team(team_id).await?;
        team.settings = settings;

        self.repository.update(team).await
    }

    /// Check if a user has a specific role in a team
    pub async fn check_user_role(
        &self,
        team_id: Uuid,
        user_id: Uuid,
        required_roles: &[TeamRole],
    ) -> Result<bool> {
        let member = self.repository.get_member(team_id, user_id).await?;

        match member {
            Some(m) => {
                let has_role = required_roles
                    .iter()
                    .any(|r| std::mem::discriminant(r) == std::mem::discriminant(&m.role));
                Ok(has_role)
            }
            None => Ok(false),
        }
    }

    /// Check if a user is an admin or owner of a team
    pub async fn is_team_admin(&self, team_id: Uuid, user_id: Uuid) -> Result<bool> {
        self.check_user_role(team_id, user_id, &[TeamRole::Owner, TeamRole::Admin])
            .await
    }

    // Helper method to validate team name
    fn validate_team_name(&self, name: &str) -> Result<()> {
        if name.is_empty() {
            return Err(GatewayError::Validation(
                "Team name cannot be empty".to_string(),
            ));
        }

        if name.len() > 100 {
            return Err(GatewayError::Validation(
                "Team name cannot exceed 100 characters".to_string(),
            ));
        }

        // Check for valid characters (alphanumeric, hyphens, underscores)
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(GatewayError::Validation(
                "Team name can only contain alphanumeric characters, hyphens, and underscores"
                    .to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
