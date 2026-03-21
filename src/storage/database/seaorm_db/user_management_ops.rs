//! User management database operations
//!
//! Stores user-management domain objects as JSON snapshots in the
//! `um_users`, `um_teams`, and `um_organizations` tables created by
//! migration `m20240301_000001_create_user_management_tables`.

use crate::core::user_management::{Organization, Team, User};
use crate::utils::error::gateway_error::{GatewayError, Result};
use sea_orm::{ConnectionTrait, DbBackend, Statement, Value};
use tracing::debug;

use super::types::{DatabaseBackendType, SeaOrmDatabase};

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

impl SeaOrmDatabase {
    /// Return the sea_orm DbBackend matching the live connection.
    fn db_backend(&self) -> DbBackend {
        match self.backend_type {
            DatabaseBackendType::PostgreSQL => DbBackend::Postgres,
            DatabaseBackendType::SQLite => DbBackend::Sqlite,
        }
    }

    /// Return the positional placeholder for parameter `n` (1-based).
    ///
    /// SQLite uses `?`; PostgreSQL uses `$N`.
    fn ph(&self, n: usize) -> String {
        match self.backend_type {
            DatabaseBackendType::PostgreSQL => format!("${}", n),
            DatabaseBackendType::SQLite => "?".to_string(),
        }
    }

    fn deserialize<T: serde::de::DeserializeOwned>(data: &str) -> Result<T> {
        serde_json::from_str(data).map_err(|e| GatewayError::Internal(e.to_string()))
    }

