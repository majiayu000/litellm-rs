//! SeaORM-backed TeamRepository implementation
//!
//! Stores `core::models::team::{Team, TeamMember}` as JSON snapshots in the
//! `teams` and `team_members` tables created by migration
//! `m20240301_000002_create_teams_table`.  Works with both SQLite and
//! PostgreSQL backends via the live `SeaOrmDatabase` connection.

mod canonical;
mod conversions;
mod legacy_sync;
mod repository_impl;

use sea_orm::DbBackend;
use std::sync::Arc;
use uuid::Uuid;

use crate::core::models::team::Team;
use crate::utils::error::gateway_error::Result;

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

    /// Read a team's lifecycle for authentication without migrating legacy data.
    ///
    /// Management's `TeamRepository::get` may insert a canonical team from a
    /// legacy snapshot. Authentication must not do that: a concurrent delete
    /// could commit after the snapshot was read but before that insert, reviving
    /// the deleted principal. Canonical state remains authoritative when present.
    pub(crate) async fn get_for_authentication(&self, id: Uuid) -> Result<Option<Team>> {
        if let Some(team) = self.get_canonical(id).await? {
            return Ok(Some(team));
        }
        match self.get_legacy_um_team(id).await? {
            Some(legacy) => Self::legacy_team_to_core(&legacy).map(|(team, _)| Some(team)),
            None => Ok(None),
        }
    }

    pub(super) fn backend(&self) -> DbBackend {
        match self.db.backend_type {
            DatabaseBackendType::PostgreSQL => DbBackend::Postgres,
            DatabaseBackendType::SQLite => DbBackend::Sqlite,
        }
    }

    /// Return the positional placeholder for parameter `n` (1-based).
    pub(super) fn ph(&self, n: usize) -> String {
        match self.db.backend_type {
            DatabaseBackendType::PostgreSQL => format!("${}", n),
            DatabaseBackendType::SQLite => "?".to_string(),
        }
    }
}
