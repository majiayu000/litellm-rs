//! JWT types and data structures

use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

/// JWT handler for token operations
#[derive(Clone)]
pub struct JwtHandler {
    /// Encoding key for signing tokens
    pub(super) encoding_key: EncodingKey,
    /// Decoding key for verifying tokens
    pub(super) decoding_key: DecodingKey,
    /// JWT algorithm
    pub(super) algorithm: Algorithm,
    /// Token expiration time in seconds
    pub(super) expiration: u64,
    /// Token issuer
    pub(super) issuer: String,
}

impl std::fmt::Debug for JwtHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtHandler")
            .field("algorithm", &self.algorithm)
            .field("expiration", &self.expiration)
            .field("issuer", &self.issuer)
            .field("encoding_key", &"[REDACTED]")
            .field("decoding_key", &"[REDACTED]")
            .finish()
    }
}

/// JWT claims structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: Uuid,
    /// Issued at timestamp
    pub iat: u64,
    /// Expiration timestamp
    pub exp: u64,
    /// Issuer
    pub iss: String,
    /// Audience
    pub aud: String,
    /// JWT ID
    pub jti: String,
    /// User role
    pub role: String,
    /// User permissions
    pub permissions: Vec<String>,
    /// Team ID (optional)
    pub team_id: Option<Uuid>,
    /// Session ID (optional)
    pub session_id: Option<String>,
    /// Token type
    pub token_type: TokenType,
}

/// Proof that a user's membership and team are both active in canonical storage.
///
/// The type is nameable inside the crate so auth flows can preserve it across
/// policy stages. Only the authentication module can construct one.
#[derive(Debug, Clone)]
pub(crate) struct VerifiedActiveTeam {
    user_id: Uuid,
    team_id: Uuid,
}

impl VerifiedActiveTeam {
    pub(in crate::auth) fn new(
        user_id: Uuid,
        team_id: Uuid,
        _seal: crate::auth::system::TeamValidationSeal,
    ) -> Self {
        Self { user_id, team_id }
    }

    pub(crate) fn matches_user(&self, user_id: Uuid) -> bool {
        self.user_id == user_id
    }

    pub(crate) fn team_id(&self) -> Uuid {
        self.team_id
    }
}

/// Presence-aware provenance for the active-team claim carried by access tokens.
///
/// This stays outside [`Claims`] so the public claims struct remains source-compatible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum TeamScopeMarker {
    #[default]
    Absent,
    Present(u8),
}

impl TeamScopeMarker {
    pub(crate) fn is_absent(&self) -> bool {
        matches!(self, Self::Absent)
    }
}

fn serialize_team_scope_marker<S>(
    marker: &TeamScopeMarker,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match marker {
        TeamScopeMarker::Present(version) => serializer.serialize_u8(*version),
        TeamScopeMarker::Absent => serializer.serialize_unit(),
    }
}

fn deserialize_team_scope_marker<'de, D>(
    deserializer: D,
) -> std::result::Result<TeamScopeMarker, D::Error>
where
    D: Deserializer<'de>,
{
    u8::deserialize(deserializer).map(TeamScopeMarker::Present)
}

/// Internal access-token envelope. Only a physically absent marker is legacy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AccessTokenClaims {
    #[serde(flatten)]
    pub(crate) public: Claims,
    #[serde(
        default,
        skip_serializing_if = "TeamScopeMarker::is_absent",
        serialize_with = "serialize_team_scope_marker",
        deserialize_with = "deserialize_team_scope_marker"
    )]
    pub(crate) team_scope_version: TeamScopeMarker,
}

/// Token type enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenType {
    /// Access token for API access
    Access,
    /// Refresh token for obtaining new access tokens
    Refresh,
    /// Password reset token
    PasswordReset,
    /// Email verification token
    EmailVerification,
    /// Invitation token
    Invitation,
}