    fn serialize<T: serde::Serialize>(value: &T) -> Result<String> {
        serde_json::to_string(value).map_err(|e| GatewayError::Internal(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// User operations
// ---------------------------------------------------------------------------

impl SeaOrmDatabase {
    /// Retrieve a user management user by their string ID.
    pub async fn get_user(&self, user_id: &str) -> Result<Option<User>> {
        debug!("um: get_user {}", user_id);
        let sql = format!("SELECT data FROM um_users WHERE user_id = {}", self.ph(1));
        let stmt = Statement::from_sql_and_values(
            self.db_backend(),
            &sql,
            [Value::String(Some(Box::new(user_id.to_owned())))],
        );
        match self.db.query_one(stmt).await.map_err(GatewayError::from)? {
            None => Ok(None),
            Some(row) => {
                let data: String = row.try_get("", "data").map_err(GatewayError::from)?;
                Ok(Some(Self::deserialize(&data)?))
            }
        }
    }

    /// Retrieve a user management user by their email address.
    pub async fn get_user_by_email(&self, email: &str) -> Result<Option<User>> {
        debug!("um: get_user_by_email {}", email);
        let sql = format!("SELECT data FROM um_users WHERE email = {}", self.ph(1));
        let stmt = Statement::from_sql_and_values(
            self.db_backend(),
            &sql,
            [Value::String(Some(Box::new(email.to_owned())))],
        );
        match self.db.query_one(stmt).await.map_err(GatewayError::from)? {
            None => Ok(None),
            Some(row) => {
                let data: String = row.try_get("", "data").map_err(GatewayError::from)?;
                Ok(Some(Self::deserialize(&data)?))
            }
        }
    }

    /// Persist a new user management user to the database.
    ///
    /// Named `um_create_user` to avoid colliding with the existing
    /// `create_user` method which operates on a different `User` type.
    pub async fn um_create_user(&self, user: &User) -> Result<()> {
        debug!("um: um_create_user {}", user.user_id);
        let data = Self::serialize(user)?;
        let sql = format!(
            "INSERT INTO um_users (user_id, email, data, spend) VALUES ({}, {}, {}, {})",
            self.ph(1),
            self.ph(2),
            self.ph(3),
            self.ph(4),
        );
        let stmt = Statement::from_sql_and_values(
            self.db_backend(),
            &sql,
            [
                Value::String(Some(Box::new(user.user_id.clone()))),
                Value::String(Some(Box::new(user.email.clone()))),
                Value::String(Some(Box::new(data))),
                Value::Double(Some(user.spend)),
            ],
        );
        self.db.execute(stmt).await.map_err(GatewayError::from)?;
        Ok(())
    }

    /// Persist all mutable fields of a user management user (full update).
    pub async fn update_user(&self, user: &User) -> Result<()> {
        debug!("um: update_user {}", user.user_id);
        let data = Self::serialize(user)?;
        let sql = format!(
            "UPDATE um_users SET email = {}, data = {}, spend = {} WHERE user_id = {}",
            self.ph(1),
            self.ph(2),
            self.ph(3),
            self.ph(4),
        );
        let stmt = Statement::from_sql_and_values(
            self.db_backend(),
            &sql,
            [
                Value::String(Some(Box::new(user.email.clone()))),
                Value::String(Some(Box::new(data))),
                Value::Double(Some(user.spend)),
                Value::String(Some(Box::new(user.user_id.clone()))),
            ],
        );
        self.db.execute(stmt).await.map_err(GatewayError::from)?;
        Ok(())
    }

    /// Remove a user management user from the database by their string ID.
    pub async fn delete_user(&self, user_id: &str) -> Result<()> {
        debug!("um: delete_user {}", user_id);
        let sql = format!("DELETE FROM um_users WHERE user_id = {}", self.ph(1));
        let stmt = Statement::from_sql_and_values(
            self.db_backend(),
            &sql,
            [Value::String(Some(Box::new(user_id.to_owned())))],
        );
        self.db.execute(stmt).await.map_err(GatewayError::from)?;
        Ok(())
    }

    /// Add `cost` to the recorded spend for the given user ID.
    pub async fn update_user_spend(&self, user_id: &str, cost: f64) -> Result<()> {
        debug!("um: update_user_spend {} += {}", user_id, cost);
        let sql = format!(
            "UPDATE um_users SET spend = spend + {} WHERE user_id = {}",
            self.ph(1),
            self.ph(2),
        );
        let stmt = Statement::from_sql_and_values(
            self.db_backend(),
            &sql,
            [
                Value::Double(Some(cost)),
                Value::String(Some(Box::new(user_id.to_owned()))),
            ],
        );
        self.db.execute(stmt).await.map_err(GatewayError::from)?;
        Ok(())
    }

    /// List user management users with offset-based pagination.
    pub async fn list_users(&self, offset: u32, limit: u32) -> Result<Vec<User>> {
        debug!("um: list_users offset={} limit={}", offset, limit);
        let sql = format!(
            "SELECT data FROM um_users ORDER BY created_at ASC LIMIT {} OFFSET {}",
            self.ph(1),
            self.ph(2),
        );
        let stmt = Statement::from_sql_and_values(
            self.db_backend(),
            &sql,
            [
                Value::BigUnsigned(Some(limit as u64)),
                Value::BigUnsigned(Some(offset as u64)),
            ],
        );
        let rows = self.db.query_all(stmt).await.map_err(GatewayError::from)?;
        rows.into_iter()
            .map(|row| {
                let data: String = row.try_get("", "data").map_err(GatewayError::from)?;
                Self::deserialize(&data)
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Team operations (user_management::Team — distinct from core::models::Team)
// ---------------------------------------------------------------------------

impl SeaOrmDatabase {
    /// Retrieve a team by its string ID.
    pub async fn get_team(&self, team_id: &str) -> Result<Option<Team>> {
        debug!("um: get_team {}", team_id);
        let sql = format!("SELECT data FROM um_teams WHERE team_id = {}", self.ph(1));
        let stmt = Statement::from_sql_and_values(
            self.db_backend(),
            &sql,
            [Value::String(Some(Box::new(team_id.to_owned())))],
        );
        match self.db.query_one(stmt).await.map_err(GatewayError::from)? {
            None => Ok(None),
            Some(row) => {
                let data: String = row.try_get("", "data").map_err(GatewayError::from)?;
                Ok(Some(Self::deserialize(&data)?))
            }
        }
    }

    /// Persist a new team to the database.
    pub async fn create_team(&self, team: &Team) -> Result<()> {
        debug!("um: create_team {}", team.team_id);
        let data = Self::serialize(team)?;
        let sql = format!(
            "INSERT INTO um_teams (team_id, data, spend) VALUES ({}, {}, {})",
            self.ph(1),
            self.ph(2),
            self.ph(3),
        );
        let stmt = Statement::from_sql_and_values(
            self.db_backend(),
            &sql,
            [
                Value::String(Some(Box::new(team.team_id.clone()))),
                Value::String(Some(Box::new(data))),
                Value::Double(Some(team.spend)),
            ],
        );
        self.db.execute(stmt).await.map_err(GatewayError::from)?;
        Ok(())
    }

    /// Persist all mutable fields of a team (full update).
    pub async fn update_team(&self, team: &Team) -> Result<()> {
        debug!("um: update_team {}", team.team_id);
        let data = Self::serialize(team)?;
        let sql = format!(
            "UPDATE um_teams SET data = {}, spend = {} WHERE team_id = {}",
            self.ph(1),
            self.ph(2),
            self.ph(3),
        );
        let stmt = Statement::from_sql_and_values(
            self.db_backend(),
            &sql,
            [
                Value::String(Some(Box::new(data))),
                Value::Double(Some(team.spend)),
                Value::String(Some(Box::new(team.team_id.clone()))),
            ],
        );
        self.db.execute(stmt).await.map_err(GatewayError::from)?;
        Ok(())
    }

    /// Add `cost` to the recorded spend for the given team ID.
    pub async fn update_team_spend(&self, team_id: &str, cost: f64) -> Result<()> {
        debug!("um: update_team_spend {} += {}", team_id, cost);
        let sql = format!(
            "UPDATE um_teams SET spend = spend + {} WHERE team_id = {}",
            self.ph(1),
            self.ph(2),
        );
        let stmt = Statement::from_sql_and_values(
            self.db_backend(),
            &sql,
            [
                Value::Double(Some(cost)),
                Value::String(Some(Box::new(team_id.to_owned()))),
            ],
        );
        self.db.execute(stmt).await.map_err(GatewayError::from)?;
        Ok(())
    }

    /// List teams with offset-based pagination.
    pub async fn list_teams(&self, offset: u32, limit: u32) -> Result<Vec<Team>> {
        debug!("um: list_teams offset={} limit={}", offset, limit);
        let sql = format!(
            "SELECT data FROM um_teams ORDER BY created_at ASC LIMIT {} OFFSET {}",
            self.ph(1),
            self.ph(2),
        );
        let stmt = Statement::from_sql_and_values(
            self.db_backend(),
            &sql,
            [
                Value::BigUnsigned(Some(limit as u64)),
                Value::BigUnsigned(Some(offset as u64)),
            ],
        );
        let rows = self.db.query_all(stmt).await.map_err(GatewayError::from)?;
        rows.into_iter()
            .map(|row| {
                let data: String = row.try_get("", "data").map_err(GatewayError::from)?;
                Self::deserialize(&data)
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Organization operations
// ---------------------------------------------------------------------------

impl SeaOrmDatabase {
    /// Persist a new organization to the database.
    pub async fn create_organization(&self, organization: &Organization) -> Result<()> {
        debug!("um: create_organization {}", organization.organization_id);
        let data = Self::serialize(organization)?;
        let sql = format!(
            "INSERT INTO um_organizations (organization_id, data, spend) VALUES ({}, {}, {})",
            self.ph(1),
            self.ph(2),
            self.ph(3),
        );
        let stmt = Statement::from_sql_and_values(
            self.db_backend(),
            &sql,
            [
                Value::String(Some(Box::new(organization.organization_id.clone()))),
                Value::String(Some(Box::new(data))),
                Value::Double(Some(organization.spend)),
            ],
        );
        self.db.execute(stmt).await.map_err(GatewayError::from)?;
        Ok(())
    }

    /// Retrieve an organization by its string ID.
    pub async fn get_organization(&self, organization_id: &str) -> Result<Option<Organization>> {
        debug!("um: get_organization {}", organization_id);
        let sql = format!(
            "SELECT data FROM um_organizations WHERE organization_id = {}",
            self.ph(1)
        );
        let stmt = Statement::from_sql_and_values(
            self.db_backend(),
            &sql,
            [Value::String(Some(Box::new(organization_id.to_owned())))],
        );
        match self.db.query_one(stmt).await.map_err(GatewayError::from)? {
            None => Ok(None),
            Some(row) => {
                let data: String = row.try_get("", "data").map_err(GatewayError::from)?;
                Ok(Some(Self::deserialize(&data)?))
            }
        }
    }
}
