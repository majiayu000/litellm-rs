//! Core authentication system implementation

use super::types::{AuthMethod, AuthResult, AuthzResult};
use crate::config::models::auth::AuthConfig;
use crate::core::models::user::types::{User, UserRole};
use crate::core::types::context::RequestContext;
use crate::storage::StorageLayer;
use crate::utils::error::gateway_error::Result;
use std::sync::Arc;
use tracing::{debug, info};

/// Main authentication system
#[derive(Clone)]
pub struct AuthSystem {
    /// Authentication configuration
    pub(super) config: Arc<AuthConfig>,
    /// Storage layer for user data
    pub(super) storage: Arc<StorageLayer>,
    /// JWT handler
    pub(super) jwt: Arc<crate::auth::jwt::types::JwtHandler>,
    /// API key handler
    pub(super) api_key: Arc<crate::auth::api_key::creation::ApiKeyHandler>,
    /// RBAC system
    pub(super) rbac: Arc<crate::auth::rbac::RbacSystem>,
}

impl AuthSystem {
    /// Create a new authentication system
    pub async fn new(config: &AuthConfig, storage: Arc<StorageLayer>) -> Result<Self> {
        info!("Initializing authentication system");

        let config = Arc::new(config.clone());

        // Initialize JWT handler
        let jwt = Arc::new(crate::auth::jwt::types::JwtHandler::new(&config).await?);

        // Initialize API key handler
        let api_key =
            Arc::new(crate::auth::api_key::creation::ApiKeyHandler::new(storage.clone()).await?);

        // Initialize RBAC system
        let rbac = Arc::new(crate::auth::rbac::RbacSystem::new(&config.rbac).await?);

        info!("Authentication system initialized successfully");

        Ok(Self {
            config,
            storage,
            jwt,
            api_key,
            rbac,
        })
    }

    /// Authenticate a request
    pub async fn authenticate(
        &self,
        auth_method: AuthMethod,
        context: RequestContext,
    ) -> Result<AuthResult> {
        debug!("Authenticating request: {:?}", auth_method);

        match auth_method {
            AuthMethod::Jwt(token) => self.authenticate_jwt(&token, context).await,
            AuthMethod::ApiKey(key) => self.authenticate_api_key(&key, context).await,
            AuthMethod::Session(session_id) => {
                self.authenticate_session(&session_id, context).await
            }
            AuthMethod::None => Ok(AuthResult {
                success: false,
                user: None,
                api_key: None,
                session: None,
                error: Some("No authentication provided".to_string()),
                context,
            }),
        }
    }

    /// Authenticate using JWT token
    async fn authenticate_jwt(
        &self,
        token: &str,
        mut context: RequestContext,
    ) -> Result<AuthResult> {
        match self.jwt.verify_token(token).await {
            Ok(claims) => {
                // Get user from database
                if let Ok(Some(user)) = self.storage.db().find_user_by_id(claims.sub).await {
                    if user.is_active() {
                        context.user_id = Some(user.id().to_string());
                        if let Some(team_id) = user.team_ids.first().copied() {
                            context.set_team_id(team_id);
                        } else {
                            context.clear_team_id();
                        }

                        Ok(AuthResult {
                            success: true,
                            user: Some(user),
                            api_key: None,
                            session: None,
                            error: None,
                            context,
                        })
                    } else {
                        Ok(AuthResult {
                            success: false,
                            user: None,
                            api_key: None,
                            session: None,
                            error: Some("User account is not active".to_string()),
                            context,
                        })
                    }
                } else {
                    Ok(AuthResult {
                        success: false,
                        user: None,
                        api_key: None,
                        session: None,
                        error: Some("User not found".to_string()),
                        context,
                    })
                }
            }
            Err(e) => Ok(AuthResult {
                success: false,
                user: None,
                api_key: None,
                session: None,
                error: Some(format!("Invalid JWT token: {}", e)),
                context,
            }),
        }
    }

