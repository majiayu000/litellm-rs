//! SeaORM-backed TeamRepository implementation
//!
//! Stores `core::models::team::{Team, TeamMember}` as JSON snapshots in the
//! `teams` and `team_members` tables created by migration
//! `m20240301_000002_create_teams_table`.  Works with both SQLite and
//! PostgreSQL backends via the live `SeaOrmDatabase` connection.

use crate::core::models::team::{Team, TeamMember, TeamRole, TeamStatus};
use crate::core::teams::repository::TeamRepository;
use crate::utils::error::gateway_error::{GatewayError, Result};
use async_trait::async_trait;
use sea_orm::{ConnectionTrait, DbBackend, Statement, Value};
use std::sync::Arc;
use uuid::Uuid;

use super::types::{DatabaseBackendType, SeaOrmDatabase};

/// SeaORM-backed team repository (supports SQLite and PostgreSQL).
pub struct SeaOrmTeamRepository {
    db: Arc<SeaOrmDatabase>,
}

impl SeaOrmTeamRepository {
    /// Create a new repository wrapping the given database connection.
    pub fn new(db: Arc<SeaOrmDatabase>) -> Self {
        Self { db }
    }

    fn backend(&self) -> DbBackend {
        match self.db.backend_type {
            DatabaseBackendType::PostgreSQL => DbBackend::Postgres,
            DatabaseBackendType::SQLite => DbBackend::Sqlite,
        }
    }

    /// Return the positional placeholder for parameter `n` (1-based).
    fn ph(&self, n: usize) -> String {
        match self.db.backend_type {
            DatabaseBackendType::PostgreSQL => format!("${}", n),
            DatabaseBackendType::SQLite => "?".to_string(),
        }
    }

    fn to_json<T: serde::Serialize>(v: &T) -> Result<String> {
        serde_json::to_string(v).map_err(|e| GatewayError::Internal(e.to_string()))
    }

    fn from_json<T: serde::de::DeserializeOwned>(s: &str) -> Result<T> {
        serde_json::from_str(s).map_err(|e| GatewayError::Internal(e.to_string()))
    }
}

