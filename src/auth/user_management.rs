//! User management operations

use super::system::AuthSystem;
use crate::core::models::user::types::User;
use crate::utils::auth::crypto::password::{hash_password, verify_password};
use crate::utils::error::gateway_error::{GatewayError, Result};
use tracing::info;

impl AuthSystem {
    /// Create a new user
    pub async fn create_user(
        &self,
        username: String,
        email: String,
        password: String,
    ) -> Result<User> {
        info!("Creating new user: {}", username);

        // Hash password
        let password_hash = hash_password(&password)?;

        // Create user
        let user = User::new(username, email, password_hash);

        // Store in database
        self.storage.db().create_user(&user).await
    }

    /// Login user and create session
    pub async fn login(&self, username: &str, password: &str) -> Result<(User, String)> {
        let user = self.verify_login_credentials(username, password).await?;
        self.finish_login(user, None).await
    }

    /// Login with an explicit, server-verified active-team selection.
    pub(crate) async fn login_with_active_team(
        &self,
        username: &str,
        password: &str,
        team_id: uuid::Uuid,
    ) -> Result<(User, String)> {
        let user = self.verify_login_credentials(username, password).await?;
        let verified_team = self
            .validate_active_team(user.id(), team_id)
            .await?
            .ok_or_else(|| GatewayError::bad_request("Invalid active team selection"))?;
        self.finish_login(user, Some(verified_team)).await
    }

    async fn verify_login_credentials(&self, username: &str, password: &str) -> Result<User> {
        info!("User login attempt: {}", username);

        // Find user
        let user = self
            .storage
            .db()
            .find_user_by_username(username)
            .await?
            .ok_or_else(|| GatewayError::auth("Invalid username or password"))?;

        // Verify password
        if !verify_password(password, &user.password_hash)? {
            return Err(GatewayError::auth("Invalid username or password"));
        }

        // Check if user is active
        if !user.is_active() {
            return Err(GatewayError::auth("Account is not active"));
        }

        Ok(user)
    }

    async fn finish_login(
        &self,
        user: User,
        verified_team: Option<crate::auth::jwt::types::VerifiedActiveTeam>,
    ) -> Result<(User, String)> {
        // Create session
        let session_id = uuid::Uuid::new_v4();
        let permissions = self.get_user_permissions(&user).await?;
        let session_token = match verified_team {
            Some(ref verified_team) => {
                self.jwt
                    .create_access_token_for_verified_team(
                        user.id(),
                        format!("{:?}", user.role),
                        permissions,
                        verified_team,
                        Some(session_id),
                    )
                    .await?
            }
            None => {
                self.jwt
                    .create_access_token(
                        user.id(),
                        format!("{:?}", user.role),
                        permissions,
                        None,
                        Some(session_id),
                    )
                    .await?
            }
        };

        // Update last login
        self.storage.db().update_user_last_login(user.id()).await?;

        info!("User logged in successfully: {}", user.username);
        Ok((user, session_token))
    }

    /// Logout user
    pub async fn logout(&self, session_token: &str) -> Result<()> {
        info!("User logout");

        // Extract session ID from token
        if let Ok(claims) = self.jwt.verify_access_token(session_token).await
            && claims.session_id.is_some()
        {
            // Store invalidated session (in practice, you'd use Redis or similar)
            // For now, just log the session invalidation
            info!("Session invalidated");
        }

        Ok(())
    }
}
