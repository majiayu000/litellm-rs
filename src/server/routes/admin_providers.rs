//! Admin-only provider create/update/disable/delete.
//!
//! Mutations clone the pinned config, apply through [`AppState::apply_runtime`],
//! then persist a sanitized revision. Secrets are accepted only as `${VAR}`
//! references and are never returned.

use super::admin::require_admin;
use crate::config::Config;
use crate::config::models::provider::ProviderConfig;
use crate::core::audit::{AuditEvent, UserAction};
use crate::core::models::user::types::User;
use crate::core::router::RoutingSnapshot;
use crate::server::state::AppState;
use actix_web::{HttpMessage, HttpRequest, HttpResponse, web};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use tracing::warn;

const ADMIN_ERROR: &str = "Admin role required for provider administration";

#[derive(Debug, Serialize, Clone, PartialEq)]
struct PublicProvider {
    name: String,
    provider_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    base_url: Option<String>,
    enabled: bool,
    models: Vec<String>,
    tags: Vec<String>,
    weight: f32,
    priority: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_key_ref: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderPatch {
    #[serde(default)]
    provider_type: Option<String>,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    base_url: Option<Option<String>>,
    #[serde(default)]
    models: Option<Vec<String>>,
    #[serde(default)]
    tags: Option<Vec<String>>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    weight: Option<f32>,
    #[serde(default)]
    priority: Option<u32>,
    #[serde(default)]
    rpm: Option<u32>,
    #[serde(default)]
    tpm: Option<u32>,
    #[serde(default)]
    settings: Option<HashMap<String, Value>>,
}

#[derive(Debug, Serialize)]
struct ProviderListResponse {
    success: bool,
    generation: u64,
    providers: Vec<PublicProvider>,
}

#[derive(Debug, Serialize)]
struct ProviderMutationResponse {
    success: bool,
    generation: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<PublicProvider>,
}

pub(super) fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/admin/providers")
            .service(
                web::resource("")
                    .route(web::get().to(list_providers))
                    .route(web::post().to(create_provider)),
            )
            .service(
                web::resource("/")
                    .route(web::get().to(list_providers))
                    .route(web::post().to(create_provider)),
            )
            .service(
                web::resource("/{name}")
                    .route(web::patch().to(update_provider))
                    .route(web::delete().to(delete_provider)),
            ),
    );
}

async fn list_providers(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> actix_web::Result<HttpResponse> {
    if let Some(forbidden) = require_admin(&req, &state, "list providers", ADMIN_ERROR) {
        return Ok(forbidden);
    }

    let runtime = state.pin_runtime();
    let refs = api_key_refs_from_revision(
        state
            .storage
            .database
            .latest_provider_config_revision()
            .await
            .ok()
            .flatten()
            .as_ref()
            .map(|row| &row.sanitized_payload),
    );
    Ok(HttpResponse::Ok().json(ProviderListResponse {
        success: true,
        generation: runtime.generation,
        providers: runtime
            .config
            .providers()
            .iter()
            .map(|provider| public_provider(provider, refs.get(&provider.name).cloned()))
            .collect(),
    }))
}

async fn create_provider(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<ProviderConfig>,
) -> actix_web::Result<HttpResponse> {
    if let Some(forbidden) = require_admin(&req, &state, "create provider", ADMIN_ERROR) {
        return Ok(forbidden);
    }
    let actor = actor_id(&req);
    let mut incoming = body.into_inner();
    incoming.name = incoming.name.trim().to_string();
    if incoming.name.is_empty() {
        return Ok(json_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "Provider name cannot be empty",
        ));
    }

    let mut api_key_ref = None;
    if let Err(message) = prepare_provider_secrets(&mut incoming, &mut api_key_ref) {
        return Ok(json_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            &message,
        ));
    }

    let mut candidate = (*state.pin_runtime().config).clone();
    if candidate
        .gateway
        .providers
        .iter()
        .any(|provider| provider.name == incoming.name)
    {
        return Ok(json_error(
            actix_web::http::StatusCode::CONFLICT,
            &format!("Provider '{}' already exists", incoming.name),
        ));
    }

    let before = public_providers(&candidate, &api_key_refs_from_live(&state).await);
    let created_name = incoming.name.clone();
    candidate.gateway.providers.push(incoming);
    apply_and_persist(
        &state,
        candidate,
        actor,
        "create",
        Some(&created_name),
        before,
        api_key_ref,
    )
    .await
}

