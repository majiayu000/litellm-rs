//! User login endpoint

use crate::server::middleware::AuthRateLimiter;
use crate::server::routes::ApiResponse;
use crate::server::state::AppState;
use crate::utils::auth::crypto::password::verify_password;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{error, info, warn};

use super::models::{
    AuthFlowFailure, AuthorizationStage, EncodingStage, LoginRequest, LoginResponse,
    LoginWireRequest, PrimaryAuthStage, SequencedAuthFlow, TeamSelectionStage, UserInfo,
    execute_auth_flow,
};

type LoginFlowDriver<P, T, A, E> = SequencedAuthFlow<P, T, A, E>;

struct LoginFlowOutput {
    user: crate::core::models::user::types::User,
    access_token: String,
    refresh_token: String,
}

#[async_trait(?Send)]
trait LoginUserLookup {
    async fn find_user(
        &mut self,
        username: &str,
    ) -> crate::utils::error::gateway_error::Result<Option<crate::core::models::user::types::User>>;
}

struct DatabaseLoginUserLookup<'a>(&'a crate::storage::database::Database);

#[async_trait(?Send)]
impl LoginUserLookup for DatabaseLoginUserLookup<'_> {
    async fn find_user(
        &mut self,
        username: &str,
    ) -> crate::utils::error::gateway_error::Result<Option<crate::core::models::user::types::User>>
    {
        self.0.find_user_by_username(username).await
    }
}

trait LoginPasswordVerifier {
    fn verify(
        &mut self,
        password: &str,
        password_hash: &str,
    ) -> crate::utils::error::gateway_error::Result<bool>;
}

struct ProductionPasswordVerifier;

impl LoginPasswordVerifier for ProductionPasswordVerifier {
    fn verify(
        &mut self,
        password: &str,
        password_hash: &str,
    ) -> crate::utils::error::gateway_error::Result<bool> {
        verify_password(password, password_hash)
    }
}

struct LoginPrimaryStage<'a, U, P> {
    users: U,
    password_verifier: P,
    request: LoginRequest,
    client_ip: &'a str,
    limiter: &'a AuthRateLimiter,
}

#[async_trait(?Send)]
impl<U: LoginUserLookup, P: LoginPasswordVerifier> PrimaryAuthStage
    for LoginPrimaryStage<'_, U, P>
{
    type Principal = crate::core::models::user::types::User;

    async fn authenticate(&mut self) -> Result<Self::Principal, AuthFlowFailure> {
        let user = match self.users.find_user(&self.request.username).await {
            Ok(Some(user)) => user,
            Ok(None) => {
                warn!(
                    "Login attempt with invalid username from IP {}",
                    self.client_ip
                );
                self.limiter.record_failure(self.client_ip);
                return Err(AuthFlowFailure::unauthorized("Invalid credentials"));
            }
            Err(error) => {
                error!("Database error during login: {}", error);
                return Err(AuthFlowFailure::internal("Database error"));
            }
        };

        if !user.is_active() {
            warn!("Login attempt for inactive user from IP {}", self.client_ip);
            self.limiter.record_failure(self.client_ip);
            return Err(AuthFlowFailure::forbidden("Account is disabled"));
        }

        let password_valid = match self
            .password_verifier
            .verify(&self.request.password, &user.password_hash)
        {
            Ok(valid) => valid,
            Err(error) => {
                error!("Password verification error: {}", error);
                return Err(AuthFlowFailure::internal("Authentication error"));
            }
        };
        if !password_valid {
            warn!(
                "Login attempt with invalid password from IP {}",
                self.client_ip
            );
            self.limiter.record_failure(self.client_ip);
            return Err(AuthFlowFailure::unauthorized("Invalid credentials"));
        }

        Ok(user)
    }
}

struct LoginTeamSelectionStage<'a> {
    auth: &'a crate::auth::AuthSystem,
    team_id: Option<uuid::Uuid>,
}

