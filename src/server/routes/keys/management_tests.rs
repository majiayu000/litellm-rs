use super::handler_tests::{auth_enabled_test_state, make_user};
use crate::core::keys::{CreateKeyConfig, KeyPermissions, KeyStatus};
use crate::core::models::user::types::{User, UserRole};
use crate::server::middleware::AuthMiddleware;
use crate::server::routes::keys::configure_routes;
use crate::server::state::AppState;
use actix_web::{App, http::Method, http::StatusCode, test, web};
use serde_json::json;
use uuid::Uuid;

async fn seed_user(state: &AppState, role: UserRole) -> User {
    let mut user = make_user(role, vec![]);
    user.username = format!("management-{}", user.id());
    user.email = format!("{}@example.test", user.username);
    state.storage.db().create_user(&user).await.unwrap()
}

async fn seed_key(state: &AppState, user: &User, permissions: KeyPermissions) -> (Uuid, String) {
    state
        .key_manager
        .generate_key(CreateKeyConfig {
            name: "management fixture".to_string(),
            user_id: Some(user.id()),
            permissions,
            ..Default::default()
        })
        .await
        .unwrap()
}

#[actix_web::test]
async fn inference_credentials_cannot_manage_keys_even_with_an_admin_owner() {
    let state = auth_enabled_test_state().await;
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .wrap(AuthMiddleware)
            .configure(configure_routes),
    )
    .await;

    for role in [UserRole::User, UserRole::Admin] {
        let user = seed_user(&state, role).await;
        let (target_id, _) = seed_key(&state, &user, KeyPermissions::default()).await;
        for permissions in [
            KeyPermissions::default(),
            KeyPermissions {
                allowed_models: vec!["gpt-4o".to_string()],
                max_tokens_per_request: Some(8),
                ..Default::default()
            },
            KeyPermissions {
                custom_permissions: vec!["api.chat".to_string()],
                ..Default::default()
            },
            KeyPermissions {
                custom_permissions: vec!["use:api".to_string()],
                ..Default::default()
            },
        ] {
            let (caller_id, raw_key) = seed_key(&state, &user, permissions).await;
            let count = state.key_manager.count_keys(None).await.unwrap();
            let response = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/keys")
                    .insert_header(("x-api-key", raw_key.clone()))
                    .set_json(json!({"name": "unrestricted child"}))
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::FORBIDDEN);

            for id in [caller_id, target_id, Uuid::new_v4()] {
                for (method, path) in [
                    (Method::GET, format!("/v1/keys/{id}")),
                    (Method::GET, format!("/v1/keys/{id}/usage")),
                    (Method::PUT, format!("/v1/keys/{id}")),
                    (Method::DELETE, format!("/v1/keys/{id}")),
                    (Method::POST, format!("/v1/keys/{id}/rotate")),
                ] {
                    let response = test::call_service(
                        &app,
                        test::TestRequest::default()
                            .method(method.clone())
                            .uri(&path)
                            .insert_header(("x-api-key", raw_key.clone()))
                            .set_json(json!({
                                "name": "widened",
                                "permissions": {},
                                "expires_at": null,
                                "budget_id": null
                            }))
                            .to_request(),
                    )
                    .await;
                    assert_eq!(
                        response.status(),
                        StatusCode::FORBIDDEN,
                        "{method} {path} must require the credential's management permission"
                    );
                }
            }
            let response = test::call_service(
                &app,
                test::TestRequest::get()
                    .uri(&format!("/v1/keys?user_id={}", user.id()))
                    .insert_header(("x-api-key", raw_key.clone()))
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
            assert_eq!(state.key_manager.count_keys(None).await.unwrap(), count);
            let target = state.key_manager.get_key(target_id).await.unwrap().unwrap();
            assert_eq!(target.status, KeyStatus::Active);
            assert_eq!(target.name, "management fixture");

            let response = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/keys/verify")
                    .insert_header(("x-api-key", raw_key.clone()))
                    .set_json(json!({"key": raw_key.clone()}))
                    .to_request(),
            )
            .await;
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "self-verification remains allowed"
            );
            let response = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/keys/verify")
                    .insert_header(("x-api-key", raw_key))
                    .set_json(json!({"key": "gw-unknown-credential"}))
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }
    }
}

