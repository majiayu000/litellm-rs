//! Request and response models for authentication endpoints

use crate::auth::jwt::types::TokenPair;
use crate::core::models::user::types::User;
use actix_web::HttpResponse;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::routes::ApiResponse;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AuthFlowStatus {
    Unauthorized,
    Forbidden,
    BadRequest,
    InternalServerError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AuthFlowFailure {
    status: AuthFlowStatus,
    public_message: &'static str,
}

impl AuthFlowFailure {
    pub(super) const fn unauthorized(public_message: &'static str) -> Self {
        Self {
            status: AuthFlowStatus::Unauthorized,
            public_message,
        }
    }

    pub(super) const fn forbidden(public_message: &'static str) -> Self {
        Self {
            status: AuthFlowStatus::Forbidden,
            public_message,
        }
    }

    pub(super) const fn bad_request(public_message: &'static str) -> Self {
        Self {
            status: AuthFlowStatus::BadRequest,
            public_message,
        }
    }

    pub(super) const fn internal(public_message: &'static str) -> Self {
        Self {
            status: AuthFlowStatus::InternalServerError,
            public_message,
        }
    }

    pub(super) fn into_response(self) -> HttpResponse {
        let payload = || ApiResponse::<()>::error(self.public_message.to_string());
        match self.status {
            AuthFlowStatus::Unauthorized => HttpResponse::Unauthorized().json(payload()),
            AuthFlowStatus::Forbidden => HttpResponse::Forbidden().json(payload()),
            AuthFlowStatus::BadRequest => HttpResponse::BadRequest().json(payload()),
            AuthFlowStatus::InternalServerError => {
                HttpResponse::InternalServerError().json(payload())
            }
        }
    }
}

#[async_trait(?Send)]
pub(super) trait AuthFlow {
    type Principal;
    type Selection;
    type Authorization;
    type Output;

    async fn authenticate(&mut self) -> Result<Self::Principal, AuthFlowFailure>;

    async fn select_team(
        &mut self,
        principal: &Self::Principal,
    ) -> Result<Option<Self::Selection>, AuthFlowFailure>;

    async fn authorize(
        &mut self,
        principal: &Self::Principal,
    ) -> Result<Self::Authorization, AuthFlowFailure>;

    async fn encode(
        &mut self,
        principal: Self::Principal,
        selection: Option<Self::Selection>,
        authorization: Self::Authorization,
    ) -> Result<Self::Output, AuthFlowFailure>;
}

#[async_trait(?Send)]
pub(super) trait PrimaryAuthStage {
    type Principal;

    async fn authenticate(&mut self) -> Result<Self::Principal, AuthFlowFailure>;
}

#[async_trait(?Send)]
pub(super) trait TeamSelectionStage<P> {
    type Selection;

    async fn select_team(
        &mut self,
        principal: &P,
    ) -> Result<Option<Self::Selection>, AuthFlowFailure>;
}

#[async_trait(?Send)]
pub(super) trait AuthorizationStage<P> {
    type Authorization;

    async fn authorize(&mut self, principal: &P) -> Result<Self::Authorization, AuthFlowFailure>;
}

#[async_trait(?Send)]
pub(super) trait EncodingStage<P, S, A> {
    type Output;

    async fn encode(
        &mut self,
        principal: P,
        selection: Option<S>,
        authorization: A,
    ) -> Result<Self::Output, AuthFlowFailure>;
}

pub(super) struct SequencedAuthFlow<P, T, A, E> {
    pub(super) primary: P,
    pub(super) team: T,
    pub(super) authorization: A,
    pub(super) encoding: E,
}

#[async_trait(?Send)]
impl<P, T, A, E> AuthFlow for SequencedAuthFlow<P, T, A, E>
where
    P: PrimaryAuthStage,
    T: TeamSelectionStage<P::Principal>,
    A: AuthorizationStage<P::Principal>,
    E: EncodingStage<
            P::Principal,
            T::Selection,
            <A as AuthorizationStage<P::Principal>>::Authorization,
        >,
{
    type Principal = P::Principal;
    type Selection = T::Selection;
    type Authorization = <A as AuthorizationStage<P::Principal>>::Authorization;
    type Output = E::Output;

    async fn authenticate(&mut self) -> Result<Self::Principal, AuthFlowFailure> {
        self.primary.authenticate().await
    }

    async fn select_team(
        &mut self,
        principal: &Self::Principal,
    ) -> Result<Option<Self::Selection>, AuthFlowFailure> {
        self.team.select_team(principal).await
    }

    async fn authorize(
        &mut self,
        principal: &Self::Principal,
    ) -> Result<Self::Authorization, AuthFlowFailure> {
        self.authorization.authorize(principal).await
    }

    async fn encode(
        &mut self,
        principal: Self::Principal,
        selection: Option<Self::Selection>,
        authorization: Self::Authorization,
    ) -> Result<Self::Output, AuthFlowFailure> {
        self.encoding
            .encode(principal, selection, authorization)
            .await
    }
}

pub(super) async fn execute_auth_flow<F: AuthFlow>(
    flow: &mut F,
) -> Result<F::Output, AuthFlowFailure> {
    let principal = flow.authenticate().await?;
    let selection = flow.select_team(&principal).await?;
    let authorization = flow.authorize(&principal).await?;
    flow.encode(principal, selection, authorization).await
}

/// User registration request
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub full_name: Option<String>,
}