#[async_trait(?Send)]
impl TeamSelectionStage<crate::core::models::user::types::User> for LoginTeamSelectionStage<'_> {
    type Selection = crate::auth::jwt::types::VerifiedActiveTeam;

    async fn select_team(
        &mut self,
        user: &crate::core::models::user::types::User,
    ) -> Result<Option<Self::Selection>, AuthFlowFailure> {
        let Some(team_id) = self.team_id else {
            return Ok(None);
        };
        match self.auth.validate_active_team(user.id(), team_id).await {
            Ok(Some(verified)) => Ok(Some(verified)),
            Ok(None) => Err(AuthFlowFailure::bad_request("Invalid team selection")),
            Err(error) => {
                error!("Team validation failed during login: {}", error);
                Err(AuthFlowFailure::internal("Internal server error"))
            }
        }
    }
}

struct LoginAuthorizationStage<'a> {
    auth: &'a crate::auth::AuthSystem,
}

#[async_trait(?Send)]
impl AuthorizationStage<crate::core::models::user::types::User> for LoginAuthorizationStage<'_> {
    type Authorization = Vec<String>;

    async fn authorize(
        &mut self,
        user: &crate::core::models::user::types::User,
    ) -> Result<Self::Authorization, AuthFlowFailure> {
        self.auth
            .rbac()
            .get_user_permissions(user)
            .await
            .map_err(|error| {
                error!("Failed to load permissions during login: {}", error);
                AuthFlowFailure::internal("Internal server error")
            })
    }
}

struct LoginEncodingStage<'a> {
    auth: &'a crate::auth::AuthSystem,
    database: &'a crate::storage::database::Database,
}

#[async_trait(?Send)]
impl
    EncodingStage<
        crate::core::models::user::types::User,
        crate::auth::jwt::types::VerifiedActiveTeam,
        Vec<String>,
    > for LoginEncodingStage<'_>
{
    type Output = LoginFlowOutput;

    async fn encode(
        &mut self,
        user: crate::core::models::user::types::User,
        verified_team: Option<crate::auth::jwt::types::VerifiedActiveTeam>,
        permissions: Vec<String>,
    ) -> Result<Self::Output, AuthFlowFailure> {
        if let Err(error) = self.database.update_user_last_login(user.id()).await {
            warn!("Failed to update last login time: {}", error);
        }

        let access_token_result = match verified_team.as_ref() {
            Some(verified_team) => {
                self.auth
                    .jwt()
                    .create_access_token_for_verified_team(
                        user.id(),
                        format!("{:?}", user.role),
                        permissions,
                        verified_team,
                        None,
                    )
                    .await
            }
            None => {
                self.auth
                    .jwt()
                    .create_access_token(
                        user.id(),
                        format!("{:?}", user.role),
                        permissions,
                        None,
                        None,
                    )
                    .await
            }
        };
        let access_token = access_token_result.map_err(|error| {
            error!("Failed to generate access token: {}", error);
            AuthFlowFailure::internal("Token generation failed")
        })?;
        let refresh_token = self
            .auth
            .jwt()
            .create_refresh_token(user.id(), None)
            .await
            .map_err(|error| {
                error!("Failed to generate refresh token: {}", error);
                AuthFlowFailure::internal("Token generation failed")
            })?;

        Ok(LoginFlowOutput {
            user,
            access_token,
            refresh_token,
        })
    }
}

/// Global login rate limiter: 5 attempts per IP per minute
static LOGIN_RATE_LIMITER: std::sync::OnceLock<Arc<AuthRateLimiter>> = std::sync::OnceLock::new();

fn get_login_rate_limiter() -> Arc<AuthRateLimiter> {
    LOGIN_RATE_LIMITER
        .get_or_init(|| Arc::new(AuthRateLimiter::new(5, 60, 60)))
        .clone()
}

/// Parse the leftmost valid IP from an X-Forwarded-For header value.
fn client_ip_from_xff(xff: &str) -> Option<String> {
    xff.split(',')
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<std::net::IpAddr>().ok())
        .map(|ip| ip.to_string())
}

/// Extract the rate-limiting key (client IP) from the request.
///
/// * When the immediate peer is a **trusted proxy** (configured in `server.trusted_proxies`),
///   the leftmost valid IP from `X-Forwarded-For` is used as the real client address.
/// * Otherwise the raw TCP peer address is used so that `X-Forwarded-For` cannot be
///   spoofed by untrusted callers.
///
/// Port numbers are always stripped so the limit applies per-IP, not per-connection.
fn extract_client_ip(req: &HttpRequest, trusted_proxies: &[String]) -> String {
    let peer = req
        .connection_info()
        .peer_addr()
        .unwrap_or("unknown")
        .to_string();

    let peer_ip = peer
        .parse::<std::net::SocketAddr>()
        .map(|addr| addr.ip().to_string())
        .unwrap_or(peer);

    // Only consult X-Forwarded-For when the request arrives from a trusted proxy
    if !trusted_proxies.is_empty()
        && trusted_proxies.contains(&peer_ip)
        && let Some(xff) = req.headers().get("x-forwarded-for")
        && let Ok(xff_str) = xff.to_str()
        && let Some(client_ip) = client_ip_from_xff(xff_str)
    {
        return client_ip;
    }

    peer_ip
}

