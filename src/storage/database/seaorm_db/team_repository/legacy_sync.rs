use crate::core::models::team::{Team, TeamMember};
use crate::core::user_management::Team as LegacyTeam;
use crate::utils::error::gateway_error::{GatewayError, Result};
use sea_orm::{ConnectionTrait, Statement, Value};
use std::collections::HashSet;
use tracing::warn;
use uuid::Uuid;

use super::SeaOrmTeamRepository;

impl SeaOrmTeamRepository {
    pub(super) async fn get_legacy_um_team(&self, id: Uuid) -> Result<Option<LegacyTeam>> {
        let sql = format!("SELECT data FROM um_teams WHERE team_id = {}", self.ph(1));
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

    pub(super) async fn list_legacy_um_teams(&self) -> Result<Vec<LegacyTeam>> {
        let sql = "SELECT data FROM um_teams ORDER BY created_at ASC";
        let stmt = Statement::from_string(self.backend(), sql);
        let rows = self
            .db
            .db
            .query_all(stmt)
            .await
            .map_err(GatewayError::from)?;

        let mut teams = Vec::with_capacity(rows.len());
        for row in rows {
            let data: String = row.try_get("", "data").map_err(GatewayError::from)?;
            match Self::from_json::<LegacyTeam>(&data) {
                Ok(team) => teams.push(team),
                Err(err) => warn!(error = %err, "Skipping invalid legacy um_teams row"),
            }
        }
        Ok(teams)
    }

    async fn ensure_legacy_team_inserted(&self, team: &Team) -> Result<bool> {
        if self.get_canonical(team.id()).await?.is_some() {
            return Ok(true);
        }

        if let Some(existing) = self.get_canonical_by_name(&team.name).await? {
            warn!(
                legacy_team_id = %team.id(),
                existing_team_id = %existing.id(),
                team_name = %team.name,
                "Skipping legacy team sync because canonical team name already exists"
            );
            return Ok(false);
        }

        match self.insert_canonical_team(team).await {
            Ok(()) => Ok(true),
            Err(err) => {
                if self.get_canonical(team.id()).await?.is_some() {
                    return Ok(true);
                }

                if let Some(existing) = self.get_canonical_by_name(&team.name).await? {
                    warn!(
                        legacy_team_id = %team.id(),
                        existing_team_id = %existing.id(),
                        team_name = %team.name,
                        "Skipping legacy team sync after concurrent canonical name insert"
                    );
                    return Ok(false);
                }

                Err(err)
            }
        }
    }

    async fn ensure_legacy_member_inserted(&self, member: &TeamMember) -> Result<()> {
        if self
            .get_canonical_member(member.team_id, member.user_id)
            .await?
            .is_some()
        {
            return Ok(());
        }

        match self.insert_canonical_member(member).await {
            Ok(()) => Ok(()),
            Err(err) => {
                if self
                    .get_canonical_member(member.team_id, member.user_id)
                    .await?
                    .is_some()
                {
                    Ok(())
                } else {
                    Err(err)
                }
            }
        }
    }

    async fn sync_legacy_members(&self, team_id: Uuid, members: Vec<TeamMember>) -> Result<()> {
        let legacy_user_ids: HashSet<Uuid> = members.iter().map(|member| member.user_id).collect();

        for member in &members {
            self.ensure_legacy_member_inserted(member).await?;
        }

        for existing in self.list_canonical_members(team_id).await? {
            if !legacy_user_ids.contains(&existing.user_id) {
                self.delete_canonical_member(team_id, existing.user_id)
                    .await?;
            }
        }

        Ok(())
    }

    pub(super) async fn persist_legacy_team(&self, legacy: &LegacyTeam) -> Result<Option<Team>> {
        let (team, members) = match Self::legacy_team_to_core(legacy) {
            Ok(converted) => converted,
            Err(err) => {
                warn!(error = %err, legacy_team_id = %legacy.team_id, "Skipping legacy team sync");
                return Ok(None);
            }
        };

        if !self.ensure_legacy_team_inserted(&team).await? {
            return Ok(None);
        }

        self.sync_legacy_members(team.id(), members).await?;
        Ok(Some(team))
    }

    pub(super) async fn sync_legacy_um_teams(&self) -> Result<()> {
        for legacy in self.list_legacy_um_teams().await? {
            let _ = self.persist_legacy_team(&legacy).await?;
        }
        Ok(())
    }

    pub(super) async fn sync_legacy_team_from_canonical(&self, team_id: Uuid) -> Result<()> {
        let Some(team) = self.get_canonical(team_id).await? else {
            return Ok(());
        };

        let members = self.list_canonical_members(team_id).await?;
        let mut legacy = Self::core_team_to_legacy(&team, members);
        if let Some(existing) = self.db.get_team(&legacy.team_id).await? {
            if legacy.organization_id.is_none() {
                legacy.organization_id = existing.organization_id;
            }
            legacy.settings = existing.settings;
            self.db.update_team(&legacy).await
        } else {
            self.db.create_team(&legacy).await
        }
    }

    pub(super) async fn add_legacy_user_team(&self, user_id: Uuid, team_id: Uuid) -> Result<()> {
        let user_id = user_id.to_string();
        let team_id = team_id.to_string();
        if let Some(mut user) = self.db.get_user(&user_id).await?
            && !user.teams.contains(&team_id)
        {
            user.teams.push(team_id);
            self.db.update_user(&user).await?;
        }
        Ok(())
    }

    pub(super) async fn remove_legacy_user_team(&self, user_id: Uuid, team_id: Uuid) -> Result<()> {
        let user_id = user_id.to_string();
        let team_id = team_id.to_string();
        if let Some(mut user) = self.db.get_user(&user_id).await? {
            let original_len = user.teams.len();
            user.teams.retain(|existing| existing != &team_id);
            if user.teams.len() != original_len {
                self.db.update_user(&user).await?;
            }
        }
        Ok(())
    }
}