async fn update_provider(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<ProviderPatch>,
) -> actix_web::Result<HttpResponse> {
    if let Some(forbidden) = require_admin(&req, &state, "update provider", ADMIN_ERROR) {
        return Ok(forbidden);
    }
    let actor = actor_id(&req);
    let name = path.into_inner();
    let patch = body.into_inner();

    let mut candidate = (*state.pin_runtime().config).clone();
    let Some(index) = candidate
        .gateway
        .providers
        .iter()
        .position(|provider| provider.name == name)
    else {
        return Ok(json_error(
            actix_web::http::StatusCode::NOT_FOUND,
            &format!("Provider '{name}' was not found"),
        ));
    };

    let mut refs = api_key_refs_from_live(&state).await;
    let before = public_providers(&candidate, &refs);
    let mut api_key_ref = refs.remove(&name);
    if let Err(message) = apply_patch(
        &mut candidate.gateway.providers[index],
        &patch,
        &mut api_key_ref,
    ) {
        return Ok(json_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            &message,
        ));
    }

    apply_and_persist(
        &state,
        candidate,
        actor,
        "update",
        Some(&name),
        before,
        api_key_ref,
    )
    .await
}

async fn delete_provider(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> actix_web::Result<HttpResponse> {
    if let Some(forbidden) = require_admin(&req, &state, "delete provider", ADMIN_ERROR) {
        return Ok(forbidden);
    }
    let actor = actor_id(&req);
    let name = path.into_inner();

    let runtime = state.pin_runtime();
    let mut candidate = (*runtime.config).clone();
    let Some(index) = candidate
        .gateway
        .providers
        .iter()
        .position(|provider| provider.name == name)
    else {
        return Ok(json_error(
            actix_web::http::StatusCode::NOT_FOUND,
            &format!("Provider '{name}' was not found"),
        ));
    };

    let deleted_models = candidate.gateway.providers[index].models.clone();
    candidate.gateway.providers.remove(index);
    if let Some(message) = delete_conflict(
        &name,
        &deleted_models,
        &candidate.gateway.providers,
        &runtime.unified_router.load_routing_snapshot(),
        &candidate.gateway.model_aliases,
    ) {
        return Ok(json_error(actix_web::http::StatusCode::CONFLICT, &message));
    }

    let before = public_providers(&runtime.config, &api_key_refs_from_live(&state).await);
    apply_and_persist(
        &state,
        candidate,
        actor,
        "delete",
        Some(&name),
        before,
        None,
    )
    .await
}

async fn apply_and_persist(
    state: &web::Data<AppState>,
    candidate: Config,
    actor: String,
    operation: &str,
    focus: Option<&str>,
    before: Vec<PublicProvider>,
    new_api_key_ref: Option<String>,
) -> actix_web::Result<HttpResponse> {
    let mut refs = api_key_refs_from_live(state).await;
    if let (Some(name), Some(env_name)) = (focus, new_api_key_ref) {
        refs.insert(name.to_string(), env_name);
    }
    if operation == "delete"
        && let Some(name) = focus
    {
        refs.remove(name);
    }

    let after = public_providers(&candidate, &refs);
    match state.apply_runtime(candidate).await {
        Ok(generation) => {
            let payload = json!({
                "operation": operation,
                "provider": focus,
                "diff": {
                    "before": before,
                    "after": after,
                },
                "providers": after,
            });
            if let Err(error) = state
                .storage
                .database
                .insert_provider_config_revision(generation, &actor, payload.clone())
                .await
            {
                return Ok(json_error(
                    actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("Runtime applied but revision persist failed: {error}"),
                ));
            }
            emit_audit(state, &actor, generation, operation, payload).await;
            let provider =
                focus.and_then(|name| after.into_iter().find(|provider| provider.name == name));
            Ok(HttpResponse::Ok().json(ProviderMutationResponse {
                success: true,
                generation,
                provider,
            }))
        }
        Err(error) => Ok(apply_failure_response(error)),
    }
}

async fn emit_audit(
    state: &web::Data<AppState>,
    actor: &str,
    generation: u64,
    operation: &str,
    payload: Value,
) {
    let event = provider_config_audit_event(actor, generation, operation, payload);
    if let Err(error) = state.audit_logger.log(event).await {
        warn!("Failed to record provider config audit event: {error}");
    }
}

fn provider_config_audit_event(
    actor: &str,
    generation: u64,
    operation: &str,
    payload: Value,
) -> AuditEvent {
    AuditEvent::user_action(actor, UserAction::SettingsChanged)
        .with_metadata("generation", json!(generation))
        .with_metadata("operation", json!(operation))
        .with_metadata("diff", payload.get("diff").cloned().unwrap_or(Value::Null))
        .with_source("admin_providers")
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

fn public_provider(provider: &ProviderConfig, api_key_ref: Option<String>) -> PublicProvider {
    PublicProvider {
        name: provider.name.clone(),
        provider_type: provider.provider_type.clone(),
        base_url: provider.base_url.clone(),
        enabled: provider.enabled,
        models: provider.models.clone(),
        tags: provider.tags.clone(),
        weight: provider.weight,
        priority: provider.priority,
        api_key_ref,
    }
}

fn public_providers(config: &Config, refs: &HashMap<String, String>) -> Vec<PublicProvider> {
    config
        .providers()
        .iter()
        .map(|provider| public_provider(provider, refs.get(&provider.name).cloned()))
        .collect()
}

async fn api_key_refs_from_live(state: &web::Data<AppState>) -> HashMap<String, String> {
    api_key_refs_from_revision(
        state
            .storage
            .database
            .latest_provider_config_revision()
            .await
            .ok()
            .flatten()
            .as_ref()
            .map(|row| &row.sanitized_payload),
    )
}

fn api_key_refs_from_revision(payload: Option<&Value>) -> HashMap<String, String> {
    let mut refs = HashMap::new();
    let Some(Value::Array(providers)) = payload.and_then(|value| value.get("providers")) else {
        return refs;
    };
    for provider in providers {
        let Some(name) = provider.get("name").and_then(Value::as_str) else {
            continue;
        };
        if let Some(env_name) = provider.get("api_key_ref").and_then(Value::as_str)
            && !env_name.is_empty()
        {
            refs.insert(name.to_string(), env_name.to_string());
        }
    }
    refs
}

fn prepare_provider_secrets(
    provider: &mut ProviderConfig,
    api_key_ref: &mut Option<String>,
) -> Result<(), String> {
    if !provider.api_key.is_empty() {
        let (resolved, env_name) = resolve_env_reference(&provider.api_key, "api_key")?;
        provider.api_key = resolved;
        *api_key_ref = Some(env_name);
    }
    resolve_settings_secrets(&mut provider.settings)
}

fn apply_patch(
    provider: &mut ProviderConfig,
    patch: &ProviderPatch,
    api_key_ref: &mut Option<String>,
) -> Result<(), String> {
    if let Some(provider_type) = &patch.provider_type {
        provider.provider_type = provider_type.clone();
    }
    if let Some(api_key) = &patch.api_key {
        let (resolved, env_name) = resolve_env_reference(api_key, "api_key")?;
        provider.api_key = resolved;
        *api_key_ref = Some(env_name);
    }
    if let Some(base_url) = &patch.base_url {
        provider.base_url = base_url.clone();
    }
    if let Some(models) = &patch.models {
        provider.models = models.clone();
    }
    if let Some(tags) = &patch.tags {
        provider.tags = tags.clone();
    }
    if let Some(enabled) = patch.enabled {
        provider.enabled = enabled;
    }
    if let Some(weight) = patch.weight {
        provider.weight = weight;
    }
    if let Some(priority) = patch.priority {
        provider.priority = priority;
    }
    if let Some(rpm) = patch.rpm {
        provider.rpm = rpm;
    }
    if let Some(tpm) = patch.tpm {
        provider.tpm = tpm;
    }
    if let Some(settings) = &patch.settings {
        provider.settings = settings.clone();
        resolve_settings_secrets(&mut provider.settings)?;
    }
    Ok(())
}

fn resolve_settings_secrets(settings: &mut HashMap<String, Value>) -> Result<(), String> {
    for (key, value) in settings.iter_mut() {
        resolve_secret_value(key, value)?;
    }
    Ok(())
}

fn resolve_secret_value(key: &str, value: &mut Value) -> Result<(), String> {
    match value {
        Value::String(raw) if is_secret_setting_key(key) || looks_like_raw_secret(raw) => {
            let (resolved, _) = resolve_env_reference(raw, key)?;
            *raw = resolved;
            Ok(())
        }
        Value::String(raw) if env_ref_name(raw).is_some() => {
            let (resolved, _) = resolve_env_reference(raw, key)?;
            *raw = resolved;
            Ok(())
        }
        Value::Object(map) if is_secret_setting_key(key) => {
            for (nested_key, nested_value) in map.iter_mut() {
                resolve_secret_value(nested_key, nested_value)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn is_secret_setting_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "headers"
            | "header"
            | "auth"
            | "authorization"
            | "api_key"
            | "apikey"
            | "token"
            | "secret"
            | "password"
            | "bearer"
            | "private_key"
            | "access_token"
    )
}

fn looks_like_raw_secret(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("sk-") || trimmed.starts_with("Bearer ") || trimmed.starts_with("bearer ")
}

fn env_ref_name(value: &str) -> Option<&str> {
    let value = value.trim();
    let inner = value.strip_prefix("${")?.strip_suffix('}')?;
    let mut chars = inner.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    chars
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        .then_some(inner)
}

fn resolve_env_reference(value: &str, field: &str) -> Result<(String, String), String> {
    let Some(var) = env_ref_name(value) else {
        return Err(format!(
            "{field} must be an existing env reference of the form ${{VAR}}"
        ));
    };
    match std::env::var(var) {
        Ok(resolved) => Ok((resolved, var.to_string())),
        Err(_) => Err(format!(
            "Environment variable '{var}' referenced by {field} is not set"
        )),
    }
}

fn delete_conflict(
    name: &str,
    deleted_models: &[String],
    remaining: &[ProviderConfig],
    snapshot: &RoutingSnapshot,
    aliases: &HashMap<String, String>,
) -> Option<String> {
    if snapshot
        .provider_names
        .values()
        .any(|configured| configured == name)
    {
        return Some(format!(
            "Provider '{name}' is still referenced by live routing; disable it or remove routing references before delete"
        ));
    }

    let covered: HashSet<&str> = remaining
        .iter()
        .filter(|provider| provider.enabled)
        .flat_map(|provider| provider.models.iter().map(String::as_str))
        .collect();
    let unique: Vec<&str> = deleted_models
        .iter()
        .map(String::as_str)
        .filter(|model| !model.is_empty() && !covered.contains(model))
        .collect();
    let alias_hits: Vec<&str> = aliases
        .iter()
        .filter(|(alias, target)| {
            unique
                .iter()
                .any(|model| *model == alias.as_str() || *model == target.as_str())
        })
        .map(|(alias, _)| alias.as_str())
        .collect();
    if !alias_hits.is_empty() {
        return Some(format!(
            "Provider '{name}' is the only provider serving models still listed in routing aliases: {}",
            alias_hits.join(", ")
        ));
    }
    None
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
    use std::sync::Once;

    const TEST_KEY_ENV: &str = "LITELLM_TEST_PROVIDER_KEY";
    const TEST_KEY_VALUE: &str = "sk-test-env-SECRET-1270-not-returned";
    static ENV_ONCE: Once = Once::new();

    fn ensure_test_key() {
        ENV_ONCE.call_once(|| unsafe {
            std::env::set_var(TEST_KEY_ENV, TEST_KEY_VALUE);
        });
    }

    fn base_test_config(auth_enabled: bool) -> Config {
        let mut config = crate::server::valid_test_config();
        config.gateway.auth.enable_jwt = auth_enabled;
        config.gateway.auth.enable_api_key = auth_enabled;
        config.gateway.auth.allow_anonymous = !auth_enabled;
        config.gateway.auth.jwt_secret = "AaaAaaAaaAaaAaaAaaAaaAaaAaaAaa1!".to_string();
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = None;
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

    fn create_body(name: &str, extra: serde_json::Value) -> serde_json::Value {
        let mut body = json!({
            "name": name,
            "provider_type": "openai",
            "api_key": format!("${{{TEST_KEY_ENV}}}")
        });
        if let Some(object) = extra.as_object() {
            for (key, value) in object {
                body[key] = value.clone();
            }
        }
        body
    }

    #[test]
    fn env_ref_name_requires_braced_identifier() {
        assert_eq!(
            env_ref_name("${LITELLM_TEST_PROVIDER_KEY}"),
            Some(TEST_KEY_ENV)
        );
        assert!(env_ref_name("sk-raw-secret").is_none());
        assert!(env_ref_name("$LITELLM_TEST_PROVIDER_KEY").is_none());
        assert!(env_ref_name("${LITELLM_TEST_PROVIDER_KEY:-x}").is_none());
    }

    #[test]
    fn audit_event_metadata_has_actor_generation_and_no_secrets() {
        let payload = json!({
            "diff": {
                "before": [],
                "after": [{
                    "name": "admin-openai",
                    "provider_type": "openai",
                    "enabled": true,
                    "api_key_ref": TEST_KEY_ENV
                }]
            }
        });
        let event = provider_config_audit_event("actor-9", 4, "create", payload);
        assert_eq!(event.user_id.as_deref(), Some("actor-9"));
        assert_eq!(event.action, Some(UserAction::SettingsChanged));
        assert_eq!(event.metadata.get("generation"), Some(&json!(4)));
        let serialized = serde_json::to_string(&event).expect("json");
        assert!(!serialized.contains(TEST_KEY_VALUE));
        assert!(!serialized.contains("api_key\":"));
        assert!(!serialized.contains("sk-raw"));
    }

    #[actix_web::test]
    async fn providers_require_admin_identity() {
        let state = test_state(base_test_config(true)).await;
        let app = admin_app(state, None).await;
        let resp = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri("/admin/providers")
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let body: Value = actix_test::read_body_json(resp).await;
        assert_eq!(body["success"], false);
        assert_eq!(body["error"], ADMIN_ERROR);
    }

    #[actix_web::test]
    async fn providers_reject_non_admin_user() {
        let state = test_state(base_test_config(true)).await;
        let app = admin_app(state, Some(make_test_user(UserRole::User))).await;
        let resp = actix_test::call_service(
            &app,
            actix_test::TestRequest::post()
                .uri("/admin/providers")
                .set_json(json!({"name":"x","provider_type":"openai","api_key":"sk-raw"}))
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    async fn post_raw_api_key_is_rejected_and_get_omits_secrets() {
        ensure_test_key();
        let state = test_state(base_test_config(true)).await;
        let generation = state.pin_runtime().generation;
        let app = admin_app(state.clone(), Some(make_test_user(UserRole::Admin))).await;
        let resp = actix_test::call_service(
            &app,
            actix_test::TestRequest::post()
                .uri("/admin/providers")
                .set_json(json!({
                    "name": "raw-secret-provider",
                    "provider_type": "openai",
                    "api_key": "sk-raw-not-a-ref"
                }))
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert_eq!(state.pin_runtime().generation, generation);

        let listed = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri("/admin/providers")
                .to_request(),
        )
        .await;
        assert_eq!(listed.status(), StatusCode::OK);
        let body: Value = actix_test::read_body_json(listed).await;
        let serialized = body.to_string();
        assert!(!serialized.contains("sk-raw-not-a-ref"));
        assert!(body["providers"][0].get("api_key").is_none());
    }

    #[actix_web::test]
    async fn post_env_ref_applies_and_get_hides_secret() {
        ensure_test_key();
        let admin = make_test_user(UserRole::Admin);
        let actor = admin.id().to_string();
        let state = test_state(base_test_config(true)).await;
        let before = state.pin_runtime().generation;
        let app = admin_app(state.clone(), Some(admin)).await;
        let resp = actix_test::call_service(
            &app,
            actix_test::TestRequest::post()
                .uri("/admin/providers")
                .set_json(create_body("admin-openai", json!({})))
                .to_request(),
        )
        .await;
        let status = resp.status();
        let created: Value = actix_test::read_body_json(resp).await;
        assert_eq!(status, StatusCode::OK, "create failed: {created}");
        assert_eq!(created["success"], true);
        assert_eq!(created["generation"], before + 1);
        assert_eq!(created["provider"]["name"], "admin-openai");
        assert_eq!(created["provider"]["api_key_ref"], TEST_KEY_ENV);
        assert!(created["provider"].get("api_key").is_none());
        assert!(!created.to_string().contains(TEST_KEY_VALUE));
        assert_eq!(state.pin_runtime().generation, before + 1);

        let listed = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri("/admin/providers")
                .to_request(),
        )
        .await;
        let body: Value = actix_test::read_body_json(listed).await;
        let names: Vec<&str> = body["providers"]
            .as_array()
            .expect("providers")
            .iter()
            .filter_map(|provider| provider["name"].as_str())
            .collect();
        assert!(names.contains(&"admin-openai"));
        assert!(!body.to_string().contains(TEST_KEY_VALUE));
        for provider in body["providers"].as_array().expect("providers") {
            assert!(provider.get("api_key").is_none());
            assert!(provider.get("headers").is_none());
        }

        let revision = state
            .storage
            .database
            .latest_provider_config_revision()
            .await
            .expect("lookup")
            .expect("revision");
        assert_eq!(revision.generation, (before + 1) as i64);
        assert_eq!(revision.actor, actor);
        let payload = revision.sanitized_payload.to_string();
        assert!(!payload.contains(TEST_KEY_VALUE));
        assert!(!payload.contains("sk-raw"));
    }

    #[actix_web::test]
    async fn failed_apply_leaves_generation_unchanged() {
        ensure_test_key();
        let state = test_state(base_test_config(true)).await;
        let before = state.pin_runtime().generation;
        let app = admin_app(state.clone(), Some(make_test_user(UserRole::Admin))).await;
        let resp = actix_test::call_service(
            &app,
            actix_test::TestRequest::post()
                .uri("/admin/providers")
                .set_json(json!({
                    "name": "bad-type",
                    "provider_type": "not-a-real-provider-type",
                    "api_key": format!("${{{TEST_KEY_ENV}}}")
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
                .latest_provider_config_revision()
                .await
                .expect("lookup")
                .is_none()
        );
    }

    #[actix_web::test]
    async fn delete_referenced_provider_conflicts() {
        ensure_test_key();
        let state = test_state(base_test_config(true)).await;
        let app = admin_app(state.clone(), Some(make_test_user(UserRole::Admin))).await;
        let created = actix_test::call_service(
            &app,
            actix_test::TestRequest::post()
                .uri("/admin/providers")
                .set_json(create_body("spare-openai", json!({})))
                .to_request(),
        )
        .await;
        let spare_status = created.status();
        let spare_body: Value = actix_test::read_body_json(created).await;
        assert_eq!(
            spare_status,
            StatusCode::OK,
            "spare create failed: {spare_body}"
        );

        let resp = actix_test::call_service(
            &app,
            actix_test::TestRequest::delete()
                .uri("/admin/providers/test-openai")
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::CONFLICT);
        let body: Value = actix_test::read_body_json(resp).await;
        assert_eq!(body["success"], false);
        assert!(
            body["error"]
                .as_str()
                .unwrap_or_default()
                .contains("still referenced")
        );
        assert!(
            state
                .pin_runtime()
                .config
                .providers()
                .iter()
                .any(|provider| provider.name == "test-openai")
        );
    }

    #[actix_web::test]
    async fn delete_unused_disabled_provider_applies() {
        ensure_test_key();
        let state = test_state(base_test_config(true)).await;
        let app = admin_app(state.clone(), Some(make_test_user(UserRole::Admin))).await;
        let created = actix_test::call_service(
            &app,
            actix_test::TestRequest::post()
                .uri("/admin/providers")
                .set_json(create_body(
                    "unused-disabled",
                    json!({"enabled": false, "models": []}),
                ))
                .to_request(),
        )
        .await;
        assert_eq!(created.status(), StatusCode::OK);
        let before = state.pin_runtime().generation;

        let resp = actix_test::call_service(
            &app,
            actix_test::TestRequest::delete()
                .uri("/admin/providers/unused-disabled")
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body: Value = actix_test::read_body_json(resp).await;
        assert_eq!(body["success"], true);
        assert_eq!(body["generation"], before + 1);
        assert!(
            state
                .pin_runtime()
                .config
                .providers()
                .iter()
                .all(|provider| provider.name != "unused-disabled")
        );
    }

    #[actix_web::test]
    async fn patch_can_disable_provider() {
        ensure_test_key();
        let state = test_state(base_test_config(true)).await;
        let app = admin_app(state.clone(), Some(make_test_user(UserRole::Admin))).await;
        let created = actix_test::call_service(
            &app,
            actix_test::TestRequest::post()
                .uri("/admin/providers")
                .set_json(create_body("disable-me", json!({})))
                .to_request(),
        )
        .await;
        let disable_status = created.status();
        let disable_body: Value = actix_test::read_body_json(created).await;
        assert_eq!(
            disable_status,
            StatusCode::OK,
            "create failed: {disable_body}"
        );

        let resp = actix_test::call_service(
            &app,
            actix_test::TestRequest::patch()
                .uri("/admin/providers/disable-me")
                .set_json(json!({"enabled": false}))
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let runtime = state.pin_runtime();
        let live = runtime
            .config
            .providers()
            .iter()
            .find(|provider| provider.name == "disable-me")
            .expect("provider");
        assert!(!live.enabled);
    }
}