/// Counter for probabilistic cleanup of rate limiter entries
static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);

/// User login endpoint
pub async fn login(
    req: HttpRequest,
    state: web::Data<AppState>,
    request: web::Json<LoginRequest>,
) -> ActixResult<HttpResponse> {
    login_internal(req, state, request.into_inner(), None).await
}

pub(super) async fn login_with_wire(
    req: HttpRequest,
    state: web::Data<AppState>,
    request: web::Json<LoginWireRequest>,
) -> ActixResult<HttpResponse> {
    let request = request.into_inner();
    login_internal(req, state, request.public, request.team_id).await
}

async fn login_internal(
    req: HttpRequest,
    state: web::Data<AppState>,
    request: LoginRequest,
    team_id: Option<uuid::Uuid>,
) -> ActixResult<HttpResponse> {
    let cfg = state.config.load();
    let client_ip = extract_client_ip(&req, &cfg.gateway.server.trusted_proxies);

    // Rate limit: max 5 login attempts per IP per minute
    let limiter = get_login_rate_limiter();

    // Probabilistic cleanup: every 100th request, purge stale entries
    let count = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    if count.is_multiple_of(100) {
        limiter.cleanup_old_entries();
    }

    if let Err(retry_after) = limiter.check_allowed(&client_ip) {
        warn!(
            "Login rate limit exceeded for IP {}: retry after {}s",
            client_ip, retry_after
        );
        return Ok(HttpResponse::TooManyRequests()
            .insert_header(("Retry-After", retry_after.to_string()))
            .json(ApiResponse::<()>::error(
                "Too many login attempts. Please try again later.".to_string(),
            )));
    }

    info!("User login attempt from IP {}", client_ip);
    let mut flow = LoginFlowDriver {
        primary: LoginPrimaryStage {
            users: DatabaseLoginUserLookup(state.storage.database.as_ref()),
            password_verifier: ProductionPasswordVerifier,
            request,
            client_ip: &client_ip,
            limiter: limiter.as_ref(),
        },
        team: LoginTeamSelectionStage {
            auth: state.auth.as_ref(),
            team_id,
        },
        authorization: LoginAuthorizationStage {
            auth: state.auth.as_ref(),
        },
        encoding: LoginEncodingStage {
            auth: state.auth.as_ref(),
            database: state.storage.database.as_ref(),
        },
    };
    let output = match execute_auth_flow(&mut flow).await {
        Ok(output) => output,
        Err(failure) => return Ok(failure.into_response()),
    };

    info!("User logged in successfully from IP {}", client_ip);

    let response = LoginResponse {
        access_token: output.access_token,
        refresh_token: output.refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: state.auth.jwt().expiration(),
        user: UserInfo {
            id: output.user.id(),
            username: output.user.username,
            email: output.user.email,
            full_name: output.user.display_name,
            role: format!("{:?}", output.user.role),
            email_verified: output.user.email_verified,
        },
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;
    use uuid::Uuid;

    struct UserLookupSpy {
        user: Option<crate::core::models::user::types::User>,
    }

    #[async_trait(?Send)]
    impl LoginUserLookup for UserLookupSpy {
        async fn find_user(
            &mut self,
            _username: &str,
        ) -> crate::utils::error::gateway_error::Result<
            Option<crate::core::models::user::types::User>,
        > {
            Ok(self.user.take())
        }
    }

    struct PasswordSpy;

    impl LoginPasswordVerifier for PasswordSpy {
        fn verify(
            &mut self,
            _password: &str,
            _password_hash: &str,
        ) -> crate::utils::error::gateway_error::Result<bool> {
            Ok(false)
        }
    }

    struct TeamSpy {
        selection: Result<Option<u8>, AuthFlowFailure>,
        calls: Rc<Cell<usize>>,
    }

    #[async_trait(?Send)]
    impl TeamSelectionStage<crate::core::models::user::types::User> for TeamSpy {
        type Selection = u8;

        async fn select_team(
            &mut self,
            _principal: &crate::core::models::user::types::User,
        ) -> Result<Option<Self::Selection>, AuthFlowFailure> {
            self.calls.set(self.calls.get() + 1);
            self.selection
        }
    }

    struct AuthorizationSpy {
        calls: Rc<Cell<usize>>,
    }

    #[async_trait(?Send)]
    impl AuthorizationStage<crate::core::models::user::types::User> for AuthorizationSpy {
        type Authorization = u8;

        async fn authorize(
            &mut self,
            _principal: &crate::core::models::user::types::User,
        ) -> Result<u8, AuthFlowFailure> {
            self.calls.set(self.calls.get() + 1);
            Ok(1)
        }
    }

    struct EncodingSpy {
        calls: Rc<Cell<usize>>,
    }

    #[async_trait(?Send)]
    impl EncodingStage<crate::core::models::user::types::User, u8, u8> for EncodingSpy {
        type Output = u8;

        async fn encode(
            &mut self,
            _principal: crate::core::models::user::types::User,
            _selection: Option<u8>,
            _authorization: u8,
        ) -> Result<Self::Output, AuthFlowFailure> {
            self.calls.set(self.calls.get() + 1);
            Ok(1)
        }
    }

    #[tokio::test]
    async fn gh1130_login_driver_wrong_password_never_checks_team_or_encodes() {
        for selection in [
            Ok(Some(1)),
            Err(AuthFlowFailure::bad_request("foreign team")),
        ] {
            let team_calls = Rc::new(Cell::new(0));
            let rbac_calls = Rc::new(Cell::new(0));
            let encode_calls = Rc::new(Cell::new(0));
            let mut user = crate::core::models::user::types::User::new(
                "wrong-password-user".to_string(),
                "wrong-password@example.com".to_string(),
                "unused-hash".to_string(),
            );
            user.status = crate::core::models::user::types::UserStatus::Active;
            let limiter = AuthRateLimiter::new(5, 60, 60);
            let mut flow = LoginFlowDriver {
                primary: LoginPrimaryStage {
                    users: UserLookupSpy { user: Some(user) },
                    password_verifier: PasswordSpy,
                    request: LoginRequest {
                        username: "wrong-password-user".to_string(),
                        password: "wrong".to_string(),
                    },
                    client_ip: "192.0.2.10",
                    limiter: &limiter,
                },
                team: TeamSpy {
                    selection,
                    calls: team_calls.clone(),
                },
                authorization: AuthorizationSpy {
                    calls: rbac_calls.clone(),
                },
                encoding: EncodingSpy {
                    calls: encode_calls.clone(),
                },
            };
            let failure = match execute_auth_flow(&mut flow).await {
                Ok(_) => panic!("wrong password must fail"),
                Err(failure) => failure,
            };
            assert_eq!(
                failure.into_response().status(),
                actix_web::http::StatusCode::UNAUTHORIZED
            );
            assert_eq!(team_calls.get(), 0);
            assert_eq!(rbac_calls.get(), 0);
            assert_eq!(encode_calls.get(), 0);
        }
    }

    // NOTE: Full integration tests require mocking AppState, AuthSystem, and StorageLayer.

    #[test]
    fn test_login_request_deserialization() {
        let json = r#"{"username": "testuser", "password": "pass123"}"#;
        let request: LoginRequest = serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(request.username, "testuser");
        assert_eq!(request.password, "pass123");
    }

    #[test]
    fn test_login_request_missing_fields() {
        let json = r#"{"username": "testuser"}"#;
        let result: Result<LoginRequest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_login_response_serialization() {
        let response = LoginResponse {
            access_token: "access_token_here".to_string(),
            refresh_token: "refresh_token_here".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: 3600,
            user: UserInfo {
                id: Uuid::new_v4(),
                username: "testuser".to_string(),
                email: "test@example.com".to_string(),
                full_name: Some("Test User".to_string()),
                role: "User".to_string(),
                email_verified: true,
            },
        };

        let json = serde_json::to_string(&response).expect("Failed to serialize");
        assert!(json.contains("access_token"));
        assert!(json.contains("refresh_token"));
        assert!(json.contains("Bearer"));
        assert!(json.contains("testuser"));
    }

    #[test]
    fn test_user_info_structure() {
        let user_info = UserInfo {
            id: Uuid::new_v4(),
            username: "john_doe".to_string(),
            email: "john@example.com".to_string(),
            full_name: Some("John Doe".to_string()),
            role: "Admin".to_string(),
            email_verified: true,
        };

        assert_eq!(user_info.username, "john_doe");
        assert_eq!(user_info.role, "Admin");
        assert!(user_info.email_verified);
        assert!(user_info.full_name.is_some());
    }

    #[test]
    fn test_login_rate_limiter_blocks_after_limit() {
        let limiter = AuthRateLimiter::new(5, 60, 60);
        let ip = "192.0.2.1";

        // First 5 attempts should be allowed
        for _ in 0..5 {
            assert!(limiter.check_allowed(ip).is_ok());
            limiter.record_failure(ip);
        }

        // 6th attempt should be blocked
        assert!(limiter.check_allowed(ip).is_err());
    }

    #[test]
    fn test_login_rate_limiter_different_ips_independent() {
        let limiter = AuthRateLimiter::new(5, 60, 60);
        let ip1 = "192.0.2.1";
        let ip2 = "192.0.2.2";

        // Exhaust limit for ip1
        for _ in 0..5 {
            assert!(limiter.check_allowed(ip1).is_ok());
            limiter.record_failure(ip1);
        }
        assert!(limiter.check_allowed(ip1).is_err());

        // ip2 should still be allowed
        assert!(limiter.check_allowed(ip2).is_ok());
    }

    #[test]
    fn test_extract_client_ip_strips_port() {
        // IPv4 with port: only the IP portion should be used as the rate-limit key
        let ipv4_with_port = "192.0.2.1:54321"
            .parse::<std::net::SocketAddr>()
            .map(|a| a.ip().to_string())
            .unwrap_or_else(|_| "192.0.2.1:54321".to_string());
        assert_eq!(ipv4_with_port, "192.0.2.1");

        // IPv6 with port
        let ipv6_with_port = "[::1]:54321"
            .parse::<std::net::SocketAddr>()
            .map(|a| a.ip().to_string())
            .unwrap_or_else(|_| "[::1]:54321".to_string());
        assert_eq!(ipv6_with_port, "::1");
    }

    #[test]
    fn test_success_does_not_reset_rate_limit_counter() {
        // Successful login must NOT reset the counter: an attacker with one valid
        // account must not be able to interleave their own successful logins to
        // bypass the per-IP brute-force limit against a victim account.
        let limiter = AuthRateLimiter::new(5, 60, 60);
        let ip = "192.0.2.1";

        // Accumulate 4 failures (one short of lockout)
        for _ in 0..4 {
            assert!(limiter.check_allowed(ip).is_ok());
            limiter.record_failure(ip);
        }

        // The login handler no longer calls record_success(); simulate by doing nothing.

        // The 5th failure still hits the limit and triggers lockout
        assert!(limiter.check_allowed(ip).is_ok());
        limiter.record_failure(ip);
        assert!(limiter.check_allowed(ip).is_err());
    }

    // ---- trusted-proxy XFF helpers ----

    #[test]
    fn test_client_ip_from_xff_single() {
        assert_eq!(
            client_ip_from_xff("203.0.113.5"),
            Some("203.0.113.5".to_string())
        );
    }

    #[test]
    fn test_client_ip_from_xff_chain() {
        // Leftmost address is the original client
        assert_eq!(
            client_ip_from_xff("203.0.113.5, 10.0.0.1, 10.0.0.2"),
            Some("203.0.113.5".to_string())
        );
    }

    #[test]
    fn test_client_ip_from_xff_invalid_returns_none() {
        assert_eq!(client_ip_from_xff("not-an-ip, 10.0.0.1"), None);
    }

    #[test]
    fn test_client_ip_from_xff_empty_returns_none() {
        assert_eq!(client_ip_from_xff(""), None);
    }

    #[test]
    fn test_client_ip_from_xff_ipv6() {
        assert_eq!(
            client_ip_from_xff("2001:db8::1, 10.0.0.1"),
            Some("2001:db8::1".to_string())
        );
    }
}