/// User login request
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct LoginWireRequest {
    #[serde(flatten)]
    pub(super) public: LoginRequest,
    pub(super) team_id: Option<Uuid>,
}

/// Password change request
#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// Forgot password request
#[derive(Debug, Deserialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

/// Reset password request
#[derive(Debug, Deserialize)]
pub struct ResetPasswordRequest {
    pub token: String,
    pub new_password: String,
}

/// Email verification request
#[derive(Debug, Deserialize)]
pub struct VerifyEmailRequest {
    pub token: String,
}

/// Refresh token request
#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct RefreshWireRequest {
    #[serde(flatten)]
    pub(super) public: RefreshTokenRequest,
    pub(super) team_id: Option<Uuid>,
}

/// Authentication response
#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub user: UserResponse,
    pub tokens: TokenPair,
}

/// User response (without sensitive data)
#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: uuid::Uuid,
    pub username: String,
    pub email: String,
    pub display_name: Option<String>,
    pub role: String,
    pub email_verified: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Registration response
#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub user_id: uuid::Uuid,
    pub username: String,
    pub email: String,
    pub message: String,
}

/// Login response
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub user: UserInfo,
}

/// User info for login response
#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: uuid::Uuid,
    pub username: String,
    pub email: String,
    pub full_name: Option<String>,
    pub role: String,
    pub email_verified: bool,
}

