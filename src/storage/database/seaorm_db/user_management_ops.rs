//! User management database operations
//!
//! Stub implementations — database schema for user management is not yet migrated.
//! Each method returns `GatewayError::NotImplemented` until the migration lands.

use crate::core::user_management::{Organization, Team, User};
use crate::utils::error::gateway_error::{GatewayError, Result};

use super::types::SeaOrmDatabase;

impl SeaOrmDatabase {
    /// Retrieve a user management user by their string ID.
    pub async fn get_user(&self, _user_id: &str) -> Result<Option<User>> {
        Err(GatewayError::not_implemented(
            "user_management: get_user not yet implemented",
        ))
    }

    /// Retrieve a user management user by their email address.
    pub async fn get_user_by_email(&self, _email: &str) -> Result<Option<User>> {
        Err(GatewayError::not_implemented(
            "user_management: get_user_by_email not yet implemented",
        ))
    }

    /// Persist a new user management user to the database.
    ///
    /// Named `um_create_user` to avoid colliding with the existing
    /// `create_user` method which operates on a different `User` type.
    pub async fn um_create_user(&self, _user: &User) -> Result<()> {
        Err(GatewayError::not_implemented(
            "user_management: um_create_user not yet implemented",
        ))
    }

    /// Persist all mutable fields of a user management user (full update).
    pub async fn update_user(&self, _user: &User) -> Result<()> {
        Err(GatewayError::not_implemented(
            "user_management: update_user not yet implemented",
        ))
    }

    /// Remove a user management user from the database by their string ID.
    pub async fn delete_user(&self, _user_id: &str) -> Result<()> {
        Err(GatewayError::not_implemented(
            "user_management: delete_user not yet implemented",
        ))
    }

    /// Add `cost` to the recorded spend for the given user ID.
    pub async fn update_user_spend(&self, _user_id: &str, _cost: f64) -> Result<()> {
        Err(GatewayError::not_implemented(
            "user_management: update_user_spend not yet implemented",
        ))
    }

    /// List user management users with offset-based pagination.
    pub async fn list_users(&self, _offset: u32, _limit: u32) -> Result<Vec<User>> {
        Err(GatewayError::not_implemented(
            "user_management: list_users not yet implemented",
        ))
    }

    /// Retrieve a team by its string ID.
    pub async fn get_team(&self, _team_id: &str) -> Result<Option<Team>> {
        Err(GatewayError::not_implemented(
            "user_management: get_team not yet implemented",
        ))
    }

    /// Persist a new team to the database.
    pub async fn create_team(&self, _team: &Team) -> Result<()> {
        Err(GatewayError::not_implemented(
            "user_management: create_team not yet implemented",
        ))
    }

    /// Persist all mutable fields of a team (full update).
    pub async fn update_team(&self, _team: &Team) -> Result<()> {
        Err(GatewayError::not_implemented(
            "user_management: update_team not yet implemented",
        ))
    }

    /// Add `cost` to the recorded spend for the given team ID.
    pub async fn update_team_spend(&self, _team_id: &str, _cost: f64) -> Result<()> {
        Err(GatewayError::not_implemented(
            "user_management: update_team_spend not yet implemented",
        ))
    }

    /// List teams with offset-based pagination.
    pub async fn list_teams(&self, _offset: u32, _limit: u32) -> Result<Vec<Team>> {
        Err(GatewayError::not_implemented(
            "user_management: list_teams not yet implemented",
        ))
    }

    /// Persist a new organization to the database.
    pub async fn create_organization(&self, _organization: &Organization) -> Result<()> {
        Err(GatewayError::not_implemented(
            "user_management: create_organization not yet implemented",
        ))
    }

    /// Retrieve an organization by its string ID.
    pub async fn get_organization(&self, _organization_id: &str) -> Result<Option<Organization>> {
        Err(GatewayError::not_implemented(
            "user_management: get_organization not yet implemented",
        ))
    }
}
