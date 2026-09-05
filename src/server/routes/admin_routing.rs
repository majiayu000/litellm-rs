//! Read-only admin inventory of the live routing snapshot.
//!
//! The response DTO is a public-field projection: API keys, auth headers,
//! secrets, and raw provider config are not fields and cannot be serialized.

use super::admin::require_admin;
use crate::config::models::provider::ProviderConfig;
use crate::core::providers::is_provider_selector_supported;
use crate::core::providers::registry::{ProviderDispatchKind, entry_for_name};
use crate::core::router::deployment::current_timestamp;
use crate::core::router::{Deployment, HealthStatus, RoutingSnapshot};
use crate::core::types::model::ProviderCapability;
use crate::server::state::AppState;
use actix_web::{HttpRequest, HttpResponse, web};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering::Relaxed;

const ADMIN_ERROR: &str = "Admin role required for routing inventory";

/// `GET /admin/routing/inventory`
pub(super) async fn routing_inventory(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> actix_web::Result<HttpResponse> {
    if let Some(forbidden) = require_admin(&req, &state, "inspect routing inventory", ADMIN_ERROR) {
        return Ok(forbidden);
    }

    let snapshot = state.unified_router.load_routing_snapshot();
    let cfg = state.config.load();
    Ok(HttpResponse::Ok().json(build_inventory(&snapshot, cfg.providers())))
}

pub(super) fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/admin/routing").route("/inventory", web::get().to(routing_inventory)));
}

/// Sanitized inventory of effective routing state.
#[derive(Debug, Serialize, PartialEq)]
pub struct RoutingInventory {
    pub success: bool,
    pub snapshot_generation: u64,
    pub models: Vec<PublicModelInventory>,
    pub unavailable_providers: Vec<UnavailableProviderInventory>,
}

/// Public model group and the deployments that can serve it.
#[derive(Debug, Serialize, PartialEq)]
pub struct PublicModelInventory {
    pub public_model: String,
    pub aliases: Vec<String>,
    pub deployments: Vec<DeploymentInventory>,
}

/// One deployment's live routing state. Public fields only.
#[derive(Debug, Serialize, PartialEq)]
pub struct DeploymentInventory {
    pub provider: String,
    pub deployment: String,
    pub public_model: String,
    pub model: String,
    pub capabilities: Vec<ProviderCapability>,
    pub health: InventoryHealth,
    pub available: bool,
    pub unavailable_reasons: Vec<UnavailableReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooldown: Option<CooldownInventory>,
    pub rpm: RateWindowInventory,
    pub tpm: RateWindowInventory,
    pub active_requests: u32,
}

/// Configured provider that is not present in the live routing snapshot.
#[derive(Debug, Serialize, PartialEq)]
pub struct UnavailableProviderInventory {
    pub provider: String,
    pub provider_type: String,
    pub public_models: Vec<String>,
    pub available: bool,
    pub unavailable_reasons: Vec<UnavailableReason>,
}

/// Probe-derived health. Missing evidence is `unknown`, never `healthy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InventoryHealth {
    Unknown,
    Healthy,
    Degraded,
    Unhealthy,
}

/// Why a deployment or configured provider cannot serve traffic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnavailableReason {
    Cooldown,
    Unhealthy,
    RpmLimitReached,
    TpmLimitReached,
    ParallelLimitReached,
    FeatureGated,
    Unavailable,
}

/// Cooldown remaining from live `cooldown_until`. Omitted when not cooling down.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct CooldownInventory {
    pub until_unix_secs: u64,
    pub remaining_secs: u64,
}

/// Configured limit versus live per-minute usage. `configured_limit` is `null`
/// when unlimited; `current_usage` is omitted when this minute has no data.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RateWindowInventory {
    pub configured_limit: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_usage: Option<u64>,
}

fn build_inventory(snapshot: &RoutingSnapshot, providers: &[ProviderConfig]) -> RoutingInventory {
    let aliases_by_model = aliases_by_public_model(snapshot);
    let mut models: Vec<PublicModelInventory> = snapshot
        .model_order
        .iter()
        .filter_map(|public_model| {
            let ids = snapshot.model_index.get(public_model)?;
            let mut deployments: Vec<DeploymentInventory> = ids
                .iter()
                .filter_map(|id| snapshot.deployments.get(id))
                .map(|deployment| project_deployment(snapshot, deployment))
                .collect();
            if deployments.is_empty() {
                return None;
            }
            deployments.sort_by(|left, right| left.deployment.cmp(&right.deployment));
            Some(PublicModelInventory {
                public_model: public_model.clone(),
                aliases: aliases_by_model
                    .get(public_model)
                    .cloned()
                    .unwrap_or_default(),
                deployments,
            })
        })
        .collect();
    models.sort_by(|left, right| left.public_model.cmp(&right.public_model));

    RoutingInventory {
        success: true,
        snapshot_generation: snapshot.generation(),
        models,
        unavailable_providers: unavailable_configured_providers(snapshot, providers),
    }
}

