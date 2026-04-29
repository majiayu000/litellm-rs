//! Team repository trait and implementations
//!
//! This module provides storage abstractions for team management.

use crate::core::models::team::{Team, TeamMember, TeamRole, TeamStatus};
use crate::utils::error::gateway_error::{GatewayError, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;

/// Team repository trait for abstracting storage operations
#[async_trait]
pub trait TeamRepository: Send + Sync {
    /// Create a new team
    async fn create(&self, team: Team) -> Result<Team>;

    /// Get a team by ID
    async fn get(&self, id: Uuid) -> Result<Option<Team>>;

    /// Get a team by name
    async fn get_by_name(&self, name: &str) -> Result<Option<Team>>;

    /// Update an existing team
    async fn update(&self, team: Team) -> Result<Team>;

    /// Delete a team by ID
    async fn delete(&self, id: Uuid) -> Result<()>;

    /// List all teams with pagination
    async fn list(&self, offset: u32, limit: u32) -> Result<(Vec<Team>, u64)>;

    /// Count total teams
    async fn count(&self) -> Result<u64>;

    /// Add a member to a team
    async fn add_member(&self, member: TeamMember) -> Result<TeamMember>;

    /// Get a team member
    async fn get_member(&self, team_id: Uuid, user_id: Uuid) -> Result<Option<TeamMember>>;

    /// Update a team member's role
    async fn update_member_role(
        &self,
        team_id: Uuid,
        user_id: Uuid,
        role: TeamRole,
    ) -> Result<TeamMember>;

    /// Remove a member from a team
    async fn remove_member(&self, team_id: Uuid, user_id: Uuid) -> Result<()>;

    /// List members of a team
    async fn list_members(&self, team_id: Uuid) -> Result<Vec<TeamMember>>;

    /// Get teams for a user
    async fn get_user_teams(&self, user_id: Uuid) -> Result<Vec<Team>>;
}

/// In-memory team repository for testing and development
pub struct InMemoryTeamRepository {
    teams: RwLock<HashMap<Uuid, Team>>,
    members: RwLock<HashMap<(Uuid, Uuid), TeamMember>>,
}

impl InMemoryTeamRepository {
    /// Create a new in-memory repository
    pub fn new() -> Self {
        Self {
            teams: RwLock::new(HashMap::new()),
            members: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryTeamRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TeamRepository for InMemoryTeamRepository {
    async fn create(&self, team: Team) -> Result<Team> {
        let mut teams = self
            .teams
            .write()
            .map_err(|_| GatewayError::Internal("Lock poisoned".to_string()))?;

        // Check for name conflict
        if teams.values().any(|t| t.name == team.name) {
            return Err(GatewayError::Conflict(format!(
                "Team with name '{}' already exists",
                team.name
            )));
        }

        let id = team.id();
        teams.insert(id, team.clone());
        Ok(team)
    }

    async fn get(&self, id: Uuid) -> Result<Option<Team>> {
        let teams = self
            .teams
            .read()
            .map_err(|_| GatewayError::Internal("Lock poisoned".to_string()))?;
        Ok(teams.get(&id).cloned())
    }

    async fn get_by_name(&self, name: &str) -> Result<Option<Team>> {
        let teams = self
            .teams
            .read()
            .map_err(|_| GatewayError::Internal("Lock poisoned".to_string()))?;
        Ok(teams.values().find(|t| t.name == name).cloned())
    }

    async fn update(&self, mut team: Team) -> Result<Team> {
        let mut teams = self
            .teams
            .write()
            .map_err(|_| GatewayError::Internal("Lock poisoned".to_string()))?;

        let id = team.id();
        if !teams.contains_key(&id) {
            return Err(GatewayError::NotFound(format!("Team {} not found", id)));
        }

        // Check for name conflict with other teams
        if teams.values().any(|t| t.name == team.name && t.id() != id) {
            return Err(GatewayError::Conflict(format!(
                "Team with name '{}' already exists",
                team.name
            )));
        }

        team.metadata.touch();
        teams.insert(id, team.clone());
        Ok(team)
    }

    async fn delete(&self, id: Uuid) -> Result<()> {
        let mut teams = self
            .teams
            .write()
            .map_err(|_| GatewayError::Internal("Lock poisoned".to_string()))?;

        if teams.remove(&id).is_none() {
            return Err(GatewayError::NotFound(format!("Team {} not found", id)));
        }

        // Remove all members of the team
        let mut members = self
            .members
            .write()
            .map_err(|_| GatewayError::Internal("Lock poisoned".to_string()))?;
        members.retain(|(team_id, _), _| *team_id != id);

        Ok(())
    }

    async fn list(&self, offset: u32, limit: u32) -> Result<(Vec<Team>, u64)> {
        let teams = self
            .teams
            .read()
            .map_err(|_| GatewayError::Internal("Lock poisoned".to_string()))?;

        let total = teams.len() as u64;
        let mut team_list: Vec<Team> = teams
            .values()
            .filter(|t| !matches!(t.status, TeamStatus::Deleted))
            .cloned()
            .collect();

        // Sort by created_at descending
        team_list.sort_by_key(|b| std::cmp::Reverse(b.metadata.created_at));

        let paginated: Vec<Team> = team_list
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect();

        Ok((paginated, total))
    }

    async fn count(&self) -> Result<u64> {
        let teams = self
            .teams
            .read()
            .map_err(|_| GatewayError::Internal("Lock poisoned".to_string()))?;
        Ok(teams
            .values()
            .filter(|t| !matches!(t.status, TeamStatus::Deleted))
            .count() as u64)
    }

    async fn add_member(&self, member: TeamMember) -> Result<TeamMember> {
        // Verify team exists
        {
            let teams = self
                .teams
                .read()
                .map_err(|_| GatewayError::Internal("Lock poisoned".to_string()))?;
            if !teams.contains_key(&member.team_id) {
                return Err(GatewayError::NotFound(format!(
                    "Team {} not found",
                    member.team_id
                )));
            }
        }

        let mut members = self
            .members
            .write()
            .map_err(|_| GatewayError::Internal("Lock poisoned".to_string()))?;

        let key = (member.team_id, member.user_id);
        if members.contains_key(&key) {
            return Err(GatewayError::Conflict(format!(
                "User {} is already a member of team {}",
                member.user_id, member.team_id
            )));
        }

        members.insert(key, member.clone());
        Ok(member)
    }

    async fn get_member(&self, team_id: Uuid, user_id: Uuid) -> Result<Option<TeamMember>> {
        let members = self
            .members
            .read()
            .map_err(|_| GatewayError::Internal("Lock poisoned".to_string()))?;
        Ok(members.get(&(team_id, user_id)).cloned())
    }

    async fn update_member_role(
        &self,
        team_id: Uuid,
        user_id: Uuid,
        role: TeamRole,
    ) -> Result<TeamMember> {
        let mut members = self
            .members
            .write()
            .map_err(|_| GatewayError::Internal("Lock poisoned".to_string()))?;

        let key = (team_id, user_id);
        let member = members.get_mut(&key).ok_or_else(|| {
            GatewayError::NotFound(format!("Member {} not found in team {}", user_id, team_id))
        })?;

        member.role = role;
        member.metadata.touch();
        Ok(member.clone())
    }

    async fn remove_member(&self, team_id: Uuid, user_id: Uuid) -> Result<()> {
        let mut members = self
            .members
            .write()
            .map_err(|_| GatewayError::Internal("Lock poisoned".to_string()))?;

        let key = (team_id, user_id);
        if members.remove(&key).is_none() {
            return Err(GatewayError::NotFound(format!(
                "Member {} not found in team {}",
                user_id, team_id
            )));
        }

        Ok(())
    }

    async fn list_members(&self, team_id: Uuid) -> Result<Vec<TeamMember>> {
        let members = self
            .members
            .read()
            .map_err(|_| GatewayError::Internal("Lock poisoned".to_string()))?;

        let team_members: Vec<TeamMember> = members
            .iter()
            .filter(|((tid, _), _)| *tid == team_id)
            .map(|(_, m)| m.clone())
            .collect();

        Ok(team_members)
    }

    async fn get_user_teams(&self, user_id: Uuid) -> Result<Vec<Team>> {
        let members = self
            .members
            .read()
            .map_err(|_| GatewayError::Internal("Lock poisoned".to_string()))?;

        let team_ids: Vec<Uuid> = members
            .iter()
            .filter(|((_, uid), _)| *uid == user_id)
            .map(|((tid, _), _)| *tid)
            .collect();

        let teams = self
            .teams
            .read()
            .map_err(|_| GatewayError::Internal("Lock poisoned".to_string()))?;

        let user_teams: Vec<Team> = team_ids
            .into_iter()
            .filter_map(|tid| teams.get(&tid).cloned())
            .collect();

        Ok(user_teams)
    }
}

#[cfg(feature = "postgres")]
pub mod postgres {
    //! PostgreSQL implementation of TeamRepository
    //!
    //! This module provides a PostgreSQL-backed implementation.

    use super::*;
    use crate::storage::database::Database;
    use std::sync::Arc;

    /// PostgreSQL team repository
    pub struct PostgresTeamRepository {
        _db: Arc<Database>,
    }

    impl PostgresTeamRepository {
        /// Create a new PostgreSQL repository
        pub fn new(db: Arc<Database>) -> Self {
            Self { _db: db }
        }
    }

    #[async_trait]
    impl TeamRepository for PostgresTeamRepository {
        async fn create(&self, _team: Team) -> Result<Team> {
            // NOTE: PostgreSQL storage for teams not yet implemented.
            // This is a placeholder that would use SeaORM entities
            Err(GatewayError::NotImplemented(
                "PostgreSQL team storage not yet implemented".to_string(),
            ))
        }

        async fn get(&self, _id: Uuid) -> Result<Option<Team>> {
            Err(GatewayError::NotImplemented(
                "PostgreSQL team storage not yet implemented".to_string(),
            ))
        }

        async fn get_by_name(&self, _name: &str) -> Result<Option<Team>> {
            Err(GatewayError::NotImplemented(
                "PostgreSQL team storage not yet implemented".to_string(),
            ))
        }

        async fn update(&self, _team: Team) -> Result<Team> {
            Err(GatewayError::NotImplemented(
                "PostgreSQL team storage not yet implemented".to_string(),
            ))
        }

        async fn delete(&self, _id: Uuid) -> Result<()> {
            Err(GatewayError::NotImplemented(
                "PostgreSQL team storage not yet implemented".to_string(),
            ))
        }

        async fn list(&self, _offset: u32, _limit: u32) -> Result<(Vec<Team>, u64)> {
            Err(GatewayError::NotImplemented(
                "PostgreSQL team storage not yet implemented".to_string(),
            ))
        }

        async fn count(&self) -> Result<u64> {
            Err(GatewayError::NotImplemented(
                "PostgreSQL team storage not yet implemented".to_string(),
            ))
        }

        async fn add_member(&self, _member: TeamMember) -> Result<TeamMember> {
            Err(GatewayError::NotImplemented(
                "PostgreSQL team storage not yet implemented".to_string(),
            ))
        }

        async fn get_member(&self, _team_id: Uuid, _user_id: Uuid) -> Result<Option<TeamMember>> {
            Err(GatewayError::NotImplemented(
                "PostgreSQL team storage not yet implemented".to_string(),
            ))
        }

        async fn update_member_role(
            &self,
            _team_id: Uuid,
            _user_id: Uuid,
            _role: TeamRole,
        ) -> Result<TeamMember> {
            Err(GatewayError::NotImplemented(
                "PostgreSQL team storage not yet implemented".to_string(),
            ))
        }

        async fn remove_member(&self, _team_id: Uuid, _user_id: Uuid) -> Result<()> {
            Err(GatewayError::NotImplemented(
                "PostgreSQL team storage not yet implemented".to_string(),
            ))
        }

        async fn list_members(&self, _team_id: Uuid) -> Result<Vec<TeamMember>> {
            Err(GatewayError::NotImplemented(
                "PostgreSQL team storage not yet implemented".to_string(),
            ))
        }

        async fn get_user_teams(&self, _user_id: Uuid) -> Result<Vec<Team>> {
            Err(GatewayError::NotImplemented(
                "PostgreSQL team storage not yet implemented".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_team() {
        let repo = InMemoryTeamRepository::new();
        let team = Team::new("test-team".to_string(), Some("Test Team".to_string()));

        let created = repo.create(team.clone()).await.unwrap();
        assert_eq!(created.name, "test-team");
    }

    #[tokio::test]
    async fn test_get_team() {
        let repo = InMemoryTeamRepository::new();
        let team = Team::new("test-team".to_string(), None);
        let id = team.id();

        repo.create(team).await.unwrap();
        let fetched = repo.get(id).await.unwrap();

        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().name, "test-team");
    }

    #[tokio::test]
    async fn test_get_team_not_found() {
        let repo = InMemoryTeamRepository::new();
        let result = repo.get(Uuid::new_v4()).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_by_name() {
        let repo = InMemoryTeamRepository::new();
        let team = Team::new("unique-name".to_string(), None);
        repo.create(team).await.unwrap();

        let fetched = repo.get_by_name("unique-name").await.unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().name, "unique-name");
    }

    #[tokio::test]
    async fn test_update_team() {
        let repo = InMemoryTeamRepository::new();
        let team = Team::new("original".to_string(), None);
        let id = team.id();

        repo.create(team).await.unwrap();

        let mut updated_team = repo.get(id).await.unwrap().unwrap();
        updated_team.description = Some("Updated description".to_string());

        let result = repo.update(updated_team).await.unwrap();
        assert_eq!(result.description, Some("Updated description".to_string()));
    }

    #[tokio::test]
    async fn test_delete_team() {
        let repo = InMemoryTeamRepository::new();
        let team = Team::new("to-delete".to_string(), None);
        let id = team.id();

        repo.create(team).await.unwrap();
        repo.delete(id).await.unwrap();

        let fetched = repo.get(id).await.unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn test_list_teams() {
        let repo = InMemoryTeamRepository::new();

        for i in 0..5 {
            let team = Team::new(format!("team-{}", i), None);
            repo.create(team).await.unwrap();
        }

        let (teams, total) = repo.list(0, 10).await.unwrap();
        assert_eq!(teams.len(), 5);
        assert_eq!(total, 5);
    }

    #[tokio::test]
    async fn test_list_teams_pagination() {
        let repo = InMemoryTeamRepository::new();

        for i in 0..10 {
            let team = Team::new(format!("team-{}", i), None);
            repo.create(team).await.unwrap();
        }

        let (teams, total) = repo.list(0, 5).await.unwrap();
        assert_eq!(teams.len(), 5);
        assert_eq!(total, 10);

        let (teams2, _) = repo.list(5, 5).await.unwrap();
        assert_eq!(teams2.len(), 5);
    }

    #[tokio::test]
    async fn test_duplicate_team_name() {
        let repo = InMemoryTeamRepository::new();
        let team1 = Team::new("duplicate".to_string(), None);
        let team2 = Team::new("duplicate".to_string(), None);

        repo.create(team1).await.unwrap();
        let result = repo.create(team2).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_add_member() {
        let repo = InMemoryTeamRepository::new();
        let team = Team::new("team".to_string(), None);
        let team_id = team.id();
        repo.create(team).await.unwrap();

        let user_id = Uuid::new_v4();
        let member = TeamMember::new(team_id, user_id, TeamRole::Member, None);

        let created = repo.add_member(member).await.unwrap();
        assert_eq!(created.user_id, user_id);
        assert!(matches!(created.role, TeamRole::Member));
    }

    #[tokio::test]
    async fn test_get_member() {
        let repo = InMemoryTeamRepository::new();
        let team = Team::new("team".to_string(), None);
        let team_id = team.id();
        repo.create(team).await.unwrap();

        let user_id = Uuid::new_v4();
        let member = TeamMember::new(team_id, user_id, TeamRole::Admin, None);
        repo.add_member(member).await.unwrap();

        let fetched = repo.get_member(team_id, user_id).await.unwrap();
        assert!(fetched.is_some());
        assert!(matches!(fetched.unwrap().role, TeamRole::Admin));
    }

    #[tokio::test]
    async fn test_update_member_role() {
        let repo = InMemoryTeamRepository::new();
        let team = Team::new("team".to_string(), None);
        let team_id = team.id();
        repo.create(team).await.unwrap();

        let user_id = Uuid::new_v4();
        let member = TeamMember::new(team_id, user_id, TeamRole::Member, None);
        repo.add_member(member).await.unwrap();

        let updated = repo
            .update_member_role(team_id, user_id, TeamRole::Admin)
            .await
            .unwrap();
        assert!(matches!(updated.role, TeamRole::Admin));
    }

    #[tokio::test]
    async fn test_remove_member() {
        let repo = InMemoryTeamRepository::new();
        let team = Team::new("team".to_string(), None);
        let team_id = team.id();
        repo.create(team).await.unwrap();

        let user_id = Uuid::new_v4();
        let member = TeamMember::new(team_id, user_id, TeamRole::Member, None);
        repo.add_member(member).await.unwrap();

        repo.remove_member(team_id, user_id).await.unwrap();

        let fetched = repo.get_member(team_id, user_id).await.unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn test_list_members() {
        let repo = InMemoryTeamRepository::new();
        let team = Team::new("team".to_string(), None);
        let team_id = team.id();
        repo.create(team).await.unwrap();

        for _ in 0..3 {
            let member = TeamMember::new(team_id, Uuid::new_v4(), TeamRole::Member, None);
            repo.add_member(member).await.unwrap();
        }

        let members = repo.list_members(team_id).await.unwrap();
        assert_eq!(members.len(), 3);
    }

    #[tokio::test]
    async fn test_get_user_teams() {
        let repo = InMemoryTeamRepository::new();
        let user_id = Uuid::new_v4();

        for i in 0..3 {
            let team = Team::new(format!("team-{}", i), None);
            let team_id = team.id();
            repo.create(team).await.unwrap();

            let member = TeamMember::new(team_id, user_id, TeamRole::Member, None);
            repo.add_member(member).await.unwrap();
        }

        let teams = repo.get_user_teams(user_id).await.unwrap();
        assert_eq!(teams.len(), 3);
    }

    #[tokio::test]
    async fn test_add_member_to_nonexistent_team() {
        let repo = InMemoryTeamRepository::new();
        let member = TeamMember::new(Uuid::new_v4(), Uuid::new_v4(), TeamRole::Member, None);

        let result = repo.add_member(member).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_duplicate_member() {
        let repo = InMemoryTeamRepository::new();
        let team = Team::new("team".to_string(), None);
        let team_id = team.id();
        repo.create(team).await.unwrap();

        let user_id = Uuid::new_v4();
        let member1 = TeamMember::new(team_id, user_id, TeamRole::Member, None);
        let member2 = TeamMember::new(team_id, user_id, TeamRole::Admin, None);

        repo.add_member(member1).await.unwrap();
        let result = repo.add_member(member2).await;

        assert!(result.is_err());
    }
}
