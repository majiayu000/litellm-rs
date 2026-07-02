use crate::core::models::team::{Team, TeamMember};
use crate::utils::error::gateway_error::{GatewayError, Result};
use sea_orm::{ConnectionTrait, Statement, Value};
use uuid::Uuid;

use super::SeaOrmTeamRepository;
use crate::storage::database::seaorm_db::types::DatabaseBackendType;

impl SeaOrmTeamRepository {
    /// SQL predicate for filtering logically deleted teams from JSON payload.
    pub(super) fn non_deleted_team_predicate(&self) -> &'static str {
        match self.db.backend_type {
            DatabaseBackendType::PostgreSQL => {
                "((data::jsonb ->> 'status') IS NULL OR (data::jsonb ->> 'status') <> 'deleted')"
            }
            DatabaseBackendType::SQLite => {
                "(json_extract(data, '$.status') IS NULL OR json_extract(data, '$.status') <> 'deleted')"
            }
        }
    }

    pub(super) async fn insert_canonical_team(&self, team: &Team) -> Result<()> {
        let id = team.id().to_string();
        let name = team.name.clone();
        let data = Self::to_json(team)?;
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
        Ok(())
    }

    pub(super) async fn insert_canonical_member(&self, member: &TeamMember) -> Result<()> {
        let team_id = member.team_id.to_string();
        let user_id = member.user_id.to_string();
        let data = Self::to_json(member)?;
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
        Ok(())
    }

    pub(super) async fn get_canonical(&self, id: Uuid) -> Result<Option<Team>> {
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

    pub(super) async fn get_canonical_by_name(&self, name: &str) -> Result<Option<Team>> {
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

    pub(super) async fn get_canonical_member(
        &self,
        team_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<TeamMember>> {
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

    pub(super) async fn list_canonical_members(&self, team_id: Uuid) -> Result<Vec<TeamMember>> {
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

    pub(super) async fn delete_canonical_member(&self, team_id: Uuid, user_id: Uuid) -> Result<()> {
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
}
