//! Token refresh endpoint

use crate::server::routes::ApiResponse;
use crate::server::state::AppState;
use actix_web::{HttpResponse, Result as ActixResult, web};
use async_trait::async_trait;
use tracing::{debug, error, warn};

use super::models::{
    AuthFlowFailure, AuthorizationStage, EncodingStage, PrimaryAuthStage, RefreshTokenRequest,
    RefreshWireRequest, SequencedAuthFlow, TeamSelectionStage, execute_auth_flow,
};

type RefreshFlowDriver<P, T, A, E> = SequencedAuthFlow<P, T, A, E>;

#[async_trait(?Send)]
trait RefreshTokenVerifier {
    async fn verify(
        &mut self,
        refresh_token: &str,
    ) -> crate::utils::error::gateway_error::Result<uuid::Uuid>;
}

struct JwtRefreshTokenVerifier<'a>(&'a crate::auth::jwt::types::JwtHandler);

#[async_trait(?Send)]
impl RefreshTokenVerifier for JwtRefreshTokenVerifier<'_> {
    async fn verify(
        &mut self,
        refresh_token: &str,
    ) -> crate::utils::error::gateway_error::Result<uuid::Uuid> {
        self.0.verify_refresh_token(refresh_token).await
    }
}

#[async_trait(?Send)]
trait RefreshUserLookup {
    async fn find_user(
        &mut self,
        user_id: uuid::Uuid,
    ) -> crate::utils::error::gateway_error::Result<Option<crate::core::models::user::types::User>>;
}

struct DatabaseRefreshUserLookup<'a>(&'a crate::storage::database::Database);

#[async_trait(?Send)]
impl RefreshUserLookup for DatabaseRefreshUserLookup<'_> {
    async fn find_user(
        &mut self,
        user_id: uuid::Uuid,
    ) -> crate::utils::error::gateway_error::Result<Option<crate::core::models::user::types::User>>
    {
        self.0.find_user_by_id(user_id).await
    }
}

struct RefreshPrimaryStage<V, U> {
    verifier: V,
    users: U,
    request: RefreshTokenRequest,
}

#[async_trait(?Send)]
impl<V: RefreshTokenVerifier, U: RefreshUserLookup> PrimaryAuthStage for RefreshPrimaryStage<V, U> {
    type Principal = crate::core::models::user::types::User;

    async fn authenticate(&mut self) -> Result<Self::Principal, AuthFlowFailure> {
        let user_id = self
            .verifier
            .verify(&self.request.refresh_token)
            .await
            .map_err(|error| {
                warn!("Invalid refresh token: {}", error);
                AuthFlowFailure::unauthorized("Invalid refresh token")
            })?;
        let user = match self.users.find_user(user_id).await {
            Ok(Some(user)) => user,
            Ok(None) => {
                warn!("Refresh token for non-existent user: {}", user_id);
                return Err(AuthFlowFailure::unauthorized("Invalid token"));
            }
            Err(error) => {
                error!("Database error during token refresh: {}", error);
                return Err(AuthFlowFailure::internal("Database error"));
            }
        };
        if !user.is_active() {
            return Err(AuthFlowFailure::unauthorized("Invalid token"));
        }
        Ok(user)
    }
}

struct RefreshTeamSelectionStage<'a> {
    auth: &'a crate::auth::AuthSystem,
    team_id: Option<uuid::Uuid>,
}

#[async_trait(?Send)]
impl TeamSelectionStage<crate::core::models::user::types::User> for RefreshTeamSelectionStage<'_> {
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
                error!("Team validation failed during token refresh: {}", error);
                Err(AuthFlowFailure::internal("Internal server error"))
            }
        }
    }
}

struct RefreshAuthorizationStage<'a> {
    auth: &'a crate::auth::AuthSystem,
}

#[async_trait(?Send)]
impl AuthorizationStage<crate::core::models::user::types::User> for RefreshAuthorizationStage<'_> {
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
                error!("Failed to load permissions during token refresh: {}", error);
                AuthFlowFailure::internal("Internal server error")
            })
    }
}

struct RefreshEncodingStage<'a> {
    jwt: &'a crate::auth::jwt::types::JwtHandler,
}

#[async_trait(?Send)]
impl
    EncodingStage<
        crate::core::models::user::types::User,
        crate::auth::jwt::types::VerifiedActiveTeam,
        Vec<String>,
    > for RefreshEncodingStage<'_>
{
    type Output = crate::auth::jwt::types::TokenPair;

    async fn encode(
        &mut self,
        user: crate::core::models::user::types::User,
        verified_team: Option<crate::auth::jwt::types::VerifiedActiveTeam>,
        permissions: Vec<String>,
    ) -> Result<Self::Output, AuthFlowFailure> {
        let token_result = match verified_team.as_ref() {
            Some(verified_team) => {
                self.jwt
                    .create_token_pair_for_verified_team(
                        user.id(),
                        user.role.to_string(),
                        permissions,
                        verified_team,
                        None,
                    )
                    .await
            }
            None => {
                self.jwt
                    .create_token_pair(user.id(), user.role.to_string(), permissions, None, None)
                    .await
            }
        };
        token_result.map_err(|error| {
            error!("Failed to generate new tokens: {}", error);
            AuthFlowFailure::internal("Internal server error")
        })
    }
}