    /// Authenticate using API key
    async fn authenticate_api_key(
        &self,
        key: &str,
        mut context: RequestContext,
    ) -> Result<AuthResult> {
        match self.api_key.verify_key(key).await {
            Ok(Some((api_key, user))) => {
                context.set_api_key_id(api_key.metadata.id);
                context.user_id = api_key.user_id.map(|id| id.to_string());
                if let Some(team_id) = api_key.team_id {
                    context.set_team_id(team_id);
                } else {
                    context.clear_team_id();
                }

                Ok(AuthResult {
                    success: true,
                    user,
                    api_key: Some(api_key),
                    session: None,
                    error: None,
                    context,
                })
            }
            Ok(None) => Ok(AuthResult {
                success: false,
                user: None,
                api_key: None,
                session: None,
                error: Some("Invalid API key".to_string()),
                context,
            }),
            Err(e) => Ok(AuthResult {
                success: false,
                user: None,
                api_key: None,
                session: None,
                error: Some(format!("API key verification failed: {}", e)),
                context,
            }),
        }
    }

    /// Authenticate using session
    async fn authenticate_session(
        &self,
        session_id: &str,
        mut context: RequestContext,
    ) -> Result<AuthResult> {
        // TODO: Implement session verification
        match self.jwt.verify_token(session_id).await {
            Ok(claims) => {
                // Try to find user by ID from claims
                match self.storage.db().find_user_by_id(claims.sub).await {
                    Ok(Some(user)) => {
                        context.user_id = Some(user.id().to_string());
                        if let Some(team_id) = user.team_ids.first().copied() {
                            context.set_team_id(team_id);
                        } else {
                            context.clear_team_id();
                        }

                        Ok(AuthResult {
                            success: true,
                            user: Some(user),
                            api_key: None,
                            session: None, // TODO: Create proper session object
                            error: None,
                            context,
                        })
                    }
                    Ok(None) => Ok(AuthResult {
                        success: false,
                        user: None,
                        api_key: None,
                        session: None,
                        error: Some("User not found".to_string()),
                        context,
                    }),
                    Err(e) => Ok(AuthResult {
                        success: false,
                        user: None,
                        api_key: None,
                        session: None,
                        error: Some(format!("User lookup failed: {}", e)),
                        context,
                    }),
                }
            }
            Err(e) => Ok(AuthResult {
                success: false,
                user: None,
                api_key: None,
                session: None,
                error: Some(format!("Session verification failed: {}", e)),
                context,
            }),
        }
    }

    /// Authorize a user for specific permissions
    pub async fn authorize(&self, user: &User, permissions: &[String]) -> Result<AuthzResult> {
        debug!(
            "Authorizing user {} for permissions: {:?}",
            user.username, permissions
        );

        let user_permissions = self.rbac.get_user_permissions(user).await?;
        let allowed = self.rbac.check_permissions(&user_permissions, permissions);

        Ok(AuthzResult {
            allowed,
            required_permissions: permissions.to_vec(),
            user_permissions,
            reason: if !allowed {
                Some("Insufficient permissions".to_string())
            } else {
                None
            },
        })
    }

    /// Get user permissions based on role
    pub(super) async fn get_user_permissions(&self, user: &User) -> Result<Vec<String>> {
        let permissions = match user.role {
            UserRole::Admin => vec![
                "read:all".to_string(),
                "write:all".to_string(),
                "delete:all".to_string(),
                "manage:users".to_string(),
                "manage:api_keys".to_string(),
                "manage:teams".to_string(),
            ],
            UserRole::SuperAdmin => vec![
                "read:all".to_string(),
                "write:all".to_string(),
                "delete:all".to_string(),
                "manage:users".to_string(),
                "manage:api_keys".to_string(),
                "manage:teams".to_string(),
                "manage:system".to_string(),
            ],
            UserRole::Manager => vec![
                "read:team".to_string(),
                "write:team".to_string(),
                "manage:team_users".to_string(),
                "manage:team_api_keys".to_string(),
            ],
            UserRole::User => vec![
                "read:own".to_string(),
                "write:own".to_string(),
                "use:api".to_string(),
            ],
            UserRole::ApiUser => vec!["use:api".to_string()],
            UserRole::Viewer => vec!["read:own".to_string()],
        };
        Ok(permissions)
    }

    /// Get authentication configuration
    pub fn config(&self) -> &AuthConfig {
        &self.config
    }

    /// Get JWT handler
    pub fn jwt(&self) -> &crate::auth::jwt::types::JwtHandler {
        &self.jwt
    }

    /// Get API key handler
    pub fn api_key(&self) -> &crate::auth::api_key::creation::ApiKeyHandler {
        &self.api_key
    }

    /// Get RBAC system
    pub fn rbac(&self) -> &crate::auth::rbac::RbacSystem {
        &self.rbac
    }
}
