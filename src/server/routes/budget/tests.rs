use super::*;
use crate::core::models::{
    ApiKey, Metadata, UsageStats,
    user::{
        preferences::UserPreferences,
        types::{User, UserProfile, UserRole, UserStatus},
    },
};
use actix_web::dev::Service;
use actix_web::{App, HttpMessage, test, web};
use std::sync::Arc;

fn provider_request(provider: &str, max_budget: f64) -> SetProviderBudgetRequest {
    SetProviderBudgetRequest {
        provider: provider.to_string(),
        max_budget,
        reset_period: ResetPeriod::Monthly,
        soft_limit_percentage: 0.8,
        currency: Currency::USD,
        enabled: true,
    }
}

fn model_request(model: &str, max_budget: f64) -> SetModelBudgetRequest {
    SetModelBudgetRequest {
        model: model.to_string(),
        max_budget,
        reset_period: ResetPeriod::Monthly,
        soft_limit_percentage: 0.8,
        currency: Currency::USD,
        enabled: true,
    }
}

fn make_test_user(role: UserRole) -> User {
    User {
        metadata: Metadata::new(),
        username: "testuser".to_string(),
        email: "test@example.com".to_string(),
        display_name: None,
        password_hash: "hash".to_string(),
        role,
        status: UserStatus::Active,
        team_ids: vec![],
        preferences: UserPreferences::default(),
        usage_stats: UsageStats::default(),
        rate_limits: None,
        last_login_at: None,
        email_verified: true,
        two_factor_enabled: false,
        profile: UserProfile::default(),
    }
}

fn make_team_api_key() -> ApiKey {
    ApiKey {
        metadata: Metadata::new(),
        name: "team-key".to_string(),
        key_hash: "hash".to_string(),
        key_prefix: "gw-test".to_string(),
        user_id: None,
        team_id: Some(uuid::Uuid::new_v4()),
        permissions: vec![],
        rate_limits: None,
        expires_at: None,
        is_active: true,
        last_used_at: None,
        usage_stats: UsageStats::default(),
    }
}

fn seeded_budget_limits() -> Arc<UnifiedBudgetLimits> {
    let budget_limits = Arc::new(UnifiedBudgetLimits::new());
    budget_limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    budget_limits
        .models
        .set_model_limit("gpt-4", ModelLimitConfig::new(500.0, ResetPeriod::Monthly));
    budget_limits
}

#[actix_web::test]
async fn test_set_provider_budget() {
    let budget_limits = Arc::new(UnifiedBudgetLimits::new());
    let admin = make_test_user(UserRole::Admin);
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert::<User>(admin.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(budget_limits))
            .configure(configure_budget_routes),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/budget/providers")
        .set_json(provider_request("openai", 1000.0))
        .to_request();

    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn test_set_provider_budget_validation() {
    let budget_limits = Arc::new(UnifiedBudgetLimits::new());
    let admin = make_test_user(UserRole::Admin);
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert::<User>(admin.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(budget_limits))
            .configure(configure_budget_routes),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/budget/providers")
        .set_json(provider_request("", 1000.0))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);

    let req = test::TestRequest::post()
        .uri("/v1/budget/providers")
        .set_json(provider_request("openai", -100.0))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_web::test]