fn aliases_by_public_model(snapshot: &RoutingSnapshot) -> HashMap<String, Vec<String>> {
    let mut aliases: HashMap<String, Vec<String>> = HashMap::new();
    for alias in snapshot.model_aliases.keys() {
        let target = snapshot.resolve_model_name(alias);
        aliases.entry(target).or_default().push(alias.clone());
    }
    for names in aliases.values_mut() {
        names.sort();
    }
    aliases
}

fn project_deployment(snapshot: &RoutingSnapshot, deployment: &Deployment) -> DeploymentInventory {
    let now = current_timestamp();
    let in_cooldown = deployment.is_in_cooldown();
    let minute = deployment.state.minute_counters(now);
    let active_requests = deployment.state.active_requests.load(Relaxed);
    let mut unavailable_reasons = Vec::new();

    if in_cooldown {
        unavailable_reasons.push(UnavailableReason::Cooldown);
    }
    if !deployment.is_healthy() {
        match deployment.state.health_status() {
            HealthStatus::Unknown => {}
            HealthStatus::Healthy | HealthStatus::Degraded => {}
            HealthStatus::Unhealthy | HealthStatus::Cooldown => {
                unavailable_reasons.push(UnavailableReason::Unhealthy);
            }
        }
    }
    if let Some(limit) = deployment.config.max_parallel_requests
        && active_requests >= limit
    {
        unavailable_reasons.push(UnavailableReason::ParallelLimitReached);
    }
    if let Some(limit) = deployment.config.rpm_limit
        && minute.rpm >= limit
    {
        unavailable_reasons.push(UnavailableReason::RpmLimitReached);
    }
    if let Some(limit) = deployment.config.tpm_limit
        && minute.tpm >= limit
    {
        unavailable_reasons.push(UnavailableReason::TpmLimitReached);
    }

    let available = unavailable_reasons.is_empty() && deployment.is_healthy() && !in_cooldown;

    DeploymentInventory {
        provider: snapshot
            .provider_names
            .get(&deployment.id)
            .cloned()
            .unwrap_or_else(|| deployment.provider.name().to_string()),
        deployment: deployment.id.clone(),
        public_model: deployment.model_name.clone(),
        model: deployment.model.clone(),
        capabilities: deployment_capabilities(deployment),
        health: inventory_health(deployment.state.probe_health_status()),
        available,
        unavailable_reasons,
        cooldown: cooldown_inventory(deployment, now, in_cooldown),
        rpm: rate_window(
            deployment.config.rpm_limit,
            minute.rpm,
            has_minute_usage(deployment),
        ),
        tpm: rate_window(
            deployment.config.tpm_limit,
            minute.tpm,
            has_minute_usage(deployment),
        ),
        active_requests,
    }
}

fn deployment_capabilities(deployment: &Deployment) -> Vec<ProviderCapability> {
    deployment
        .provider
        .capabilities()
        .iter()
        .filter(|capability| {
            deployment
                .provider
                .supports_capability_for_model(&deployment.model, capability)
        })
        .cloned()
        .collect()
}

fn inventory_health(status: HealthStatus) -> InventoryHealth {
    match status {
        HealthStatus::Unknown => InventoryHealth::Unknown,
        HealthStatus::Healthy => InventoryHealth::Healthy,
        HealthStatus::Degraded => InventoryHealth::Degraded,
        HealthStatus::Unhealthy | HealthStatus::Cooldown => InventoryHealth::Unhealthy,
    }
}

fn cooldown_inventory(
    deployment: &Deployment,
    now: u64,
    in_cooldown: bool,
) -> Option<CooldownInventory> {
    if !in_cooldown {
        return None;
    }
    let until = deployment.state.cooldown_until.load(Relaxed);
    if until == 0 {
        return None;
    }
    Some(CooldownInventory {
        until_unix_secs: until,
        remaining_secs: until.saturating_sub(now),
    })
}

fn has_minute_usage(deployment: &Deployment) -> bool {
    deployment.state.last_request_at.load(Relaxed) > 0
        || deployment.state.total_requests.load(Relaxed) > 0
}