#[actix_web::test]
async fn management_grants_are_operation_specific_and_keep_owner_isolation() {
    let state = auth_enabled_test_state().await;
    let user = seed_user(&state, UserRole::User).await;
    let foreign_user = seed_user(&state, UserRole::User).await;
    let (target_id, _) = seed_key(&state, &user, KeyPermissions::default()).await;
    let (foreign_id, _) = seed_key(&state, &foreign_user, KeyPermissions::default()).await;
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .wrap(AuthMiddleware)
            .configure(configure_routes),
    )
    .await;

    for permission in ["api_keys.read", "keys.list_all"] {
        let (_, key) = seed_key(
            &state,
            &user,
            KeyPermissions {
                custom_permissions: vec![permission.to_string()],
                ..Default::default()
            },
        )
        .await;
        for (method, path, expected) in [
            (Method::GET, format!("/v1/keys/{target_id}"), StatusCode::OK),
            (
                Method::GET,
                format!("/v1/keys/{foreign_id}"),
                StatusCode::FORBIDDEN,
            ),
            (
                Method::PUT,
                format!("/v1/keys/{target_id}"),
                StatusCode::FORBIDDEN,
            ),
            (
                Method::DELETE,
                format!("/v1/keys/{target_id}"),
                StatusCode::FORBIDDEN,
            ),
            (
                Method::POST,
                format!("/v1/keys/{target_id}/rotate"),
                StatusCode::FORBIDDEN,
            ),
        ] {
            let response = test::call_service(
                &app,
                test::TestRequest::default()
                    .method(method)
                    .uri(&path)
                    .insert_header(("x-api-key", key.clone()))
                    .set_json(json!({"name": "forbidden"}))
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), expected, "{permission} at {path}");
        }
    }

    let (_, key) = seed_key(
        &state,
        &user,
        KeyPermissions {
            custom_permissions: vec!["api_keys.write".to_string()],
            ..Default::default()
        },
    )
    .await;
    for (path, expected) in [
        (format!("/v1/keys/{target_id}"), StatusCode::OK),
        (format!("/v1/keys/{foreign_id}"), StatusCode::FORBIDDEN),
    ] {
        let response = test::call_service(
            &app,
            test::TestRequest::put()
                .uri(&path)
                .insert_header(("x-api-key", key.clone()))
                .set_json(json!({"name": "authorized update"}))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), expected);
    }
    for (path, body) in [
        ("/v1/keys".to_string(), json!({"name": "authorized child"})),
        (format!("/v1/keys/{target_id}/rotate"), json!({})),
    ] {
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&path)
                .insert_header(("x-api-key", key.clone()))
                .set_json(body)
                .to_request(),
        )
        .await;
        assert!(response.status().is_success(), "{path}");
    }
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/keys")
            .insert_header(("x-api-key", key))
            .set_json(json!({"name": "admin child", "permissions": {"is_admin": true}}))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let (revocable_id, _) = seed_key(&state, &user, KeyPermissions::default()).await;
    let (_, key) = seed_key(
        &state,
        &user,
        KeyPermissions {
            custom_permissions: vec!["api_keys.delete".to_string()],
            ..Default::default()
        },
    )
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/v1/keys/{revocable_id}"))
            .insert_header(("x-api-key", key))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[actix_web::test]
