use crate::core::models::team::{MemberStatus, Team, TeamMember, TeamRole, TeamStatus};
use crate::core::models::{Metadata, UsageStats};
use crate::core::user_management::{
    Team as LegacyTeam, TeamMember as LegacyTeamMember, TeamRole as LegacyTeamRole,
    TeamSettings as LegacyTeamSettings,
};
use crate::utils::error::gateway_error::{GatewayError, Result};
use tracing::warn;
use uuid::Uuid;

use super::SeaOrmTeamRepository;

impl SeaOrmTeamRepository {
    pub(super) fn to_json<T: serde::Serialize>(v: &T) -> Result<String> {
        serde_json::to_string(v).map_err(|e| GatewayError::Internal(e.to_string()))
    }

    pub(super) fn from_json<T: serde::de::DeserializeOwned>(s: &str) -> Result<T> {
        serde_json::from_str(s).map_err(|e| GatewayError::Internal(e.to_string()))
    }

    fn invalid_legacy_uuid(kind: &str, value: &str) -> GatewayError {
        GatewayError::Internal(format!(
            "legacy user_management {} '{}' is not a valid UUID",
            kind, value
        ))
    }

    fn legacy_role_to_core(role: &LegacyTeamRole) -> TeamRole {
        match role {
            LegacyTeamRole::Owner => TeamRole::Owner,
            LegacyTeamRole::Admin => TeamRole::Admin,
            LegacyTeamRole::Member => TeamRole::Member,
            LegacyTeamRole::ReadOnly => TeamRole::Viewer,
        }
    }

    fn core_role_to_legacy(role: &TeamRole) -> LegacyTeamRole {
        match role {
            TeamRole::Owner => LegacyTeamRole::Owner,
            TeamRole::Admin | TeamRole::Manager => LegacyTeamRole::Admin,
            TeamRole::Member => LegacyTeamRole::Member,
            TeamRole::Viewer => LegacyTeamRole::ReadOnly,
        }
    }

    fn string_vec_metadata(team: &Team, key: &str) -> Vec<String> {
        team.team_metadata
            .get(key)
            .and_then(|value| value.as_array())
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn string_metadata(team: &Team, key: &str) -> Option<String> {
        team.team_metadata
            .get(key)
            .or_else(|| team.metadata.extra.get(key))
            .and_then(|value| value.as_str())
            .map(str::to_owned)
    }

    fn legacy_budget_reset_at(team: &Team) -> Option<chrono::DateTime<chrono::Utc>> {
        Self::string_metadata(team, "legacy_budget_reset_at").and_then(|value| {
            chrono::DateTime::parse_from_rfc3339(&value)
                .ok()
                .map(|timestamp| timestamp.with_timezone(&chrono::Utc))
        })
    }

    fn legacy_member_to_core(
        team_id: Uuid,
        member: &LegacyTeamMember,
    ) -> Result<Option<TeamMember>> {
        if !member.is_active {
            return Ok(None);
        }

        let user_id = match Uuid::parse_str(&member.user_id) {
            Ok(id) => id,
            Err(_) => {
                warn!(
                    legacy_user_id = %member.user_id,
                    "Skipping legacy team member because user_id is not a UUID"
                );
                return Ok(None);
            }
        };

        let mut core_member = TeamMember::new(
            team_id,
            user_id,
            Self::legacy_role_to_core(&member.role),
            None,
        );
        core_member.joined_at = member.joined_at;
        core_member.status = if member.is_active {
            MemberStatus::Active
        } else {
            MemberStatus::Left
        };
        Ok(Some(core_member))
    }

    fn core_member_to_legacy(member: &TeamMember) -> LegacyTeamMember {
        LegacyTeamMember {
            user_id: member.user_id.to_string(),
            role: Self::core_role_to_legacy(&member.role),
            joined_at: member.joined_at,
            is_active: member.is_active(),
        }
    }

    pub(super) fn legacy_team_to_core(legacy: &LegacyTeam) -> Result<(Team, Vec<TeamMember>)> {
        let team_id = Uuid::parse_str(&legacy.team_id)
            .map_err(|_| Self::invalid_legacy_uuid("team_id", &legacy.team_id))?;

        let mut metadata = Metadata::new();
        metadata.id = team_id;
        metadata.created_at = legacy.created_at;
        metadata.updated_at = legacy.created_at;
        metadata
            .extra
            .insert("legacy_um_team".to_string(), serde_json::Value::Bool(true));
        if let Some(organization_id) = &legacy.organization_id {
            metadata.extra.insert(
                "legacy_organization_id".to_string(),
                serde_json::Value::String(organization_id.clone()),
            );
        }

        let mut team = Team::new(legacy.team_name.clone(), Some(legacy.team_name.clone()));
        team.metadata = metadata;
        team.description = legacy.description.clone();
        team.status = if legacy.is_active {
            TeamStatus::Active
        } else {
            TeamStatus::Inactive
        };
        team.usage_stats = UsageStats {
            total_cost: legacy.spend,
            cost_today: legacy.spend,
            last_reset: legacy.created_at,
            ..UsageStats::default()
        };
        team.team_metadata = legacy
            .metadata
            .iter()
            .map(|(key, value)| (key.clone(), serde_json::Value::String(value.clone())))
            .collect();
        team.team_metadata.insert(
            "legacy_permissions".to_string(),
            serde_json::json!(legacy.permissions),
        );
        team.team_metadata.insert(
            "legacy_models".to_string(),
            serde_json::json!(legacy.models),
        );
        if let Some(max_budget) = legacy.max_budget {
            team.team_metadata.insert(
                "legacy_max_budget".to_string(),
                serde_json::json!(max_budget),
            );
        }
        if let Some(duration) = &legacy.budget_duration {
            team.team_metadata.insert(
                "legacy_budget_duration".to_string(),
                serde_json::Value::String(duration.clone()),
            );
        }
        if let Some(reset_at) = legacy.budget_reset_at {
            team.team_metadata.insert(
                "legacy_budget_reset_at".to_string(),
                serde_json::Value::String(reset_at.to_rfc3339()),
            );
        }

        let members = legacy
            .members
            .iter()
            .filter_map(
                |member| match Self::legacy_member_to_core(team_id, member) {
                    Ok(member) => member,
                    Err(err) => {
                        warn!(error = %err, "Skipping invalid legacy team member");
                        None
                    }
                },
            )
            .collect();

        Ok((team, members))
    }

    pub(super) fn core_team_to_legacy(team: &Team, members: Vec<TeamMember>) -> LegacyTeam {
        let metadata = team
            .team_metadata
            .iter()
            .filter_map(|(key, value)| value.as_str().map(|value| (key.clone(), value.to_owned())))
            .collect();

        LegacyTeam {
            team_id: team.id().to_string(),
            team_name: team.name.clone(),
            description: team.description.clone(),
            organization_id: Self::string_metadata(team, "legacy_organization_id"),
            members: members.iter().map(Self::core_member_to_legacy).collect(),
            permissions: Self::string_vec_metadata(team, "legacy_permissions"),
            models: Self::string_vec_metadata(team, "legacy_models"),
            max_budget: team
                .team_metadata
                .get("legacy_max_budget")
                .and_then(|value| value.as_f64()),
            spend: team.usage_stats.total_cost,
            budget_duration: Self::string_metadata(team, "legacy_budget_duration"),
            budget_reset_at: Self::legacy_budget_reset_at(team),
            metadata,
            is_active: team.is_active(),
            created_at: team.metadata.created_at,
            settings: LegacyTeamSettings::default(),
        }
    }
}