fn rate_window(
    configured_limit: Option<u64>,
    current: u64,
    has_usage: bool,
) -> RateWindowInventory {
    RateWindowInventory {
        configured_limit,
        current_usage: has_usage.then_some(current),
    }
}

fn unavailable_configured_providers(
    snapshot: &RoutingSnapshot,
    providers: &[ProviderConfig],
) -> Vec<UnavailableProviderInventory> {
    let routed: HashSet<&str> = snapshot
        .provider_names
        .values()
        .map(String::as_str)
        .collect();
    let mut unavailable = Vec::new();
    for provider in providers {
        if !provider.enabled || routed.contains(provider.name.as_str()) {
            continue;
        }
        let selector = if provider.provider_type.trim().is_empty() {
            provider.name.as_str()
        } else {
            provider.provider_type.as_str()
        };
        unavailable.push(UnavailableProviderInventory {
            provider: provider.name.clone(),
            provider_type: provider.provider_type.clone(),
            public_models: provider.models.clone(),
            available: false,
            unavailable_reasons: vec![configured_provider_reason(selector)],
        });
    }
    unavailable.sort_by(|left, right| left.provider.cmp(&right.provider));
    unavailable
}

fn configured_provider_reason(selector: &str) -> UnavailableReason {
    if is_provider_selector_supported(selector) {
        return UnavailableReason::Unavailable;
    }
    match entry_for_name(selector) {
        Some(entry) if entry.dispatch_kind == ProviderDispatchKind::UnsupportedEnum => {
            UnavailableReason::FeatureGated
        }
        _ => UnavailableReason::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::core::models::{
        ApiKey, Metadata, UsageStats,
        user::{
            preferences::UserPreferences,
            types::{User, UserProfile, UserRole, UserStatus},
        },
    };
    use crate::core::providers::Provider;
    use crate::core::providers::openai::OpenAIProvider;
    use crate::core::router::{Deployment, DeploymentConfig};
    use crate::server::HttpServer;
    use actix_web::dev::Service;
    use actix_web::{App, HttpMessage, http::StatusCode, test as actix_test};
    use std::collections::BTreeSet;
    use std::sync::atomic::Ordering;

    const SECRET: &str = "sk-inventory-redaction-secret-do-not-leak";

    fn base_test_config(auth_enabled: bool) -> Config {
        let mut config = Config::default();
        config.gateway.auth.enable_jwt = auth_enabled;
        config.gateway.auth.enable_api_key = auth_enabled;
        config.gateway.auth.allow_anonymous = !auth_enabled;
        config.gateway.auth.jwt_secret = "AaaAaaAaaAaaAaaAaaAaaAaaAaaAaa1!".to_string();
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = None;
        config.gateway.providers.push(ProviderConfig {
            name: "bootstrap".to_string(),
            provider_type: "openai".to_string(),
            api_key: "sk-test".to_string(),
            enabled: false,
            models: vec!["bootstrap-model".to_string()],
            ..ProviderConfig::default()
        });
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

    fn make_api_key() -> ApiKey {
        ApiKey {
            metadata: Metadata::new(),
            name: "route-key".to_string(),
            key_hash: "hashed".to_string(),
            key_prefix: "gw-ab".to_string(),
            user_id: None,
            team_id: None,
            permissions: vec!["api.chat".to_string()],
            rate_limits: None,
            expires_at: None,
            is_active: true,
            last_used_at: None,
            usage_stats: UsageStats::default(),
        }
    }

    async fn admin_app(
        state: web::Data<AppState>,
        user: Option<User>,
        api_key: Option<ApiKey>,
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
                    if let Some(api_key) = api_key.clone() {
                        req.extensions_mut().insert(api_key);
                    }
                    srv.call(req)
                })
                .configure(crate::server::routes::admin::configure_routes),
        )
        .await
    }

    async fn get_inventory(
        app: &impl Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
    ) -> (StatusCode, serde_json::Value) {
        let req = actix_test::TestRequest::get()
            .uri("/admin/routing/inventory")
            .to_request();
        let resp = actix_test::call_service(app, req).await;
        let status = resp.status();
        let body = actix_test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or_else(|_| {
            panic!(
                "inventory response should be JSON, got {}",
                String::from_utf8_lossy(&body)
            )
        });
        (status, json)
    }

    async fn openai_deployment(id: &str, public_model: &str, wire_model: &str) -> Deployment {
        let provider = Provider::OpenAI(
            OpenAIProvider::with_api_key(SECRET)
                .await
                .expect("test provider"),
        );
        Deployment::new(
            id.to_string(),
            provider,
            wire_model.to_string(),
            public_model.to_string(),
        )
    }

    fn json_keys(value: &serde_json::Value) -> BTreeSet<String> {
        let mut keys = BTreeSet::new();
        collect_keys(value, &mut keys);
        keys
    }

    fn collect_keys(value: &serde_json::Value, keys: &mut BTreeSet<String>) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, child) in map {
                    keys.insert(key.clone());
                    collect_keys(child, keys);
                }
            }
            serde_json::Value::Array(items) => {
                for child in items {
                    collect_keys(child, keys);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn pydantic_ai_is_reported_as_feature_gated() {
        assert_eq!(
            configured_provider_reason("pydantic_ai"),
            UnavailableReason::FeatureGated
        );
        assert_ne!(
            configured_provider_reason("openai"),
            UnavailableReason::FeatureGated
        );
        assert_eq!(
            configured_provider_reason("totally_unknown_provider"),
            UnavailableReason::Unavailable
        );
    }

    #[test]
    fn unknown_probe_health_is_not_healthy() {
        assert_eq!(
            inventory_health(HealthStatus::Unknown),
            InventoryHealth::Unknown
        );
        assert_eq!(
            inventory_health(HealthStatus::Unhealthy),
            InventoryHealth::Unhealthy
        );
        assert_ne!(
            inventory_health(HealthStatus::Unknown),
            InventoryHealth::Healthy
        );
    }

    #[actix_web::test]
    async fn inventory_requires_admin_identity() {
        let state = test_state(base_test_config(true)).await;
        let app = admin_app(state, None, None).await;
        let (status, body) = get_inventory(&app).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["success"], false);
        assert_eq!(body["error"], ADMIN_ERROR);
    }

    #[actix_web::test]
    async fn inventory_rejects_non_admin_user() {
        let state = test_state(base_test_config(true)).await;
        let app = admin_app(state, Some(make_test_user(UserRole::User)), None).await;
        let (status, _) = get_inventory(&app).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    async fn inventory_rejects_api_key_without_admin_user() {
        let state = test_state(base_test_config(true)).await;
        let app = admin_app(state, None, Some(make_api_key())).await;
        let (status, _) = get_inventory(&app).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    async fn inventory_empty_state() {
        let state = test_state(base_test_config(true)).await;
        let app = admin_app(state, Some(make_test_user(UserRole::Admin)), None).await;
        let (status, body) = get_inventory(&app).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["success"], true);
        assert_eq!(body["models"], serde_json::json!([]));
        assert_eq!(body["unavailable_providers"], serde_json::json!([]));
        assert!(body["snapshot_generation"].as_u64().is_some());
    }

    #[actix_web::test]
    async fn inventory_redacts_secrets_and_raw_config() {
        let state = test_state(base_test_config(true)).await;
        state.unified_router.add_deployment(
            openai_deployment("secret-dep", "gpt-4", "gpt-4-turbo")
                .await
                .with_config(DeploymentConfig {
                    rpm_limit: Some(50),
                    tpm_limit: Some(10_000),
                    ..DeploymentConfig::default()
                }),
        );

        let mut cfg = (*state.config.load()).clone();
        cfg.gateway.providers.push(ProviderConfig {
            name: "hidden-ollama".to_string(),
            provider_type: "pydantic_ai".to_string(),
            api_key: SECRET.to_string(),
            models: vec!["llama3".to_string()],
            settings: [(
                "headers".to_string(),
                serde_json::json!({"Authorization": format!("Bearer {SECRET}")}),
            )]
            .into_iter()
            .collect(),
            ..ProviderConfig::default()
        });
        state.config.store(cfg);

        let app = admin_app(state, Some(make_test_user(UserRole::Admin)), None).await;
        let (status, body) = get_inventory(&app).await;
        assert_eq!(status, StatusCode::OK);

        let encoded = body.to_string();
        assert!(
            !encoded.contains(SECRET),
            "inventory leaked secret material: {encoded}"
        );
        let keys = json_keys(&body);
        for forbidden in [
            "api_key",
            "api_token",
            "authorization",
            "Authorization",
            "headers",
            "custom_headers",
            "settings",
            "password",
            "secret",
            "credential",
            "key_hash",
        ] {
            assert!(
                !keys.contains(forbidden),
                "inventory serialized forbidden key {forbidden}: {keys:?}"
            );
        }
        assert_eq!(
            body["models"][0]["deployments"][0]["rpm"]["configured_limit"],
            50
        );
        assert!(body["models"][0]["deployments"][0]["rpm"]["current_usage"].is_null());
    }

    #[actix_web::test]
    async fn inventory_reports_aliases_and_multi_deployment() {
        let state = test_state(base_test_config(true)).await;
        let mut primary = openai_deployment("openai-a", "gpt-4", "gpt-4-turbo").await;
        primary.config.rpm_limit = Some(100);
        primary.config.tpm_limit = Some(1_000);
        let secondary = openai_deployment("openai-b", "gpt-4", "gpt-4-turbo").await;
        state.unified_router.add_deployment(primary);
        state.unified_router.add_deployment(secondary);
        state
            .unified_router
            .add_model_alias("gpt4", "gpt-4")
            .expect("alias");
        state.unified_router.record_success("openai-a", 25, 1_000);

        let app = admin_app(state, Some(make_test_user(UserRole::Admin)), None).await;
        let (status, body) = get_inventory(&app).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["models"].as_array().map(Vec::len), Some(1));
        assert_eq!(body["models"][0]["public_model"], "gpt-4");
        assert_eq!(body["models"][0]["aliases"], serde_json::json!(["gpt4"]));
        let deployments = body["models"][0]["deployments"].as_array().unwrap();
        assert_eq!(deployments.len(), 2);
        assert_eq!(deployments[0]["deployment"], "openai-a");
        assert_eq!(deployments[1]["deployment"], "openai-b");
        assert_eq!(deployments[0]["rpm"]["configured_limit"], 100);
        assert_eq!(deployments[0]["rpm"]["current_usage"], 1);
        assert_eq!(deployments[0]["tpm"]["configured_limit"], 1_000);
        assert_eq!(deployments[0]["tpm"]["current_usage"], 25);
        assert!(deployments[1]["rpm"]["current_usage"].is_null());
        assert!(deployments[0]["capabilities"].as_array().is_some());
    }

    #[actix_web::test]
    async fn inventory_distinguishes_unknown_health_from_unhealthy() {
        let state = test_state(base_test_config(true)).await;
        let unknown = openai_deployment("dep-unknown", "gpt-4", "gpt-4-turbo").await;
        let unhealthy = openai_deployment("dep-unhealthy", "gpt-4", "gpt-4-turbo").await;
        state.unified_router.add_deployment(unknown);
        state.unified_router.add_deployment(unhealthy);
        let stored = state
            .unified_router
            .get_deployment("dep-unhealthy")
            .expect("unhealthy deployment");
        stored
            .state
            .health
            .store(HealthStatus::Unhealthy as u8, Ordering::Relaxed);
        stored
            .state
            .set_probe_health_status(HealthStatus::Unhealthy);

        let app = admin_app(state, Some(make_test_user(UserRole::Admin)), None).await;
        let (status, body) = get_inventory(&app).await;
        assert_eq!(status, StatusCode::OK);
        let deployments = body["models"][0]["deployments"].as_array().unwrap();
        let unknown = deployments
            .iter()
            .find(|row| row["deployment"] == "dep-unknown")
            .unwrap();
        let unhealthy = deployments
            .iter()
            .find(|row| row["deployment"] == "dep-unhealthy")
            .unwrap();
        assert_eq!(unknown["health"], "unknown");
        assert_ne!(unknown["health"], "healthy");
        assert_eq!(unknown["available"], true);
        assert_eq!(unhealthy["health"], "unhealthy");
        assert_eq!(unhealthy["available"], false);
        assert!(
            unhealthy["unavailable_reasons"]
                .as_array()
                .unwrap()
                .iter()
                .any(|reason| reason == "unhealthy")
        );
    }

    #[actix_web::test]
    async fn inventory_reports_feature_gated_provider_unavailable() {
        let state = test_state(base_test_config(true)).await;
        let mut cfg = (*state.config.load()).clone();
        cfg.gateway.providers.push(ProviderConfig {
            name: "offline-pydantic".to_string(),
            provider_type: "pydantic_ai".to_string(),
            api_key: SECRET.to_string(),
            models: vec!["agent-model".to_string()],
            ..ProviderConfig::default()
        });
        state.config.store(cfg);

        let app = admin_app(state, Some(make_test_user(UserRole::Admin)), None).await;
        let (status, body) = get_inventory(&app).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body["unavailable_providers"][0]["provider"],
            "offline-pydantic"
        );
        assert_eq!(body["unavailable_providers"][0]["available"], false);
        assert_eq!(
            body["unavailable_providers"][0]["unavailable_reasons"],
            serde_json::json!(["feature_gated"])
        );
        assert_eq!(
            body["unavailable_providers"][0]["public_models"],
            serde_json::json!(["agent-model"])
        );
        assert!(!body.to_string().contains(SECRET));
    }
}