/// Refresh token endpoint
pub async fn refresh_token(
    state: web::Data<AppState>,
    request: web::Json<RefreshTokenRequest>,
) -> ActixResult<HttpResponse> {
    refresh_token_internal(state, request.into_inner(), None).await
}

pub(super) async fn refresh_token_with_wire(
    state: web::Data<AppState>,
    request: web::Json<RefreshWireRequest>,
) -> ActixResult<HttpResponse> {
    let request = request.into_inner();
    refresh_token_internal(state, request.public, request.team_id).await
}

async fn refresh_token_internal(
    state: web::Data<AppState>,
    request: RefreshTokenRequest,
    team_id: Option<uuid::Uuid>,
) -> ActixResult<HttpResponse> {
    debug!("Token refresh request");
    let mut flow = RefreshFlowDriver {
        primary: RefreshPrimaryStage {
            verifier: JwtRefreshTokenVerifier(state.auth.jwt()),
            users: DatabaseRefreshUserLookup(state.storage.database.as_ref()),
            request,
        },
        team: RefreshTeamSelectionStage {
            auth: state.auth.as_ref(),
            team_id,
        },
        authorization: RefreshAuthorizationStage {
            auth: state.auth.as_ref(),
        },
        encoding: RefreshEncodingStage {
            jwt: state.auth.jwt(),
        },
    };
    match execute_auth_flow(&mut flow).await {
        Ok(tokens) => Ok(HttpResponse::Ok().json(ApiResponse::success(tokens))),
        Err(failure) => Ok(failure.into_response()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    struct VerifierSpy {
        valid: bool,
        user_id: uuid::Uuid,
        calls: Rc<Cell<usize>>,
    }

    #[async_trait(?Send)]
    impl RefreshTokenVerifier for VerifierSpy {
        async fn verify(
            &mut self,
            _refresh_token: &str,
        ) -> crate::utils::error::gateway_error::Result<uuid::Uuid> {
            self.calls.set(self.calls.get() + 1);
            if self.valid {
                Ok(self.user_id)
            } else {
                Err(crate::utils::error::gateway_error::GatewayError::auth(
                    "invalid refresh",
                ))
            }
        }
    }

    struct UserLookupSpy {
        user: Option<crate::core::models::user::types::User>,
        calls: Rc<Cell<usize>>,
    }

    #[async_trait(?Send)]
    impl RefreshUserLookup for UserLookupSpy {
        async fn find_user(
            &mut self,
            _user_id: uuid::Uuid,
        ) -> crate::utils::error::gateway_error::Result<
            Option<crate::core::models::user::types::User>,
        > {
            self.calls.set(self.calls.get() + 1);
            Ok(self.user.take())
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
    async fn gh1130_refresh_driver_primary_failures_never_check_team_or_encode() {
        for (case_name, primary_kind, selection) in [
            ("invalid_refresh_with_team", "invalid", Ok(Some(1))),
            (
                "invalid_refresh_foreign_team",
                "invalid",
                Err(AuthFlowFailure::bad_request("foreign team")),
            ),
            ("missing_user_with_team", "missing", Ok(Some(1))),
            ("missing_user_without_team", "missing", Ok(None)),
            ("inactive_user_with_team", "inactive", Ok(Some(1))),
            ("inactive_user_without_team", "inactive", Ok(None)),
        ] {
            let verifier_calls = Rc::new(Cell::new(0));
            let user_lookup_calls = Rc::new(Cell::new(0));
            let team_calls = Rc::new(Cell::new(0));
            let rbac_calls = Rc::new(Cell::new(0));
            let encode_calls = Rc::new(Cell::new(0));
            let user_id = uuid::Uuid::new_v4();
            let user = if primary_kind == "inactive" {
                let mut user = crate::core::models::user::types::User::new(
                    "inactive-refresh-user".to_string(),
                    "inactive-refresh@example.com".to_string(),
                    "unused-hash".to_string(),
                );
                user.status = crate::core::models::user::types::UserStatus::Inactive;
                Some(user)
            } else {
                None
            };
            let mut flow = RefreshFlowDriver {
                primary: RefreshPrimaryStage {
                    verifier: VerifierSpy {
                        valid: primary_kind != "invalid",
                        user_id,
                        calls: verifier_calls.clone(),
                    },
                    users: UserLookupSpy {
                        user,
                        calls: user_lookup_calls.clone(),
                    },
                    request: RefreshTokenRequest {
                        refresh_token: "refresh".to_string(),
                    },
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
                Ok(_) => panic!("invalid refresh principal must fail"),
                Err(failure) => failure,
            };
            assert_eq!(
                failure.into_response().status(),
                actix_web::http::StatusCode::UNAUTHORIZED,
                "{case_name}"
            );
            assert_eq!(verifier_calls.get(), 1, "{case_name}");
            assert_eq!(
                user_lookup_calls.get(),
                usize::from(primary_kind != "invalid"),
                "{case_name}"
            );
            assert_eq!(team_calls.get(), 0, "{case_name}");
            assert_eq!(rbac_calls.get(), 0, "{case_name}");
            assert_eq!(encode_calls.get(), 0, "{case_name}");
        }
    }
}