async fn scoped_admin_management_credentials_cannot_inherit_global_target_scope() {
    let state = auth_enabled_test_state().await;
    let admin = seed_user(&state, UserRole::Admin).await;
    let foreign = seed_user(&state, UserRole::User).await;
    let (foreign_id, _) = seed_key(&state, &foreign, KeyPermissions::default()).await;
    let (unowned_id, _) = state
        .key_manager
        .generate_key(CreateKeyConfig {
            name: "unowned target".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let (_, credential) = seed_key(
        &state,
        &admin,
        KeyPermissions {
            custom_permissions: vec![
                "api_keys.read".into(),
                "api_keys.write".into(),
                "api_keys.delete".into(),
            ],
            ..Default::default()
        },
    )
    .await;
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .wrap(AuthMiddleware)
            .configure(configure_routes),
    )
    .await;

    let count = state.key_manager.count_keys(None).await.unwrap();
    for scope in [
        json!({"user_id": foreign.id()}),
        json!({"team_id": Uuid::new_v4()}),
    ] {
        let mut body = scope;
        body["name"] = json!("foreign child");
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/keys")
                .insert_header(("x-api-key", credential.clone()))
                .set_json(body)
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    for id in [foreign_id, unowned_id] {
        let before = state.key_manager.get_key(id).await.unwrap().unwrap();
        for (method, path) in [
            (Method::GET, format!("/v1/keys/{id}")),
            (Method::PUT, format!("/v1/keys/{id}")),
            (Method::POST, format!("/v1/keys/{id}/rotate")),
            (Method::DELETE, format!("/v1/keys/{id}")),
        ] {
            let response = test::call_service(
                &app,
                test::TestRequest::default()
                    .method(method.clone())
                    .uri(&path)
                    .insert_header(("x-api-key", credential.clone()))
                    .set_json(json!({"name": "unauthorized change"}))
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::FORBIDDEN, "{method} {path}");
        }
        let after = state.key_manager.get_key(id).await.unwrap().unwrap();
        assert_eq!(after.status, KeyStatus::Active);
        assert_eq!(after.name, before.name);
    }
    for path in [
        "/v1/keys".to_string(),
        format!("/v1/keys?user_id={}", foreign.id()),
        format!("/v1/keys?team_id={}", Uuid::new_v4()),
    ] {
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&path)
                .insert_header(("x-api-key", credential.clone()))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{path}");
    }
    assert_eq!(
        state.key_manager.count_keys(None).await.unwrap(),
        count,
        "denied create/rotate must not create credentials"
    );

    // Omitting create scope retains the ordinary self default, never a
    // globally unowned target inherited from the administrator's role.
    let own_count = state
        .key_manager
        .list_user_keys(admin.id())
        .await
        .unwrap()
        .len();
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/keys")
            .insert_header(("x-api-key", credential.clone()))
            .set_json(json!({"name": "own child"}))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        state
            .key_manager
            .list_user_keys(admin.id())
            .await
            .unwrap()
            .len(),
        own_count + 1
    );
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/v1/keys?user_id={}", admin.id()))
            .insert_header(("x-api-key", credential.clone()))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    for method in [Method::GET, Method::PUT, Method::POST, Method::DELETE] {
        let (id, _) = seed_key(&state, &admin, KeyPermissions::default()).await;
        let path = if method == Method::POST {
            format!("/v1/keys/{id}/rotate")
        } else {
            format!("/v1/keys/{id}")
        };
        let response = test::call_service(
            &app,
            test::TestRequest::default()
                .method(method.clone())
                .uri(&path)
                .insert_header(("x-api-key", credential.clone()))
                .set_json(json!({"name": "own update"}))
                .to_request(),
        )
        .await;
        assert!(response.status().is_success(), "self {method} {path}");
    }
}

#[actix_web::test]
async fn logged_in_users_keep_their_existing_key_management_workflow() {
    let state = auth_enabled_test_state().await;
    let user = seed_user(&state, UserRole::User).await;
    let token = state
        .auth
        .jwt()
        .create_access_token(user.id(), "user".to_string(), vec![], None, None)
        .await
        .unwrap();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::clone(&state))
            .wrap(AuthMiddleware)
            .configure(configure_routes),
    )
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/keys")
            .insert_header(("authorization", format!("Bearer {token}")))
            .set_json(json!({"name": "session-created key"}))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let keys = state.key_manager.list_user_keys(user.id()).await.unwrap();
    assert_eq!(keys.len(), 1);
    let response = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/v1/keys/{}", keys[0].id))
            .insert_header(("authorization", format!("Bearer {token}")))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[actix_web::test]