async fn test_list_provider_budgets() {
    let budget_limits = Arc::new(UnifiedBudgetLimits::new());
    budget_limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    budget_limits.providers.set_provider_limit(
        "anthropic",
        ProviderLimitConfig::new(500.0, ResetPeriod::Monthly),
    );

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(budget_limits))
            .configure(configure_budget_routes),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/v1/budget/providers")
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn test_get_provider_budget() {
    let budget_limits = seeded_budget_limits();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(budget_limits))
            .configure(configure_budget_routes),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/v1/budget/providers/openai")
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn test_get_provider_budget_not_found() {
    let budget_limits = Arc::new(UnifiedBudgetLimits::new());
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(budget_limits))
            .configure(configure_budget_routes),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/v1/budget/providers/nonexistent")
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn test_delete_provider_budget() {
    let budget_limits = seeded_budget_limits();
    let admin = make_test_user(UserRole::Admin);
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert::<User>(admin.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(budget_limits))
            .configure(configure_budget_routes),
    )
    .await;

    let req = test::TestRequest::delete()
        .uri("/v1/budget/providers/openai")
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn test_set_model_budget() {
    let budget_limits = Arc::new(UnifiedBudgetLimits::new());
    let admin = make_test_user(UserRole::Admin);
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert::<User>(admin.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(budget_limits))
            .configure(configure_budget_routes),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/budget/models")
        .set_json(model_request("gpt-4", 500.0))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn test_list_model_budgets() {
    let budget_limits = seeded_budget_limits();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(budget_limits))
            .configure(configure_budget_routes),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/v1/budget/models")
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn test_get_budget_summary() {
    let budget_limits = seeded_budget_limits();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(budget_limits))
            .configure(configure_budget_routes),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/v1/budget/summary")
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn test_budget_mutations_forbidden_for_non_admin() {
    for role in [UserRole::User, UserRole::Manager] {
        let role_label = role.to_string();
        let budget_limits = seeded_budget_limits();
        let user = make_test_user(role);
        let app = test::init_service(
            App::new()
                .wrap_fn(move |req, srv| {
                    req.extensions_mut().insert::<User>(user.clone());
                    srv.call(req)
                })
                .app_data(web::Data::new(Arc::clone(&budget_limits)))
                .configure(configure_budget_routes),
        )
        .await;

        let requests = vec![
            test::TestRequest::post()
                .uri("/v1/budget/providers")
                .set_json(provider_request("openai", 1.0))
                .to_request(),
            test::TestRequest::delete()
                .uri("/v1/budget/providers/openai")
                .to_request(),
            test::TestRequest::post()
                .uri("/v1/budget/providers/openai/reset")
                .to_request(),
            test::TestRequest::post()
                .uri("/v1/budget/models")
                .set_json(model_request("gpt-4", 1.0))
                .to_request(),
            test::TestRequest::delete()
                .uri("/v1/budget/models/gpt-4")
                .to_request(),
            test::TestRequest::post()
                .uri("/v1/budget/models/gpt-4/reset")
                .to_request(),
        ];

        for req in requests {
            let path = req.path().to_string();
            let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
            assert_eq!(
                resp.status(),
                403,
                "expected 403 for {} as {}",
                path,
                role_label
            );
        }

        assert!(
            budget_limits
                .providers
                .get_provider_usage("openai")
                .is_some()
        );
        assert!(budget_limits.models.get_model_usage("gpt-4").is_some());
    }
}

#[actix_web::test]
async fn test_budget_mutations_allowed_for_admin() {
    for role in [UserRole::Admin, UserRole::SuperAdmin] {
        let role_label = role.to_string();
        let budget_limits = Arc::new(UnifiedBudgetLimits::new());
        let user = make_test_user(role);
        let app = test::init_service(
            App::new()
                .wrap_fn(move |req, srv| {
                    req.extensions_mut().insert::<User>(user.clone());
                    srv.call(req)
                })
                .app_data(web::Data::new(Arc::clone(&budget_limits)))
                .configure(configure_budget_routes),
        )
        .await;

        let requests = vec![
            test::TestRequest::post()
                .uri("/v1/budget/providers")
                .set_json(provider_request("openai", 1000.0))
                .to_request(),
            test::TestRequest::post()
                .uri("/v1/budget/models")
                .set_json(model_request("gpt-4", 500.0))
                .to_request(),
            test::TestRequest::post()
                .uri("/v1/budget/providers/openai/reset")
                .to_request(),
            test::TestRequest::post()
                .uri("/v1/budget/models/gpt-4/reset")
                .to_request(),
            test::TestRequest::delete()
                .uri("/v1/budget/providers/openai")
                .to_request(),
            test::TestRequest::delete()
                .uri("/v1/budget/models/gpt-4")
                .to_request(),
        ];

        for req in requests {
            let path = req.path().to_string();
            let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
            assert!(
                resp.status().is_success(),
                "expected success for {} as {}",
                path,
                role_label
            );
        }
    }
}

#[actix_web::test]
async fn test_budget_mutations_forbidden_for_team_api_key() {
    let budget_limits = Arc::new(UnifiedBudgetLimits::new());
    let api_key = make_team_api_key();
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert::<ApiKey>(api_key.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(Arc::clone(&budget_limits)))
            .configure(configure_budget_routes),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/budget/providers")
        .set_json(provider_request("openai", 1000.0))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 403);
    assert!(
        budget_limits
            .providers
            .get_provider_usage("openai")
            .is_none()
    );
}

#[actix_web::test]
async fn test_read_endpoints_allowed_for_non_admin() {
    let budget_limits = seeded_budget_limits();
    let user = make_test_user(UserRole::User);
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert::<User>(user.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(Arc::clone(&budget_limits)))
            .configure(configure_budget_routes),
    )
    .await;

    for uri in [
        "/v1/budget/providers",
        "/v1/budget/providers/openai",
        "/v1/budget/models",
        "/v1/budget/models/gpt-4",
        "/v1/budget/summary",
    ] {
        let req = test::TestRequest::get().uri(uri).to_request();
        let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
        assert!(resp.status().is_success(), "expected success for {}", uri);
    }
}