/// Token pair (access + refresh)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPair {
    /// Access token
    pub access_token: String,
    /// Refresh token
    pub refresh_token: String,
    /// Token type (always "Bearer")
    pub token_type: String,
    /// Expires in seconds
    pub expires_in: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== TokenType Tests ====================

    #[test]
    fn test_token_type_access() {
        let token_type = TokenType::Access;
        let json = serde_json::to_string(&token_type).unwrap();
        assert!(json.contains("access"));
    }

    #[test]
    fn test_token_type_refresh() {
        let token_type = TokenType::Refresh;
        let json = serde_json::to_string(&token_type).unwrap();
        assert!(json.contains("refresh"));
    }

    #[test]
    fn test_token_type_password_reset() {
        let token_type = TokenType::PasswordReset;
        let json = serde_json::to_string(&token_type).unwrap();
        assert!(json.contains("password_reset"));
    }

    #[test]
    fn test_token_type_email_verification() {
        let token_type = TokenType::EmailVerification;
        let json = serde_json::to_string(&token_type).unwrap();
        assert!(json.contains("email_verification"));
    }

    #[test]
    fn test_token_type_invitation() {
        let token_type = TokenType::Invitation;
        let json = serde_json::to_string(&token_type).unwrap();
        assert!(json.contains("invitation"));
    }

    #[test]
    fn test_token_type_all_variants_serialization() {
        let variants = vec![
            TokenType::Access,
            TokenType::Refresh,
            TokenType::PasswordReset,
            TokenType::EmailVerification,
            TokenType::Invitation,
        ];

        for token_type in variants {
            let json = serde_json::to_string(&token_type).unwrap();
            let parsed: TokenType = serde_json::from_str(&json).unwrap();
            // Compare debug strings since TokenType doesn't derive PartialEq
            assert_eq!(format!("{:?}", token_type), format!("{:?}", parsed));
        }
    }

    #[test]
    fn test_token_type_clone() {
        let original = TokenType::Access;
        let cloned = original.clone();
        assert_eq!(format!("{:?}", original), format!("{:?}", cloned));
    }

    #[test]
    fn test_token_type_debug() {
        let token_type = TokenType::Access;
        let debug_str = format!("{:?}", token_type);
        assert!(debug_str.contains("Access"));
    }

    // ==================== Claims Tests ====================

    #[test]
    fn test_claims_creation() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = Claims {
            sub: Uuid::new_v4(),
            iat: now,
            exp: now + 3600,
            iss: "gateway".to_string(),
            aud: "api".to_string(),
            jti: Uuid::new_v4().to_string(),
            role: "user".to_string(),
            permissions: vec!["read".to_string(), "write".to_string()],
            team_id: Some(Uuid::new_v4()),
            session_id: Some("session_123".to_string()),
            token_type: TokenType::Access,
        };

        assert!(!claims.sub.is_nil());
        assert_eq!(claims.iss, "gateway");
        assert_eq!(claims.permissions.len(), 2);
    }

    #[test]
    fn test_claims_serialization() {
        let user_id = Uuid::new_v4();
        let claims = Claims {
            sub: user_id,
            iat: 1700000000,
            exp: 1700003600,
            iss: "test_issuer".to_string(),
            aud: "test_audience".to_string(),
            jti: "unique_token_id".to_string(),
            role: "admin".to_string(),
            permissions: vec!["all".to_string()],
            team_id: None,
            session_id: None,
            token_type: TokenType::Access,
        };

        let json = serde_json::to_string(&claims).unwrap();
        assert!(json.contains(&user_id.to_string()));
        assert!(json.contains("test_issuer"));
        assert!(json.contains("admin"));

        let parsed: Claims = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.sub, user_id);
        assert_eq!(parsed.role, "admin");
    }

    #[test]
    fn test_claims_with_optional_fields() {
        let claims = Claims {
            sub: Uuid::new_v4(),
            iat: 1700000000,
            exp: 1700003600,
            iss: "gateway".to_string(),
            aud: "api".to_string(),
            jti: Uuid::new_v4().to_string(),
            role: "user".to_string(),
            permissions: vec![],
            team_id: None,
            session_id: None,
            token_type: TokenType::Access,
        };

        let json = serde_json::to_string(&claims).unwrap();
        let parsed: Claims = serde_json::from_str(&json).unwrap();

        assert!(parsed.team_id.is_none());
        assert!(parsed.session_id.is_none());
    }

    #[test]
    fn test_claims_with_all_fields() {
        let user_id = Uuid::new_v4();
        let team_id = Uuid::new_v4();

        let claims = Claims {
            sub: user_id,
            iat: 1700000000,
            exp: 1700003600,
            iss: "prod-gateway".to_string(),
            aud: "prod-api".to_string(),
            jti: "jwt_abc123".to_string(),
            role: "team_lead".to_string(),
            permissions: vec!["read".to_string(), "write".to_string(), "admin".to_string()],
            team_id: Some(team_id),
            session_id: Some("sess_xyz789".to_string()),
            token_type: TokenType::Access,
        };

        let json = serde_json::to_string(&claims).unwrap();
        let parsed: Claims = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.sub, user_id);
        assert_eq!(parsed.team_id, Some(team_id));
        assert_eq!(parsed.session_id, Some("sess_xyz789".to_string()));
        assert_eq!(parsed.permissions.len(), 3);
    }

    #[test]
    fn test_claims_clone() {
        let claims = Claims {
            sub: Uuid::new_v4(),
            iat: 1700000000,
            exp: 1700003600,
            iss: "gateway".to_string(),
            aud: "api".to_string(),
            jti: "token_id".to_string(),
            role: "user".to_string(),
            permissions: vec!["read".to_string()],
            team_id: None,
            session_id: None,
            token_type: TokenType::Access,
        };

        let cloned = claims.clone();
        assert_eq!(cloned.sub, claims.sub);
        assert_eq!(cloned.role, claims.role);
        assert_eq!(cloned.permissions, claims.permissions);
    }

    #[test]
    fn test_claims_expiration() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = Claims {
            sub: Uuid::new_v4(),
            iat: now,
            exp: now + 3600, // 1 hour from now
            iss: "gateway".to_string(),
            aud: "api".to_string(),
            jti: Uuid::new_v4().to_string(),
            role: "user".to_string(),
            permissions: vec![],
            team_id: None,
            session_id: None,
            token_type: TokenType::Access,
        };

        // Token should not be expired
        assert!(claims.exp > now);
        // Token was issued at or before now
        assert!(claims.iat <= now);
    }

    #[test]
    fn test_claims_different_token_types() {
        let base = Claims {
            sub: Uuid::new_v4(),
            iat: 1700000000,
            exp: 1700003600,
            iss: "gateway".to_string(),
            aud: "api".to_string(),
            jti: "token".to_string(),
            role: "user".to_string(),
            permissions: vec![],
            team_id: None,
            session_id: None,
            token_type: TokenType::Access,
        };

        let mut refresh_claims = base.clone();
        refresh_claims.token_type = TokenType::Refresh;

        let mut reset_claims = base.clone();
        reset_claims.token_type = TokenType::PasswordReset;

        // Serialize and verify token types
        let access_json = serde_json::to_string(&base).unwrap();
        let refresh_json = serde_json::to_string(&refresh_claims).unwrap();
        let reset_json = serde_json::to_string(&reset_claims).unwrap();

        assert!(access_json.contains("access"));
        assert!(refresh_json.contains("refresh"));
        assert!(reset_json.contains("password_reset"));
    }

    // ==================== TokenPair Tests ====================

    #[test]
    fn test_token_pair_creation() {
        let pair = TokenPair {
            access_token: "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...".to_string(),
            refresh_token: "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...refresh".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: 3600,
        };

        assert!(pair.access_token.starts_with("eyJ"));
        assert!(pair.refresh_token.contains("refresh"));
        assert_eq!(pair.token_type, "Bearer");
        assert_eq!(pair.expires_in, 3600);
    }

    #[test]
    fn test_token_pair_serialization() {
        let pair = TokenPair {
            access_token: "access_token_value".to_string(),
            refresh_token: "refresh_token_value".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: 7200,
        };

        let json = serde_json::to_string(&pair).unwrap();
        assert!(json.contains("access_token_value"));
        assert!(json.contains("refresh_token_value"));
        assert!(json.contains("Bearer"));
        assert!(json.contains("7200"));

        let parsed: TokenPair = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.access_token, "access_token_value");
        assert_eq!(parsed.expires_in, 7200);
    }

    #[test]
    fn test_token_pair_clone() {
        let pair = TokenPair {
            access_token: "access".to_string(),
            refresh_token: "refresh".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: 3600,
        };

        let cloned = pair.clone();
        assert_eq!(cloned.access_token, pair.access_token);
        assert_eq!(cloned.refresh_token, pair.refresh_token);
        assert_eq!(cloned.expires_in, pair.expires_in);
    }

    #[test]
    fn test_token_pair_debug() {
        let pair = TokenPair {
            access_token: "token123".to_string(),
            refresh_token: "refresh456".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: 3600,
        };

        let debug_str = format!("{:?}", pair);
        assert!(debug_str.contains("TokenPair"));
        assert!(debug_str.contains("token123"));
        assert!(debug_str.contains("Bearer"));
    }

    #[test]
    fn test_token_pair_different_expiration() {
        let short_lived = TokenPair {
            access_token: "short".to_string(),
            refresh_token: "refresh".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: 900, // 15 minutes
        };

        let long_lived = TokenPair {
            access_token: "long".to_string(),
            refresh_token: "refresh".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: 86400, // 24 hours
        };

        assert!(short_lived.expires_in < long_lived.expires_in);
    }

    // ==================== JwtHandler Tests ====================

    #[test]
    fn test_jwt_handler_debug_redacts_keys() {
        let handler = JwtHandler {
            encoding_key: EncodingKey::from_secret(b"secret"),
            decoding_key: DecodingKey::from_secret(b"secret"),
            algorithm: Algorithm::HS256,
            expiration: 3600,
            issuer: "test_issuer".to_string(),
        };

        let debug_str = format!("{:?}", handler);
        assert!(debug_str.contains("JwtHandler"));
        assert!(debug_str.contains("[REDACTED]"));
        assert!(debug_str.contains("HS256"));
        assert!(debug_str.contains("3600"));
        assert!(debug_str.contains("test_issuer"));
        // Should NOT contain actual secret
        assert!(!debug_str.contains("secret"));
    }

    #[test]
    fn test_jwt_handler_clone() {
        let handler = JwtHandler {
            encoding_key: EncodingKey::from_secret(b"secret"),
            decoding_key: DecodingKey::from_secret(b"secret"),
            algorithm: Algorithm::HS256,
            expiration: 3600,
            issuer: "gateway".to_string(),
        };

        let cloned = handler.clone();
        assert_eq!(cloned.expiration, handler.expiration);
        assert_eq!(cloned.issuer, handler.issuer);
    }

    #[test]
    fn test_jwt_handler_different_algorithms() {
        let hs256_handler = JwtHandler {
            encoding_key: EncodingKey::from_secret(b"secret"),
            decoding_key: DecodingKey::from_secret(b"secret"),
            algorithm: Algorithm::HS256,
            expiration: 3600,
            issuer: "test".to_string(),
        };

        let hs384_handler = JwtHandler {
            encoding_key: EncodingKey::from_secret(b"secret"),
            decoding_key: DecodingKey::from_secret(b"secret"),
            algorithm: Algorithm::HS384,
            expiration: 3600,
            issuer: "test".to_string(),
        };

        let hs512_handler = JwtHandler {
            encoding_key: EncodingKey::from_secret(b"secret"),
            decoding_key: DecodingKey::from_secret(b"secret"),
            algorithm: Algorithm::HS512,
            expiration: 3600,
            issuer: "test".to_string(),
        };

        assert_eq!(format!("{:?}", hs256_handler.algorithm), "HS256");
        assert_eq!(format!("{:?}", hs384_handler.algorithm), "HS384");
        assert_eq!(format!("{:?}", hs512_handler.algorithm), "HS512");
    }

    #[test]
    fn test_jwt_handler_various_expirations() {
        let short_expiry = JwtHandler {
            encoding_key: EncodingKey::from_secret(b"secret"),
            decoding_key: DecodingKey::from_secret(b"secret"),
            algorithm: Algorithm::HS256,
            expiration: 300, // 5 minutes
            issuer: "test".to_string(),
        };

        let long_expiry = JwtHandler {
            encoding_key: EncodingKey::from_secret(b"secret"),
            decoding_key: DecodingKey::from_secret(b"secret"),
            algorithm: Algorithm::HS256,
            expiration: 604800, // 1 week
            issuer: "test".to_string(),
        };

        assert_eq!(short_expiry.expiration, 300);
        assert_eq!(long_expiry.expiration, 604800);
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_claims_for_access_token() {
        let user_id = Uuid::new_v4();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = Claims {
            sub: user_id,
            iat: now,
            exp: now + 3600,
            iss: "gateway".to_string(),
            aud: "api".to_string(),
            jti: Uuid::new_v4().to_string(),
            role: "user".to_string(),
            permissions: vec!["api:read".to_string(), "api:write".to_string()],
            team_id: Some(Uuid::new_v4()),
            session_id: Some("sess_001".to_string()),
            token_type: TokenType::Access,
        };

        // Verify access token characteristics
        let json = serde_json::to_string(&claims).unwrap();
        assert!(json.contains("access"));
        assert!(json.contains("api:read"));
    }

    #[test]
    fn test_claims_for_refresh_token() {
        let user_id = Uuid::new_v4();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = Claims {
            sub: user_id,
            iat: now,
            exp: now + 604800, // 1 week for refresh tokens
            iss: "gateway".to_string(),
            aud: "refresh".to_string(),
            jti: Uuid::new_v4().to_string(),
            role: "user".to_string(),
            permissions: vec![], // Refresh tokens typically have minimal permissions
            team_id: None,
            session_id: Some("sess_001".to_string()),
            token_type: TokenType::Refresh,
        };

        // Verify refresh token has longer expiration
        assert!(claims.exp - claims.iat > 3600);
        assert!(claims.permissions.is_empty());
    }

    #[test]
    fn test_full_authentication_flow() {
        let user_id = Uuid::new_v4();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Create access claims
        let access_claims = Claims {
            sub: user_id,
            iat: now,
            exp: now + 3600,
            iss: "gateway".to_string(),
            aud: "api".to_string(),
            jti: Uuid::new_v4().to_string(),
            role: "user".to_string(),
            permissions: vec!["read".to_string()],
            team_id: None,
            session_id: Some("session".to_string()),
            token_type: TokenType::Access,
        };

        // Create refresh claims
        let refresh_claims = Claims {
            sub: user_id,
            iat: now,
            exp: now + 604800,
            iss: "gateway".to_string(),
            aud: "refresh".to_string(),
            jti: Uuid::new_v4().to_string(),
            role: "user".to_string(),
            permissions: vec![],
            team_id: None,
            session_id: Some("session".to_string()),
            token_type: TokenType::Refresh,
        };

        // Create token pair
        let token_pair = TokenPair {
            access_token: serde_json::to_string(&access_claims).unwrap(),
            refresh_token: serde_json::to_string(&refresh_claims).unwrap(),
            token_type: "Bearer".to_string(),
            expires_in: 3600,
        };

        assert_eq!(token_pair.token_type, "Bearer");
        assert!(!token_pair.access_token.is_empty());
        assert!(!token_pair.refresh_token.is_empty());
    }
}
