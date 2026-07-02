use crate::core::models::team::{Team, TeamMember, TeamRole};
use crate::core::teams::repository::TeamRepository;
use crate::utils::error::gateway_error::{GatewayError, Result};
use async_trait::async_trait;
use sea_orm::{ConnectionTrait, Statement, TransactionTrait, Value};
use std::collections::HashSet;
use tracing::warn;
use uuid::Uuid;

use super::SeaOrmTeamRepository;

#[async_trait]
impl TeamRepository for SeaOrmTeamRepository {
    async fn create(&self, team: Team) -> Result<Team> {
        self.insert_canonical_team(&team).await?;
        self.sync_legacy_team_from_canonical(team.id()).await?;
        Ok(team)
    }

    async fn get(&self, id: Uuid) -> Result<Option<Team>> {
        if let Some(team) = self.get_canonical(id).await? {
            return Ok(Some(team));
        }

        match self.get_legacy_um_team(id).await? {
            Some(legacy) => self.persist_legacy_team(&legacy).await,
            None => Ok(None),
        }
    }

    async fn get_by_name(&self, name: &str) -> Result<Option<Team>> {
        if let Some(team) = self.get_canonical_by_name(name).await? {
            return Ok(Some(team));
        }

        for legacy in self.list_legacy_um_teams().await? {
            if legacy.team_name == name {
                return self.persist_legacy_team(&legacy).await;
            }
        }
        Ok(None)
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
        self.sync_legacy_team_from_canonical(team.id()).await?;
        Ok(team)
    }

    async fn delete(&self, id: Uuid) -> Result<()> {
        let mut member_user_ids: HashSet<Uuid> = self
            .list_canonical_members(id)
            .await?
            .into_iter()
            .map(|member| member.user_id)
            .collect();
        if let Some(legacy) = self.get_legacy_um_team(id).await? {
            for member in legacy.members {
                match Uuid::parse_str(&member.user_id) {
                    Ok(user_id) => {
                        member_user_ids.insert(user_id);
                    }
                    Err(_) => warn!(
                        legacy_user_id = %member.user_id,
                        team_id = %id,
                        "Skipping legacy user membership cleanup because user_id is not a UUID"
                    ),
                }
            }
        }
        let id_str = id.to_string();
        let txn = self.db.db.begin().await.map_err(GatewayError::from)?;

        // Remove members before the team row (no DB-level FK constraint).
        let sql = format!("DELETE FROM team_members WHERE team_id = {}", self.ph(1));
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            &sql,
            [Value::String(Some(Box::new(id_str.clone())))],
        );
        txn.execute(stmt).await.map_err(GatewayError::from)?;

        let sql = format!("DELETE FROM teams WHERE id = {}", self.ph(1));
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            &sql,
            [Value::String(Some(Box::new(id_str)))],
        );
        txn.execute(stmt).await.map_err(GatewayError::from)?;

        let sql = format!("DELETE FROM um_teams WHERE team_id = {}", self.ph(1));
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            &sql,
            [Value::String(Some(Box::new(id.to_string())))],
        );
        txn.execute(stmt).await.map_err(GatewayError::from)?;

        txn.commit().await.map_err(GatewayError::from)?;

        for user_id in member_user_ids {
            self.remove_legacy_user_team(user_id, id).await?;
        }

        Ok(())
    }

    async fn list(&self, offset: u32, limit: u32) -> Result<(Vec<Team>, u64)> {
        self.sync_legacy_um_teams().await?;

        let sql = format!(
            "SELECT data FROM teams WHERE {} ORDER BY created_at ASC LIMIT {} OFFSET {}",
            self.non_deleted_team_predicate(),
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
            .map(|row| {
                let data: String = row.try_get("", "data").map_err(GatewayError::from)?;
                Self::from_json::<Team>(&data)
            })
            .collect();
        let count_sql = format!(
            "SELECT COUNT(*) as cnt FROM teams WHERE {}",
            self.non_deleted_team_predicate()
        );
        let count_stmt = Statement::from_string(self.backend(), count_sql);
        let total = self
            .db
            .db
            .query_one(count_stmt)
            .await
            .map_err(GatewayError::from)?
            .map(|row| row.try_get::<i64>("", "cnt").unwrap_or(0) as u64)
            .unwrap_or(0);

        Ok((teams?, total))
    }

    async fn count(&self) -> Result<u64> {
        self.sync_legacy_um_teams().await?;

        let sql = format!(
            "SELECT COUNT(*) as cnt FROM teams WHERE {}",
            self.non_deleted_team_predicate()
        );
        let stmt = Statement::from_string(self.backend(), sql);
        Ok(self
            .db
            .db
            .query_one(stmt)
            .await
            .map_err(GatewayError::from)?
            .map(|row| row.try_get::<i64>("", "cnt").unwrap_or(0) as u64)
            .unwrap_or(0))
    }

    async fn add_member(&self, member: TeamMember) -> Result<TeamMember> {
        self.insert_canonical_member(&member).await?;
        self.sync_legacy_team_from_canonical(member.team_id).await?;
        self.add_legacy_user_team(member.user_id, member.team_id)
            .await?;
        Ok(member)
    }

    async fn get_member(&self, team_id: Uuid, user_id: Uuid) -> Result<Option<TeamMember>> {
        if let Some(legacy) = self.get_legacy_um_team(team_id).await? {
            let _ = self.persist_legacy_team(&legacy).await?;
        }

        self.get_canonical_member(team_id, user_id).await
    }

    async fn update_member_role(
        &self,
        team_id: Uuid,
        user_id: Uuid,
        role: TeamRole,
    ) -> Result<TeamMember> {
        let txn = self.db.db.begin().await.map_err(GatewayError::from)?;

        // Read the member inside the transaction to prevent TOCTOU
        let read_sql = format!(
            "SELECT data FROM team_members WHERE team_id = {} AND user_id = {}",
            self.ph(1),
            self.ph(2)
        );
        let read_stmt = Statement::from_sql_and_values(
            self.backend(),
            &read_sql,
            [
                Value::String(Some(Box::new(team_id.to_string()))),
                Value::String(Some(Box::new(user_id.to_string()))),
            ],
        );
        let row = txn
            .query_one(read_stmt)
            .await
            .map_err(GatewayError::from)?
            .ok_or_else(|| {
                GatewayError::NotFound(format!("Member {} not found in team {}", user_id, team_id))
            })?;
        let raw_data: String = row.try_get("", "data").map_err(GatewayError::from)?;
        let mut member: TeamMember = Self::from_json(&raw_data)?;

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
        txn.execute(stmt).await.map_err(GatewayError::from)?;

        txn.commit().await.map_err(GatewayError::from)?;
        self.sync_legacy_team_from_canonical(team_id).await?;
        Ok(member)
    }

    async fn remove_member(&self, team_id: Uuid, user_id: Uuid) -> Result<()> {
        self.delete_canonical_member(team_id, user_id).await?;
        self.sync_legacy_team_from_canonical(team_id).await?;
        self.remove_legacy_user_team(user_id, team_id).await?;
        Ok(())
    }

    async fn list_members(&self, team_id: Uuid) -> Result<Vec<TeamMember>> {
        if let Some(legacy) = self.get_legacy_um_team(team_id).await? {
            let _ = self.persist_legacy_team(&legacy).await?;
        }

        self.list_canonical_members(team_id).await
    }

    async fn get_user_teams(&self, user_id: Uuid) -> Result<Vec<Team>> {
        self.sync_legacy_um_teams().await?;

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
