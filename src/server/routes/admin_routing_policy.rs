//! Admin-only routing policy get/update.
//!
//! Mutations clone the pinned config, apply through [`AppState::apply_runtime`],
//! then persist a sanitized revision. Only existing finite router, alias, and
//! named-provider routing fields are accepted.

use super::admin::require_admin;
use crate::config::Config;
use crate::config::models::gateway::GatewayConfig;
use crate::config::models::provider::RetryConfig;
use crate::config::models::router::{
    CircuitBreakerConfig, LoadBalancerConfig, RoutingStrategyConfig,
};
use crate::core::audit::{AuditEvent, UserAction};
use crate::core::models::user::types::User;
use crate::core::router::RoutingSnapshot;
use crate::server::state::AppState;
use actix_web::{HttpMessage, HttpRequest, HttpResponse, web};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use tracing::warn;

const ADMIN_ERROR: &str = "Admin role required for routing policy administration";

#[derive(Debug, Serialize, Clone)]
struct ProviderRoutingView {
    weight: f32,
    priority: u32,
    max_retries: u32,
    retry: RetryConfig,
}

#[derive(Debug, Serialize, Clone)]
struct RoutingPolicyView {
    strategy: RoutingStrategyConfig,
    circuit_breaker: CircuitBreakerConfig,
    load_balancer: LoadBalancerConfig,
    model_aliases: BTreeMap<String, String>,
    providers: BTreeMap<String, ProviderRoutingView>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProviderRoutingPatch {
    #[serde(default)]
    weight: Option<f32>,
    #[serde(default)]
    priority: Option<u32>,
    #[serde(default)]
    max_retries: Option<u32>,
    #[serde(default)]
    retry: Option<RetryConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RoutingPolicyUpdate {
    #[serde(default)]
    strategy: Option<RoutingStrategyConfig>,
    #[serde(default)]
    circuit_breaker: Option<CircuitBreakerConfig>,
    #[serde(default)]
    load_balancer: Option<LoadBalancerConfig>,
    #[serde(default)]
    model_aliases: Option<HashMap<String, String>>,
    #[serde(default)]
    providers: Option<HashMap<String, ProviderRoutingPatch>>,
}

#[derive(Debug, Serialize)]
struct RoutingPolicyResponse {
    success: bool,
    generation: u64,
    policy: RoutingPolicyView,
}

pub(super) async fn get_routing_policy(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> actix_web::Result<HttpResponse> {
    if let Some(forbidden) = require_admin(&req, &state, "inspect routing policy", ADMIN_ERROR) {
        return Ok(forbidden);
    }

    let runtime = state.pin_runtime();
    Ok(HttpResponse::Ok().json(RoutingPolicyResponse {
        success: true,
        generation: runtime.generation,
        policy: policy_from_config(&runtime.config),
    }))
}

pub(super) async fn put_routing_policy(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<RoutingPolicyUpdate>,
) -> actix_web::Result<HttpResponse> {
    if let Some(forbidden) = require_admin(&req, &state, "update routing policy", ADMIN_ERROR) {
        return Ok(forbidden);
    }
    let actor = actor_id(&req);
    let update = body.into_inner();
    let runtime = state.pin_runtime();
    let mut candidate = (*runtime.config).clone();
    let before = policy_from_config(&candidate);

    if let Some(providers) = &update.providers
        && let Err(message) = apply_provider_patches(&mut candidate, providers)
    {
        return Ok(json_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            &message,
        ));
    }
    if let Some(strategy) = update.strategy {
        candidate.gateway.router.strategy = strategy;
    }
    if let Some(circuit_breaker) = update.circuit_breaker {
        candidate.gateway.router.circuit_breaker = circuit_breaker;
    }
    if let Some(load_balancer) = update.load_balancer {
        candidate.gateway.router.load_balancer = load_balancer;
    }
    if let Some(model_aliases) = update.model_aliases {
        if let Err(message) = reject_unknown_alias_targets(
            &model_aliases,
            &known_canonical_models(&candidate, &runtime.unified_router.load_routing_snapshot()),
        ) {
            return Ok(json_error(
                actix_web::http::StatusCode::BAD_REQUEST,
                &message,
            ));
        }
        candidate.gateway.model_aliases = model_aliases;
    }

    apply_and_persist(&state, candidate, actor, before).await
}

async fn apply_and_persist(
    state: &web::Data<AppState>,
    candidate: Config,
    actor: String,
    before: RoutingPolicyView,
) -> actix_web::Result<HttpResponse> {
    let after = policy_from_config(&candidate);
    match state.apply_runtime(candidate).await {
        Ok(generation) => {
            let payload = json!({
                "diff": {
                    "before": before,
                    "after": after,
                },
                "policy": after,
            });
            if let Err(error) = state
                .storage
                .database
                .insert_routing_policy_revision(generation, &actor, payload.clone())
                .await
            {
                return Ok(json_error(
                    actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("Runtime applied but revision persist failed: {error}"),
                ));
            }
            emit_audit(state, &actor, generation, payload).await;
            Ok(HttpResponse::Ok().json(RoutingPolicyResponse {
                success: true,
                generation,
                policy: after,
            }))
        }
        Err(error) => Ok(apply_failure_response(error)),
    }
}

async fn emit_audit(state: &web::Data<AppState>, actor: &str, generation: u64, payload: Value) {
    let event = routing_policy_audit_event(actor, generation, payload);
    if let Err(error) = state.audit_logger.log(event).await {
        warn!("Failed to record routing policy audit event: {error}");
    }
}

fn routing_policy_audit_event(actor: &str, generation: u64, payload: Value) -> AuditEvent {
    AuditEvent::user_action(actor, UserAction::SettingsChanged)
        .with_metadata("generation", json!(generation))
        .with_metadata("diff", payload.get("diff").cloned().unwrap_or(Value::Null))
        .with_source("admin_routing_policy")
}

fn apply_failure_response(error: crate::utils::error::gateway_error::GatewayError) -> HttpResponse {
    let message = error.to_string();
    let status = match error {
        crate::utils::error::gateway_error::GatewayError::Config(_) => {
            actix_web::http::StatusCode::BAD_REQUEST
        }
        other => actix_web::error::ResponseError::status_code(&other),
    };
    json_error(status, &message)
}

fn json_error(status: actix_web::http::StatusCode, error: &str) -> HttpResponse {
    HttpResponse::build(status).json(json!({
        "success": false,
        "error": error
    }))
}

fn actor_id(req: &HttpRequest) -> String {
    req.extensions()
        .get::<User>()
        .map(|user| user.id().to_string())
        .unwrap_or_else(|| "anonymous".to_string())
}

fn policy_from_config(config: &Config) -> RoutingPolicyView {
    RoutingPolicyView {
        strategy: config.gateway.router.strategy,
        circuit_breaker: config.gateway.router.circuit_breaker.clone(),
        load_balancer: config.gateway.router.load_balancer.clone(),
        model_aliases: config
            .gateway
            .model_aliases
            .iter()
            .map(|(alias, target)| (alias.clone(), target.clone()))
            .collect(),
        providers: config
            .gateway
            .providers
            .iter()
            .map(|provider| {
                (
                    provider.name.clone(),
                    ProviderRoutingView {
                        weight: provider.weight,
                        priority: provider.priority,
                        max_retries: provider.max_retries,
                        retry: provider.retry.clone(),
                    },
                )
            })
            .collect(),
    }
}

fn apply_provider_patches(
    config: &mut Config,
    patches: &HashMap<String, ProviderRoutingPatch>,
) -> Result<(), String> {
    for (name, patch) in patches {
        let Some(provider) = config
            .gateway
            .providers
            .iter_mut()
            .find(|provider| provider.name == *name)
        else {
            return Err(format!("Provider '{name}' was not found"));
        };
        if let Some(weight) = patch.weight {
            provider.weight = weight;
        }
        if let Some(priority) = patch.priority {
            provider.priority = priority;
        }
        if let Some(max_retries) = patch.max_retries {
            provider.max_retries = max_retries;
        }
        if let Some(retry) = &patch.retry {
            provider.retry = retry.clone();
        }
    }
    Ok(())
}

fn known_canonical_models(config: &Config, snapshot: &RoutingSnapshot) -> HashSet<String> {
    let mut known: HashSet<String> = snapshot.model_index.keys().cloned().collect();
    for provider in config
        .gateway
        .providers
        .iter()
        .filter(|provider| provider.enabled)
    {
        known.insert(provider.name.clone());
        known.extend(
            provider
                .models
                .iter()
                .filter(|model| !model.is_empty())
                .cloned(),
        );
    }
    known
}

fn reject_unknown_alias_targets(
    aliases: &HashMap<String, String>,
    known_models: &HashSet<String>,
) -> Result<(), String> {
    GatewayConfig::validate_model_alias_map(aliases)?;
    let mut alias_names: Vec<&str> = aliases.keys().map(String::as_str).collect();
    alias_names.sort_unstable();
    for alias in alias_names {
        if known_models.contains(alias) {
            return Err(format!(
                "Model alias '{alias}' collides with an enabled canonical model"
            ));
        }
        let mut target = &aliases[alias];
        while let Some(next) = aliases.get(target) {
            target = next;
        }
        if !known_models.contains(target) {
            return Err(format!(
                "Model alias '{alias}' references unknown model '{target}'"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::{
        Metadata, UsageStats,
        user::{
            preferences::UserPreferences,
            types::{UserProfile, UserRole, UserStatus},
        },
    };
    use crate::server::HttpServer;
    use actix_web::dev::Service;
    use actix_web::{App, http::StatusCode, test as actix_test};

    fn base_test_config(auth_enabled: bool) -> Config {
        let mut config = crate::server::valid_test_config();
        config.gateway.auth.enable_jwt = auth_enabled;
        config.gateway.auth.enable_api_key = auth_enabled;
        config.gateway.auth.allow_anonymous = !auth_enabled;
        config.gateway.auth.jwt_secret = "AaaAaaAaaAaaAaaAaaAaaAaaAaaAaa1!".to_string();
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = None;
        config.gateway.providers[0].models = vec!["gpt-4o".to_string()];
        config
    }

    async fn test_state(config: Config) -> web::Data<AppState> {
        let server = match HttpServer::new(&config).await {
            Ok(server) => server,
            Err(error) => panic!("server startup failed: {error}"),
        };
        web::Data::new(server.state().clone())
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

    async fn admin_app(
        state: web::Data<AppState>,
        user: Option<User>,
    ) -> impl Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    > {
        actix_test::init_service(
            App::new()
                .app_data(state)
                .wrap_fn(move |req, srv| {
                    if let Some(user) = user.clone() {
                        req.extensions_mut().insert(user);
                    }
                    srv.call(req)
                })
                .configure(crate::server::routes::admin::configure_routes),
        )
        .await
    }

    #[test]
    fn audit_event_metadata_has_actor_generation_and_no_secrets() {
        let payload = json!({
            "diff": {
                "before": {"strategy": "round_robin", "model_aliases": {}},
                "after": {
                    "strategy": "least_busy",
                    "model_aliases": {"prod-chat": "gpt-4o"}
                }
            }
        });
        let event = routing_policy_audit_event("actor-9", 4, payload);
        assert_eq!(event.user_id.as_deref(), Some("actor-9"));
        assert_eq!(event.action, Some(UserAction::SettingsChanged));
        assert_eq!(event.metadata.get("generation"), Some(&json!(4)));
        let serialized = serde_json::to_string(&event).expect("json");
        assert!(!serialized.contains("api_key\":"));
        assert!(!serialized.contains("sk-"));
    }

    #[actix_web::test]
    async fn policy_requires_admin_identity() {
        let state = test_state(base_test_config(true)).await;
        let app = admin_app(state, None).await;
        let resp = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri("/admin/routing/policy")
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let body: Value = actix_test::read_body_json(resp).await;
        assert_eq!(body["success"], false);
        assert_eq!(body["error"], ADMIN_ERROR);
    }

    #[actix_web::test]
    async fn policy_rejects_non_admin_user() {
        let state = test_state(base_test_config(true)).await;
        let app = admin_app(state, Some(make_test_user(UserRole::User))).await;
        let resp = actix_test::call_service(
            &app,
            actix_test::TestRequest::put()
                .uri("/admin/routing/policy")
                .set_json(json!({"strategy": "least_busy"}))
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    async fn get_returns_current_strategy_and_aliases() {
        let mut config = base_test_config(true);
        config
            .gateway
            .model_aliases
            .insert("prod-chat".to_string(), "gpt-4o".to_string());
        config.gateway.router.strategy = RoutingStrategyConfig::LeastBusy;
        let state = test_state(config).await;
        let generation = state.pin_runtime().generation;
        let app = admin_app(state, Some(make_test_user(UserRole::Admin))).await;
        let resp = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri("/admin/routing/policy")
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body: Value = actix_test::read_body_json(resp).await;
        assert_eq!(body["success"], true);
        assert_eq!(body["generation"], generation);
        assert_eq!(body["policy"]["strategy"], "least_busy");
        assert_eq!(body["policy"]["model_aliases"]["prod-chat"], "gpt-4o");
        assert!(body["policy"].get("api_key").is_none());
        assert!(!body.to_string().contains("sk-"));
    }

    #[actix_web::test]
    async fn put_valid_strategy_and_alias_applies() {
        let admin = make_test_user(UserRole::Admin);
        let actor = admin.id().to_string();
        let state = test_state(base_test_config(true)).await;
        let before = state.pin_runtime().generation;
        let app = admin_app(state.clone(), Some(admin)).await;
        let resp = actix_test::call_service(
            &app,
            actix_test::TestRequest::put()
                .uri("/admin/routing/policy")
                .set_json(json!({
                    "strategy": "least_busy",
                    "model_aliases": {"prod-chat": "gpt-4o"}
                }))
                .to_request(),
        )
        .await;
        let status = resp.status();
        let body: Value = actix_test::read_body_json(resp).await;
        assert_eq!(status, StatusCode::OK, "put failed: {body}");
        assert_eq!(body["success"], true);
        assert_eq!(body["generation"], before + 1);
        assert_eq!(body["policy"]["strategy"], "least_busy");
        assert_eq!(body["policy"]["model_aliases"]["prod-chat"], "gpt-4o");

        let live = state.pin_runtime();
        assert_eq!(live.generation, before + 1);
        assert_eq!(
            live.config.gateway.router.strategy,
            RoutingStrategyConfig::LeastBusy
        );
        assert_eq!(
            live.config.gateway.model_aliases.get("prod-chat"),
            Some(&"gpt-4o".to_string())
        );
        assert_eq!(
            live.unified_router.config().routing_strategy,
            RoutingStrategyConfig::LeastBusy
        );

        let revision = state
            .storage
            .database
            .latest_routing_policy_revision()
            .await
            .expect("lookup")
            .expect("revision");
        assert_eq!(revision.generation, (before + 1) as i64);
        assert_eq!(revision.actor, actor);
        let payload = revision.sanitized_payload.to_string();
        assert!(!payload.contains("api_key\":"));
        assert!(!payload.contains("sk-"));
    }

    #[actix_web::test]
    async fn put_alias_to_missing_model_is_rejected() {
        let state = test_state(base_test_config(true)).await;
        let before = state.pin_runtime().generation;
        let app = admin_app(state.clone(), Some(make_test_user(UserRole::Admin))).await;
        let resp = actix_test::call_service(
            &app,
            actix_test::TestRequest::put()
                .uri("/admin/routing/policy")
                .set_json(json!({
                    "model_aliases": {"prod-chat": "missing-model-1271"}
                }))
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body: Value = actix_test::read_body_json(resp).await;
        assert_eq!(body["success"], false);
        assert!(
            body["error"]
                .as_str()
                .unwrap_or_default()
                .contains("unknown model")
        );
        assert_eq!(state.pin_runtime().generation, before);
        assert!(
            state
                .storage
                .database
                .latest_routing_policy_revision()
                .await
                .expect("lookup")
                .is_none()
        );
    }

    #[actix_web::test]
    async fn put_invalid_circuit_breaker_leaves_generation_unchanged() {
        let state = test_state(base_test_config(true)).await;
        let before = state.pin_runtime().generation;
        let app = admin_app(state.clone(), Some(make_test_user(UserRole::Admin))).await;
        let resp = actix_test::call_service(
            &app,
            actix_test::TestRequest::put()
                .uri("/admin/routing/policy")
                .set_json(json!({
                    "circuit_breaker": {"failure_threshold": 0}
                }))
                .to_request(),
        )
        .await;
        assert!(resp.status().is_client_error() || resp.status().is_server_error());
        assert_ne!(resp.status(), StatusCode::OK);
        assert_eq!(state.pin_runtime().generation, before);
        assert!(
            state
                .storage
                .database
                .latest_routing_policy_revision()
                .await
                .expect("lookup")
                .is_none()
        );
    }
}