#[async_trait]
impl TeamRepository for SeaOrmTeamRepository {
    async fn create(&self, team: Team) -> Result<Team> {
        let id = team.id().to_string();
        let name = team.name.clone();
        let data = Self::to_json(&team)?;
        let sql = format!(
            "INSERT INTO teams (id, name, data) VALUES ({}, {}, {})",
            self.ph(1),
            self.ph(2),
            self.ph(3)
        );
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            &sql,
            [
                Value::String(Some(Box::new(id))),
                Value::String(Some(Box::new(name))),
                Value::String(Some(Box::new(data))),
            ],
        );
        self.db.db.execute(stmt).await.map_err(GatewayError::from)?;
        Ok(team)
    }

    async fn get(&self, id: Uuid) -> Result<Option<Team>> {
        let sql = format!("SELECT data FROM teams WHERE id = {}", self.ph(1));
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            &sql,
            [Value::String(Some(Box::new(id.to_string())))],
        );
        match self
            .db
            .db
            .query_one(stmt)
            .await
            .map_err(GatewayError::from)?
        {
            None => Ok(None),
            Some(row) => {
                let data: String = row.try_get("", "data").map_err(GatewayError::from)?;
                Ok(Some(Self::from_json(&data)?))
            }
        }
    }

    async fn get_by_name(&self, name: &str) -> Result<Option<Team>> {
        let sql = format!("SELECT data FROM teams WHERE name = {}", self.ph(1));
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            &sql,
            [Value::String(Some(Box::new(name.to_owned())))],
        );
        match self
            .db
            .db
            .query_one(stmt)
            .await
            .map_err(GatewayError::from)?
        {
            None => Ok(None),
            Some(row) => {
                let data: String = row.try_get("", "data").map_err(GatewayError::from)?;
                Ok(Some(Self::from_json(&data)?))
            }
        }
    }

    async fn update(&self, mut team: Team) -> Result<Team> {
        team.metadata.touch();
        let data = Self::to_json(&team)?;
        let name = team.name.clone();
        let id = team.id().to_string();
        let sql = format!(
            "UPDATE teams SET name = {}, data = {} WHERE id = {}",
            self.ph(1),
            self.ph(2),
            self.ph(3)
        );
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            &sql,
            [
                Value::String(Some(Box::new(name))),
                Value::String(Some(Box::new(data))),
                Value::String(Some(Box::new(id))),
            ],
        );
        self.db.db.execute(stmt).await.map_err(GatewayError::from)?;
        Ok(team)
    }

    async fn delete(&self, id: Uuid) -> Result<()> {
        let id_str = id.to_string();
        // Remove members before the team row (no DB-level FK constraint).
        let sql = format!("DELETE FROM team_members WHERE team_id = {}", self.ph(1));
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            &sql,
            [Value::String(Some(Box::new(id_str.clone())))],
        );
        self.db.db.execute(stmt).await.map_err(GatewayError::from)?;

        let sql = format!("DELETE FROM teams WHERE id = {}", self.ph(1));
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            &sql,
            [Value::String(Some(Box::new(id_str)))],
        );
        self.db.db.execute(stmt).await.map_err(GatewayError::from)?;
        Ok(())
    }

    async fn list(&self, offset: u32, limit: u32) -> Result<(Vec<Team>, u64)> {
        let count_stmt = Statement::from_string(
            self.backend(),
            "SELECT COUNT(*) as cnt FROM teams".to_owned(),
        );
        let total: u64 = self
            .db
            .db
            .query_one(count_stmt)
            .await
            .map_err(GatewayError::from)?
            .map(|r| r.try_get::<i64>("", "cnt").unwrap_or(0) as u64)
            .unwrap_or(0);

        let sql = format!(
            "SELECT data FROM teams ORDER BY created_at ASC LIMIT {} OFFSET {}",
            self.ph(1),
            self.ph(2)
        );
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            &sql,
            [
                Value::BigUnsigned(Some(limit as u64)),
                Value::BigUnsigned(Some(offset as u64)),
            ],
        );
        let rows = self
            .db
            .db
            .query_all(stmt)
            .await
            .map_err(GatewayError::from)?;
        let teams: Result<Vec<Team>> = rows
            .into_iter()
            .filter_map(|row| {
                row.try_get::<String>("", "data")
                    .ok()
                    .map(|d| Self::from_json::<Team>(&d))
            })
            .filter(|t| !matches!(t, Ok(team) if matches!(team.status, TeamStatus::Deleted)))
            .collect();
        Ok((teams?, total))
    }

    async fn count(&self) -> Result<u64> {
        let stmt = Statement::from_string(
            self.backend(),
            "SELECT COUNT(*) as cnt FROM teams".to_owned(),
        );
        Ok(self
            .db
            .db
            .query_one(stmt)
            .await
            .map_err(GatewayError::from)?
            .map(|r| r.try_get::<i64>("", "cnt").unwrap_or(0) as u64)
            .unwrap_or(0))
    }

    async fn add_member(&self, member: TeamMember) -> Result<TeamMember> {
        let team_id = member.team_id.to_string();
        let user_id = member.user_id.to_string();
        let data = Self::to_json(&member)?;
        let sql = format!(
            "INSERT INTO team_members (team_id, user_id, data) VALUES ({}, {}, {})",
            self.ph(1),
            self.ph(2),
            self.ph(3)
        );
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            &sql,
            [
                Value::String(Some(Box::new(team_id))),
                Value::String(Some(Box::new(user_id))),
                Value::String(Some(Box::new(data))),
            ],
        );
        self.db.db.execute(stmt).await.map_err(GatewayError::from)?;
        Ok(member)
    }

    async fn get_member(&self, team_id: Uuid, user_id: Uuid) -> Result<Option<TeamMember>> {
        let sql = format!(
            "SELECT data FROM team_members WHERE team_id = {} AND user_id = {}",
            self.ph(1),
            self.ph(2)
        );
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            &sql,
            [
                Value::String(Some(Box::new(team_id.to_string()))),
                Value::String(Some(Box::new(user_id.to_string()))),
            ],
        );
        match self
            .db
            .db
            .query_one(stmt)
            .await
            .map_err(GatewayError::from)?
        {
            None => Ok(None),
            Some(row) => {
                let data: String = row.try_get("", "data").map_err(GatewayError::from)?;
                Ok(Some(Self::from_json(&data)?))
            }
        }
    }

    async fn update_member_role(
        &self,
        team_id: Uuid,
        user_id: Uuid,
        role: TeamRole,
    ) -> Result<TeamMember> {
        let mut member = self.get_member(team_id, user_id).await?.ok_or_else(|| {
            GatewayError::NotFound(format!("Member {} not found in team {}", user_id, team_id))
        })?;
        member.role = role;
        member.metadata.touch();
        let data = Self::to_json(&member)?;
        let sql = format!(
            "UPDATE team_members SET data = {} WHERE team_id = {} AND user_id = {}",
            self.ph(1),
            self.ph(2),
            self.ph(3)
        );
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            &sql,
            [
                Value::String(Some(Box::new(data))),
                Value::String(Some(Box::new(team_id.to_string()))),
                Value::String(Some(Box::new(user_id.to_string()))),
            ],
        );
        self.db.db.execute(stmt).await.map_err(GatewayError::from)?;
        Ok(member)
    }

    async fn remove_member(&self, team_id: Uuid, user_id: Uuid) -> Result<()> {
        let sql = format!(
            "DELETE FROM team_members WHERE team_id = {} AND user_id = {}",
            self.ph(1),
            self.ph(2)
        );
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            &sql,
            [
                Value::String(Some(Box::new(team_id.to_string()))),
                Value::String(Some(Box::new(user_id.to_string()))),
            ],
        );
        self.db.db.execute(stmt).await.map_err(GatewayError::from)?;
        Ok(())
    }

    async fn list_members(&self, team_id: Uuid) -> Result<Vec<TeamMember>> {
        let sql = format!(
            "SELECT data FROM team_members WHERE team_id = {}",
            self.ph(1)
        );
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            &sql,
            [Value::String(Some(Box::new(team_id.to_string())))],
        );
        let rows = self
            .db
            .db
            .query_all(stmt)
            .await
            .map_err(GatewayError::from)?;
        rows.into_iter()
            .map(|row| {
                let data: String = row.try_get("", "data").map_err(GatewayError::from)?;
                Self::from_json(&data)
            })
            .collect()
    }

    async fn get_user_teams(&self, user_id: Uuid) -> Result<Vec<Team>> {
        let sql = format!(
            "SELECT team_id FROM team_members WHERE user_id = {}",
            self.ph(1)
        );
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            &sql,
            [Value::String(Some(Box::new(user_id.to_string())))],
        );
        let rows = self
            .db
            .db
            .query_all(stmt)
            .await
            .map_err(GatewayError::from)?;
        let mut teams = Vec::new();
        for row in rows {
            let tid: String = row.try_get("", "team_id").map_err(GatewayError::from)?;
            let team_id = tid
                .parse::<Uuid>()
                .map_err(|e| GatewayError::Internal(format!("invalid team uuid {}: {}", tid, e)))?;
            if let Some(team) = self.get(team_id).await? {
                teams.push(team);
            }
        }
        Ok(teams)
    }
}
