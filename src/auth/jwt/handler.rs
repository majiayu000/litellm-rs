//! Core JWT handler implementation

use super::types::{
    AccessTokenClaims, Claims, JwtHandler, TeamScopeMarker, TokenPair, TokenType,
    VerifiedActiveTeam,
};
use crate::config::models::auth::AuthConfig;
use crate::utils::error::gateway_error::{GatewayError, Result};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};
use uuid::Uuid;

impl JwtHandler {
    /// Create a new JWT handler
    pub async fn new(config: &AuthConfig) -> Result<Self> {
        let secret = config.jwt_secret.as_bytes();

        Ok(Self {
            encoding_key: EncodingKey::from_secret(secret),
            decoding_key: DecodingKey::from_secret(secret),
            algorithm: Algorithm::HS256,
            expiration: config.jwt_expiration,
            issuer: "litellm-rs".to_string(),
        })
    }

    /// Return the configured access-token lifetime in seconds.
    pub fn expiration(&self) -> u64 {
        self.expiration
    }

    /// Create an access token for a user
    pub async fn create_access_token(
        &self,
        user_id: Uuid,
        role: String,
        permissions: Vec<String>,
        team_id: Option<Uuid>,
        session_id: Option<Uuid>,
    ) -> Result<String> {
        if team_id.is_some() {
            return Err(GatewayError::bad_request(
                "Active team selection requires verified membership",
            ));
        }

        self.encode_access_token(user_id, role, permissions, None, session_id)
            .await
    }

    /// Create an access token bound to a server-verified active team.
    pub(crate) async fn create_access_token_for_verified_team(
        &self,
        user_id: Uuid,
        role: String,
        permissions: Vec<String>,
        verified_team: &VerifiedActiveTeam,
        session_id: Option<Uuid>,
    ) -> Result<String> {
        if !verified_team.matches_user(user_id) {
            return Err(GatewayError::auth(
                "Verified team proof does not match token subject",
            ));
        }

        self.encode_access_token(
            user_id,
            role,
            permissions,
            Some(verified_team.team_id()),
            session_id,
        )
        .await
    }

    async fn encode_access_token(
        &self,
        user_id: Uuid,
        role: String,
        permissions: Vec<String>,
        team_id: Option<Uuid>,
        session_id: Option<Uuid>,
    ) -> Result<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| GatewayError::internal(format!("System time error: {}", e)))?
            .as_secs();

        let claims = AccessTokenClaims {
            public: Claims {
                sub: user_id,
                iat: now,
                exp: now + self.expiration,
                iss: self.issuer.clone(),
                aud: "api".to_string(),
                jti: Uuid::new_v4().to_string(),
                role,
                permissions,
                team_id,
                session_id: session_id.map(|id| id.to_string()),
                token_type: TokenType::Access,
            },
            team_scope_version: TeamScopeMarker::Present(1),
        };

        let header = Header::new(self.algorithm);
        let token = encode(&header, &claims, &self.encoding_key).map_err(GatewayError::from)?;

        debug!("Created access token for user: {}", user_id);
        Ok(token)
    }

    /// Create a refresh token for a user
    pub async fn create_refresh_token(
        &self,
        user_id: Uuid,
        session_id: Option<String>,
    ) -> Result<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| GatewayError::internal(format!("System time error: {}", e)))?
            .as_secs();

        let claims = Claims {
            sub: user_id,
            iat: now,
            exp: now + (self.expiration * 24), // Refresh tokens last 24x longer
            iss: self.issuer.clone(),
            aud: "refresh".to_string(),
            jti: Uuid::new_v4().to_string(),
            role: "".to_string(), // No role in refresh token
            permissions: vec![],  // No permissions in refresh token
            team_id: None,
            session_id,
            token_type: TokenType::Refresh,
        };

        let header = Header::new(self.algorithm);
        let token = encode(&header, &claims, &self.encoding_key).map_err(GatewayError::from)?;

        debug!("Created refresh token for user: {}", user_id);
        Ok(token)
    }

    /// Create a token pair (access + refresh)
    pub async fn create_token_pair(
        &self,
        user_id: Uuid,
        role: String,
        permissions: Vec<String>,
        team_id: Option<Uuid>,
        session_id: Option<Uuid>,
    ) -> Result<TokenPair> {
        let access_token = self
            .create_access_token(user_id, role, permissions, team_id, session_id)
            .await?;

        let refresh_token = self
            .create_refresh_token(user_id, session_id.map(|id| id.to_string()))
            .await?;

        Ok(TokenPair {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.expiration,
        })
    }

    /// Create a token pair whose access token is bound to a verified active team.
    pub(crate) async fn create_token_pair_for_verified_team(
        &self,
        user_id: Uuid,
        role: String,
        permissions: Vec<String>,
        verified_team: &VerifiedActiveTeam,
        session_id: Option<Uuid>,
    ) -> Result<TokenPair> {
        let access_token = self
            .create_access_token_for_verified_team(
                user_id,
                role,
                permissions,
                verified_team,
                session_id,
            )
            .await?;
        let refresh_token = self
            .create_refresh_token(user_id, session_id.map(|id| id.to_string()))
            .await?;

        Ok(TokenPair {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.expiration,
        })
    }

    /// Verify and decode an access token
    ///
    /// Only accepts tokens with the "api" audience claim, preventing
    /// refresh tokens from being used as access tokens.
    pub async fn verify_access_token(&self, token: &str) -> Result<Claims> {
        Ok(self
            .verify_access_token_with_provenance(token)
            .await?
            .public)
    }

    /// Verify an access token while preserving the team-claim provenance marker.
    pub(crate) async fn verify_access_token_with_provenance(
        &self,
        token: &str,
    ) -> Result<AccessTokenClaims> {
        let mut validation = Validation::new(self.algorithm);
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&["api"]);

        let token_data = decode::<AccessTokenClaims>(token, &self.decoding_key, &validation)
            .map_err(|e| {
                warn!("JWT verification failed: {}", e);
                GatewayError::Auth(format!("JWT error: {}", e))
            })?;

        if let TeamScopeMarker::Present(version) = token_data.claims.team_scope_version
            && version != 1
        {
            return Err(GatewayError::auth(
                "Unsupported access-token team scope version",
            ));
        }

        debug!(
            "Access token verified for user: {}",
            token_data.claims.public.sub
        );
        Ok(token_data.claims)
    }

    /// Verify a refresh token and return user ID
    ///
    /// Only accepts tokens with the "refresh" audience claim, preventing
    /// access tokens from being used as refresh tokens.
    pub async fn verify_refresh_token(&self, token: &str) -> Result<Uuid> {
        let mut validation = Validation::new(self.algorithm);
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&["refresh"]);

        let token_data = decode::<Claims>(token, &self.decoding_key, &validation).map_err(|e| {
            warn!("JWT verification failed: {}", e);
            GatewayError::Auth(format!("JWT error: {}", e))
        })?;

        if !matches!(token_data.claims.token_type, TokenType::Refresh) {
            return Err(GatewayError::auth("Invalid token type for refresh"));
        }

        debug!("Refresh token verified for user: {}", token_data.claims.sub);
        Ok(token_data.claims.sub)
    }
}