/// Refresh token response
#[derive(Debug, Serialize)]
pub struct RefreshTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id(),
            username: user.username.clone(),
            email: user.email.clone(),
            display_name: user.display_name.clone(),
            role: format!("{:?}", user.role),
            email_verified: user.email_verified,
            created_at: user.metadata.created_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy)]
    struct FlowCase {
        name: &'static str,
        selected_team: bool,
        authenticate: Result<u8, AuthFlowFailure>,
        selection: Result<Option<u8>, AuthFlowFailure>,
        authorization: Result<u8, AuthFlowFailure>,
        encoding: Result<u8, AuthFlowFailure>,
        expected: Result<u8, AuthFlowStatus>,
        expected_team_repository_calls: usize,
        expected_rbac_calls: usize,
        expected_encode_calls: usize,
    }

    struct SpyFlow {
        case: FlowCase,
        team_repository_calls: usize,
        rbac_calls: usize,
        encode_calls: usize,
    }

    #[async_trait(?Send)]
    impl AuthFlow for SpyFlow {
        type Principal = u8;
        type Selection = u8;
        type Authorization = u8;
        type Output = u8;

        async fn authenticate(&mut self) -> Result<Self::Principal, AuthFlowFailure> {
            self.case.authenticate
        }

        async fn select_team(
            &mut self,
            _principal: &Self::Principal,
        ) -> Result<Option<Self::Selection>, AuthFlowFailure> {
            if self.case.selected_team {
                self.team_repository_calls += 1;
            }
            self.case.selection
        }

        async fn authorize(
            &mut self,
            _principal: &Self::Principal,
        ) -> Result<Self::Authorization, AuthFlowFailure> {
            self.rbac_calls += 1;
            self.case.authorization
        }

        async fn encode(
            &mut self,
            _principal: Self::Principal,
            _selection: Option<Self::Selection>,
            _authorization: Self::Authorization,
        ) -> Result<Self::Output, AuthFlowFailure> {
            self.encode_calls += 1;
            self.case.encoding
        }
    }

    const OK: Result<u8, AuthFlowFailure> = Ok(1);
    const NO_TEAM: Result<Option<u8>, AuthFlowFailure> = Ok(None);
    const VALID_TEAM: Result<Option<u8>, AuthFlowFailure> = Ok(Some(1));
    const INVALID_PRIMARY: Result<u8, AuthFlowFailure> =
        Err(AuthFlowFailure::unauthorized("invalid primary"));
    const INVALID_SELECTION: Result<Option<u8>, AuthFlowFailure> =
        Err(AuthFlowFailure::bad_request("invalid selection"));
    const REPOSITORY_FAILURE: Result<Option<u8>, AuthFlowFailure> =
        Err(AuthFlowFailure::internal("repository failure"));
    const RBAC_FAILURE: Result<u8, AuthFlowFailure> =
        Err(AuthFlowFailure::internal("rbac failure"));
    const ENCODE_FAILURE: Result<u8, AuthFlowFailure> =
        Err(AuthFlowFailure::internal("encode failure"));

    #[test]
    fn gh1130_auth_flow_failures_map_to_exact_http_statuses() {
        for (failure, expected) in [
            (
                AuthFlowFailure::unauthorized("unauthorized"),
                actix_web::http::StatusCode::UNAUTHORIZED,
            ),
            (
                AuthFlowFailure::forbidden("forbidden"),
                actix_web::http::StatusCode::FORBIDDEN,
            ),
            (
                AuthFlowFailure::bad_request("bad request"),
                actix_web::http::StatusCode::BAD_REQUEST,
            ),
            (
                AuthFlowFailure::internal("internal"),
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            ),
        ] {
            assert_eq!(failure.into_response().status(), expected);
        }
    }

    macro_rules! flow_case {
        (
            $name:expr,
            $selected_team:expr,
            $authenticate:expr,
            $selection:expr,
            $authorization:expr,
            $encoding:expr,
            $expected:expr,
            $expected_calls:expr $(,)?
        ) => {{
            let expected_calls = $expected_calls;
            FlowCase {
                name: $name,
                selected_team: $selected_team,
                authenticate: $authenticate,
                selection: $selection,
                authorization: $authorization,
                encoding: $encoding,
                expected: $expected,
                expected_team_repository_calls: expected_calls.0,
                expected_rbac_calls: expected_calls.1,
                expected_encode_calls: expected_calls.2,
            }
        }};
    }

    #[tokio::test]
    async fn gh1130_authentication_selection_rbac_encoding_matrix_is_fail_closed() {
        let cases = [
            flow_case!(
                "login_wrong_password_valid_team",
                true,
                INVALID_PRIMARY,
                VALID_TEAM,
                OK,
                OK,
                Err(AuthFlowStatus::Unauthorized),
                (0, 0, 0),
            ),
            flow_case!(
                "login_wrong_password_foreign_team",
                true,
                INVALID_PRIMARY,
                INVALID_SELECTION,
                OK,
                OK,
                Err(AuthFlowStatus::Unauthorized),
                (0, 0, 0),
            ),
            flow_case!(
                "refresh_invalid_token_valid_team",
                true,
                INVALID_PRIMARY,
                VALID_TEAM,
                OK,
                OK,
                Err(AuthFlowStatus::Unauthorized),
                (0, 0, 0),
            ),
            flow_case!(
                "refresh_invalid_token_foreign_team",
                true,
                INVALID_PRIMARY,
                INVALID_SELECTION,
                OK,
                OK,
                Err(AuthFlowStatus::Unauthorized),
                (0, 0, 0),
            ),
            flow_case!(
                "refresh_missing_user_some_team",
                true,
                INVALID_PRIMARY,
                VALID_TEAM,
                OK,
                OK,
                Err(AuthFlowStatus::Unauthorized),
                (0, 0, 0),
            ),
            flow_case!(
                "refresh_missing_user_no_team",
                false,
                INVALID_PRIMARY,
                NO_TEAM,
                OK,
                OK,
                Err(AuthFlowStatus::Unauthorized),
                (0, 0, 0),
            ),
            flow_case!(
                "refresh_inactive_user_some_team",
                true,
                INVALID_PRIMARY,
                VALID_TEAM,
                OK,
                OK,
                Err(AuthFlowStatus::Unauthorized),
                (0, 0, 0),
            ),
            flow_case!(
                "refresh_inactive_user_no_team",
                false,
                INVALID_PRIMARY,
                NO_TEAM,
                OK,
                OK,
                Err(AuthFlowStatus::Unauthorized),
                (0, 0, 0),
            ),
            flow_case!(
                "login_foreign_team",
                true,
                OK,
                INVALID_SELECTION,
                OK,
                OK,
                Err(AuthFlowStatus::BadRequest),
                (1, 0, 0),
            ),
            flow_case!(
                "refresh_foreign_team",
                true,
                OK,
                INVALID_SELECTION,
                OK,
                OK,
                Err(AuthFlowStatus::BadRequest),
                (1, 0, 0),
            ),
            flow_case!(
                "login_team_repository_failure",
                true,
                OK,
                REPOSITORY_FAILURE,
                OK,
                OK,
                Err(AuthFlowStatus::InternalServerError),
                (1, 0, 0),
            ),
            flow_case!(
                "refresh_team_repository_failure",
                true,
                OK,
                REPOSITORY_FAILURE,
                OK,
                OK,
                Err(AuthFlowStatus::InternalServerError),
                (1, 0, 0),
            ),
            flow_case!(
                "login_rbac_failure",
                true,
                OK,
                VALID_TEAM,
                RBAC_FAILURE,
                OK,
                Err(AuthFlowStatus::InternalServerError),
                (1, 1, 0),
            ),
            flow_case!(
                "refresh_rbac_failure_without_team",
                false,
                OK,
                NO_TEAM,
                RBAC_FAILURE,
                OK,
                Err(AuthFlowStatus::InternalServerError),
                (0, 1, 0),
            ),
            flow_case!(
                "login_without_team_success",
                false,
                OK,
                NO_TEAM,
                OK,
                OK,
                Ok(1),
                (0, 1, 1),
            ),
            flow_case!(
                "refresh_with_valid_team_success",
                true,
                OK,
                VALID_TEAM,
                OK,
                OK,
                Ok(1),
                (1, 1, 1),
            ),
            flow_case!(
                "login_encode_failure",
                true,
                OK,
                VALID_TEAM,
                OK,
                ENCODE_FAILURE,
                Err(AuthFlowStatus::InternalServerError),
                (1, 1, 1),
            ),
        ];

        for flow_case in cases {
            let mut flow = SpyFlow {
                case: flow_case,
                team_repository_calls: 0,
                rbac_calls: 0,
                encode_calls: 0,
            };
            let actual = execute_auth_flow(&mut flow)
                .await
                .map_err(|failure| failure.status);
            assert_eq!(actual, flow_case.expected, "{}", flow_case.name);
            assert_eq!(
                flow.team_repository_calls, flow_case.expected_team_repository_calls,
                "{} team repository calls",
                flow_case.name
            );
            assert_eq!(
                flow.rbac_calls, flow_case.expected_rbac_calls,
                "{} RBAC calls",
                flow_case.name
            );
            assert_eq!(
                flow.encode_calls, flow_case.expected_encode_calls,
                "{} encode calls",
                flow_case.name
            );
        }
    }

    #[test]
    fn test_register_request_validation() {
        let request = RegisterRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "SecurePass123!".to_string(),
            full_name: Some("Test User".to_string()),
        };

        assert_eq!(request.username, "testuser");
        assert_eq!(request.email, "test@example.com");
        assert!(request.full_name.is_some());
    }

    #[test]
    fn test_user_response_conversion() {
        // This would require a real User instance in a full test
        // For now, just test the structure
        let user_response = UserResponse {
            id: uuid::Uuid::new_v4(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            display_name: Some("Test User".to_string()),
            role: "User".to_string(),
            email_verified: false,
            created_at: chrono::Utc::now(),
        };

        assert_eq!(user_response.username, "testuser");
        assert!(!user_response.email_verified);
    }
}