async fn management_key_grants_require_both_owner_and_credential_admin_authority() {
    let state = auth_enabled_test_state().await;
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .wrap(AuthMiddleware)
            .configure(configure_routes),
    )
    .await;
    for role in [UserRole::Admin, UserRole::User] {
        let user = seed_user(&state, role.clone()).await;
        for (caller_permissions, credential_admin) in [
            (
                KeyPermissions {
                    custom_permissions: vec!["api_keys.write".into()],
                    ..Default::default()
                },
                false,
            ),
            (
                KeyPermissions {
                    custom_permissions: vec!["api_keys.write".into(), "use:api".into()],
                    ..Default::default()
                },
                false,
            ),
            (
                KeyPermissions {
                    custom_permissions: vec!["api_keys.write".into(), "api.system.admin".into()],
                    ..Default::default()
                },
                false,
            ),
            (KeyPermissions::admin(), true),
            (
                KeyPermissions {
                    custom_permissions: vec!["system.admin".into()],
                    ..Default::default()
                },
                true,
            ),
            (
                KeyPermissions {
                    custom_permissions: vec!["*".into()],
                    ..Default::default()
                },
                true,
            ),
        ] {
            let may_grant = role == UserRole::Admin && credential_admin;
            let (_, credential) = seed_key(&state, &user, caller_permissions).await;
            for permissions in [
                json!({"is_admin": true}),
                json!({"custom_permissions": ["system.admin"]}),
                json!({"custom_permissions": ["*"]}),
                json!({"custom_permissions": ["api.keys.list_all"]}),
                json!({"custom_permissions": ["users.manage"]}),
            ] {
                let (target_id, _) = seed_key(&state, &user, KeyPermissions::default()).await;
                for (method, path) in [
                    (Method::POST, "/v1/keys".to_string()),
                    (Method::PUT, format!("/v1/keys/{target_id}")),
                ] {
                    let response = test::call_service(
                        &app,
                        test::TestRequest::default()
                            .method(method.clone())
                            .uri(&path)
                            .insert_header(("x-api-key", credential.clone()))
                            .set_json(
                                json!({"name":"delegated management", "permissions":permissions}),
                            )
                            .to_request(),
                    )
                    .await;
                    if may_grant {
                        assert!(
                            response.status().is_success(),
                            "{role:?} {method} {permissions}: {} {:?}",
                            response.status(),
                            test::read_body(response).await
                        );
                    } else {
                        assert_eq!(
                            response.status(),
                            StatusCode::FORBIDDEN,
                            "{role:?} {method} {permissions}"
                        );
                    }
                }
                let (management_id, _) = seed_key(
                    &state,
                    &user,
                    serde_json::from_value(permissions.clone()).unwrap(),
                )
                .await;
                let response = test::call_service(
                    &app,
                    test::TestRequest::post()
                        .uri(&format!("/v1/keys/{management_id}/rotate"))
                        .insert_header(("x-api-key", credential.clone()))
                        .to_request(),
                )
                .await;
                if may_grant {
                    assert!(response.status().is_success(), "management rotation");
                } else {
                    assert_eq!(response.status(), StatusCode::FORBIDDEN);
                    assert_eq!(
                        state
                            .key_manager
                            .get_key(management_id)
                            .await
                            .unwrap()
                            .unwrap()
                            .status,
                        KeyStatus::Active,
                        "denied rotation must not revoke the target"
                    );
                }
            }
            // A write-only automation credential still manages ordinary keys.
            let (ordinary_id, _) = seed_key(&state, &user, KeyPermissions::default()).await;
            for (method, path, body) in [
                (
                    Method::POST,
                    "/v1/keys".to_string(),
                    json!({"name":"ordinary child"}),
                ),
                (
                    Method::PUT,
                    format!("/v1/keys/{ordinary_id}"),
                    json!({"name":"ordinary update"}),
                ),
                (
                    Method::POST,
                    format!("/v1/keys/{ordinary_id}/rotate"),
                    json!({}),
                ),
            ] {
                let response = test::call_service(
                    &app,
                    test::TestRequest::default()
                        .method(method)
                        .uri(&path)
                        .insert_header(("x-api-key", credential.clone()))
                        .set_json(body)
                        .to_request(),
                )
                .await;
                assert!(response.status().is_success(), "ordinary key {path}");
            }
        }
    }
}
