//! Admin-only cursor-paginated query over the metadata-only request ledger.

use super::admin::require_admin;
use super::errors;
use crate::core::request_ledger::{RequestBilling, RequestReconciliation};
use crate::server::state::AppState;
use crate::storage::database::RequestLedgerListFilter;
use crate::storage::database::entities::request_ledger;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::{HttpRequest, HttpResponse, web};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

const ADMIN_ERROR: &str = "Admin role required for request ledger";
const DEFAULT_PAGE_LIMIT: u64 = 50;
const MAX_PAGE_LIMIT: u64 = 100;
const TERMINAL_STATUSES: &[&str] = &["completed", "failed", "cancelled"];

#[derive(Debug, Deserialize)]
struct LedgerQuery {
    finished_after: Option<String>,
    finished_before: Option<String>,
    request_id: Option<String>,
    model: Option<String>,
    provider: Option<String>,
    terminal_status: Option<String>,
    cursor: Option<String>,
    limit: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CursorPayload {
    f: String,
    i: String,
}

/// One metadata-only ledger row. Prompt, body, and credentials are not fields.
#[derive(Debug, Serialize)]
struct RequestLedgerItem {
    request_id: String,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
    method: String,
    endpoint: String,
    model: Option<String>,
    provider: Option<String>,
    deployment: Option<String>,
    status_code: i32,
    terminal_status: String,
    latency_ms: i64,
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    total_tokens: Option<i64>,
    cost: Option<f64>,
    billing: Option<RequestBilling>,
    reconciliation: Option<RequestReconciliation>,
    awaiting_duration_ms: Option<i64>,
    user_id: Option<String>,
    api_key_id: Option<String>,
    team_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct RequestLedgerPage {
    success: bool,
    items: Vec<RequestLedgerItem>,
    next_cursor: Option<String>,
    has_more: bool,
}

/// `GET /admin/request-ledger`
async fn list_request_ledger(
    req: HttpRequest,
    state: web::Data<AppState>,
    query: web::Query<LedgerQuery>,
) -> actix_web::Result<HttpResponse> {
    if let Some(forbidden) = require_admin(&req, &state, "query request ledger", ADMIN_ERROR) {
        return Ok(forbidden);
    }

    match parsed_list_request(&query) {
        Ok((filter, limit)) => list_page(&state, filter, limit).await,
        Err(message) => Ok(errors::validation_error(&message)),
    }
}

pub(super) fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/admin/request-ledger")
            .route("", web::get().to(list_request_ledger))
            .route("/", web::get().to(list_request_ledger))
            .route(
                "/{request_id}/reconciliation",
                web::post().to(reconcile_request),
            ),
    );
}

#[derive(Deserialize)]
struct ReconciliationInput {
    verified_actual_cost: f64,
    evidence_reference: String,
}

async fn reconcile_request(
    req: HttpRequest,
    state: web::Data<AppState>,
    request_id: web::Path<String>,
    input: web::Json<ReconciliationInput>,
) -> actix_web::Result<HttpResponse> {
    if let Some(forbidden) = require_admin(&req, &state, "reconcile request ledger", ADMIN_ERROR) {
        return Ok(forbidden);
    }
    if !input.verified_actual_cost.is_finite() || input.verified_actual_cost < 0.0 {
        return Ok(errors::validation_error(
            "verified_actual_cost must be finite and nonnegative",
        ));
    }
    let evidence = input.evidence_reference.trim();
    if evidence.is_empty() || evidence.len() > 512 {
        return Ok(errors::validation_error(
            "evidence_reference must contain 1 to 512 bytes",
        ));
    }
    let reconciliation = state
        .storage
        .database
        .reconcile_request_ledger(
            &request_id,
            input.verified_actual_cost,
            evidence.to_string(),
        )
        .await
        .map_err(|error| match error {
            GatewayError::Conflict(message) => actix_web::error::ErrorConflict(message),
            other => actix_web::error::ErrorInternalServerError(other.to_string()),
        })?;
    match reconciliation {
        Some(value) => {
            Ok(HttpResponse::Ok()
                .json(serde_json::json!({"success": true, "reconciliation": value})))
        }
        None => Ok(HttpResponse::NotFound()
            .json(serde_json::json!({"error": "request ledger row not found"}))),
    }
}

async fn list_page(
    state: &web::Data<AppState>,
    filter: RequestLedgerListFilter,
    limit: u64,
) -> actix_web::Result<HttpResponse> {
    let mut rows = state
        .storage
        .database
        .list_request_ledger(&filter, limit + 1)
        .await
        .map_err(|error| actix_web::error::ErrorInternalServerError(error.to_string()))?;

    let has_more = rows.len() as u64 > limit;
    if has_more {
        rows.truncate(limit as usize);
    }
    let next_cursor = if has_more {
        rows.last()
            .map(|row| encode_cursor(row.finished_at.with_timezone(&Utc), &row.request_id))
    } else {
        None
    };

    Ok(HttpResponse::Ok().json(RequestLedgerPage {
        success: true,
        items: rows
            .into_iter()
            .map(item_from_row)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| actix_web::error::ErrorInternalServerError(error.to_string()))?,
        next_cursor,
        has_more,
    }))
}

fn parsed_list_request(query: &LedgerQuery) -> Result<(RequestLedgerListFilter, u64), String> {
    let limit = query.limit.unwrap_or(DEFAULT_PAGE_LIMIT);
    if limit == 0 || limit > MAX_PAGE_LIMIT {
        return Err(format!("limit must be between 1 and {MAX_PAGE_LIMIT}"));
    }

    let finished_after = parse_optional_rfc3339(query.finished_after.as_deref(), "finished_after")?;
    let finished_before =
        parse_optional_rfc3339(query.finished_before.as_deref(), "finished_before")?;
    if let (Some(after), Some(before)) = (finished_after, finished_before)
        && after >= before
    {
        return Err("finished_after must be earlier than finished_before".to_string());
    }

    let terminal_status = nonempty(query.terminal_status.as_deref());
    if let Some(status) = terminal_status.as_deref()
        && !TERMINAL_STATUSES.contains(&status)
    {
        return Err("terminal_status must be one of completed, failed, or cancelled".to_string());
    }

    let (after_finished_at, after_request_id) = match nonempty(query.cursor.as_deref()) {
        Some(cursor) => {
            let (finished_at, request_id) = decode_cursor(&cursor)?;
            (Some(finished_at), Some(request_id))
        }
        None => (None, None),
    };

    Ok((
        RequestLedgerListFilter {
            finished_after,
            finished_before,
            request_id: nonempty(query.request_id.as_deref()),
            model: nonempty(query.model.as_deref()),
            provider: nonempty(query.provider.as_deref()),
            terminal_status,
            after_finished_at,
            after_request_id,
        },
        limit,
    ))
}

fn parse_optional_rfc3339(
    value: Option<&str>,
    field: &str,
) -> Result<Option<DateTime<Utc>>, String> {
    match nonempty(value) {
        Some(raw) => DateTime::parse_from_rfc3339(&raw)
            .map(|timestamp| Some(timestamp.with_timezone(&Utc)))
            .map_err(|_| format!("Invalid {field}: expected RFC3339 timestamp")),
        None => Ok(None),
    }
}

fn nonempty(value: Option<&str>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn encode_cursor(finished_at: DateTime<Utc>, request_id: &str) -> String {
    let payload = CursorPayload {
        f: finished_at.to_rfc3339(),
        i: request_id.to_string(),
    };
    let json = serde_json::to_vec(&payload).unwrap_or_else(|_| br#"{"f":"","i":""}"#.to_vec());
    URL_SAFE_NO_PAD.encode(json)
}

fn decode_cursor(cursor: &str) -> Result<(DateTime<Utc>, String), String> {
    let bytes = URL_SAFE_NO_PAD
        .decode(cursor.trim())
        .map_err(|_| "Invalid cursor".to_string())?;
    let payload: CursorPayload =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid cursor".to_string())?;
    if payload.i.trim().is_empty() {
        return Err("Invalid cursor".to_string());
    }
    let finished_at = DateTime::parse_from_rfc3339(&payload.f)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|_| "Invalid cursor".to_string())?;
    Ok((finished_at, payload.i))
}

fn item_from_row(row: request_ledger::Model) -> Result<RequestLedgerItem, GatewayError> {
    let billing = row
        .billing
        .map(serde_json::from_value::<RequestBilling>)
        .transpose()
        .map_err(|error| GatewayError::Storage(format!("invalid request billing: {error}")))?;
    let mut reconciliation = row
        .reconciliation
        .map(serde_json::from_value::<RequestReconciliation>)
        .transpose()
        .map_err(|error| {
            GatewayError::Storage(format!("invalid request reconciliation: {error}"))
        })?;
    if let Some(reconciliation) = reconciliation.as_mut() {
        reconciliation.budget_review_required = billing.as_ref().is_none_or(|billing| {
            billing.requires_budget_review(reconciliation.verified_actual_cost)
        });
    }
    let awaiting_duration_ms = if reconciliation.is_none() {
        billing
            .as_ref()
            .and_then(|billing| billing.awaiting_since)
            .map(|since| (Utc::now() - since).num_milliseconds().max(0))
    } else {
        None
    };
    Ok(RequestLedgerItem {
        request_id: row.request_id,
        started_at: row.started_at.with_timezone(&Utc),
        finished_at: row.finished_at.with_timezone(&Utc),
        method: row.method,
        endpoint: row.endpoint,
        model: row.model,
        provider: row.provider,
        deployment: row.deployment,
        status_code: row.status_code,
        terminal_status: row.terminal_status,
        latency_ms: row.latency_ms,
        prompt_tokens: row.prompt_tokens,
        completion_tokens: row.completion_tokens,
        total_tokens: row.total_tokens,
        cost: row.cost,
        billing,
        reconciliation,
        awaiting_duration_ms,
        user_id: row.user_id,
        api_key_id: row.api_key_id,
        team_id: row.team_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::core::models::{
        Metadata, UsageStats,
        user::{
            preferences::UserPreferences,
            types::{User, UserProfile, UserRole, UserStatus},
        },
    };
    use crate::core::request_ledger::RequestLedgerRecord;
    use crate::server::HttpServer;
    use actix_web::dev::Service;
    use actix_web::{App, HttpMessage, http::StatusCode, test as actix_test};
    use chrono::Duration;

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

    fn record(request_id: &str, finished_at: DateTime<Utc>) -> RequestLedgerRecord {
        RequestLedgerRecord {
            request_id: request_id.to_string(),
            started_at: finished_at,
            finished_at,
            method: "POST".to_string(),
            endpoint: "/v1/chat/completions".to_string(),
            model: Some("gpt-4".to_string()),
            provider: Some("openai".to_string()),
            deployment: Some("openai".to_string()),
            status_code: 200,
            terminal_status: "completed".to_string(),
            latency_ms: 9,
            prompt_tokens: Some(4),
            completion_tokens: Some(6),
            total_tokens: Some(10),
            cost: Some(0.02),
            billing: None,
            user_id: None,
            api_key_id: Some("key-id".to_string()),
            team_id: Some("team-id".to_string()),
        }
    }

    #[test]
    fn cursor_round_trip_preserves_seek_key() {
        let finished_at = DateTime::parse_from_rfc3339("2026-01-02T03:04:05Z")
            .expect("rfc3339")
            .with_timezone(&Utc);
        let encoded = encode_cursor(finished_at, "req-1");
        let (decoded_at, decoded_id) = decode_cursor(&encoded).expect("decode");
        assert_eq!(decoded_id, "req-1");
        assert_eq!(decoded_at, finished_at);
    }

    #[test]
    fn ledger_item_json_is_metadata_only() {
        let json = serde_json::to_value(RequestLedgerItem {
            request_id: "req-json".to_string(),
            started_at: Utc::now(),
            finished_at: Utc::now(),
            method: "POST".to_string(),
            endpoint: "/v1/chat/completions".to_string(),
            model: Some("gpt-4".to_string()),
            provider: Some("openai".to_string()),
            deployment: None,
            status_code: 200,
            terminal_status: "completed".to_string(),
            latency_ms: 1,
            prompt_tokens: Some(1),
            completion_tokens: Some(1),
            total_tokens: Some(2),
            cost: Some(0.01),
            billing: None,
            reconciliation: None,
            awaiting_duration_ms: None,
            user_id: None,
            api_key_id: Some("key-id".to_string()),
            team_id: None,
        })
        .expect("json");
        let object = json.as_object().expect("object");
        for forbidden in [
            "body",
            "prompt",
            "authorization",
            "header",
            "secret",
            "api_key",
            "raw_key",
        ] {
            assert!(
                !object.contains_key(forbidden),
                "{forbidden} must not be serialized"
            );
        }
        assert!(object.contains_key("api_key_id"));
        assert!(object.contains_key("request_id"));
    }

    #[actix_web::test]
    async fn request_ledger_requires_admin_identity() {
        let state = test_state(base_test_config(true)).await;
        let app = admin_app(state, None).await;
        let resp = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri("/admin/request-ledger")
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    async fn request_ledger_rejects_non_admin_user() {
        let state = test_state(base_test_config(true)).await;
        let app = admin_app(state, Some(make_test_user(UserRole::User))).await;
        let resp = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri("/admin/request-ledger")
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    async fn request_ledger_rejects_invalid_cursor_and_filters() {
        let state = test_state(base_test_config(true)).await;
        let app = admin_app(state, Some(make_test_user(UserRole::Admin))).await;

        for uri in [
            "/admin/request-ledger?cursor=not-a-cursor",
            "/admin/request-ledger?terminal_status=running",
            "/admin/request-ledger?limit=0",
            "/admin/request-ledger?limit=101",
            "/admin/request-ledger?finished_after=2026-01-02T00:00:00Z&finished_before=2026-01-01T00:00:00Z",
            "/admin/request-ledger?finished_after=not-a-time",
        ] {
            let resp = actix_test::call_service(
                &app,
                actix_test::TestRequest::get().uri(uri).to_request(),
            )
            .await;
            assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{uri}");
        }
    }

    #[actix_web::test]
    async fn request_ledger_returns_empty_page_for_admin() {
        let state = test_state(base_test_config(true)).await;
        let app = admin_app(state, Some(make_test_user(UserRole::Admin))).await;
        let resp = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri("/admin/request-ledger")
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body: serde_json::Value = actix_test::read_body_json(resp).await;
        assert_eq!(body["success"], true);
        assert_eq!(body["items"], serde_json::json!([]));
        assert_eq!(body["has_more"], false);
        assert!(body["next_cursor"].is_null());
    }

    #[actix_web::test]
    async fn request_ledger_pages_and_filters_seeded_rows() {
        let state = test_state(base_test_config(true)).await;
        let t1 = Utc::now() - Duration::seconds(30);
        let t2 = Utc::now() - Duration::seconds(20);
        let t3 = Utc::now() - Duration::seconds(10);
        state
            .storage
            .database
            .store_request_ledger(&record("req-a", t1), 30)
            .await
            .expect("a");
        state
            .storage
            .database
            .store_request_ledger(&record("req-b", t2), 30)
            .await
            .expect("b");
        let mut failed = record("req-c", t3);
        failed.terminal_status = "failed".to_string();
        failed.model = Some("claude".to_string());
        state
            .storage
            .database
            .store_request_ledger(&failed, 30)
            .await
            .expect("c");

        let app = admin_app(state, Some(make_test_user(UserRole::Admin))).await;
        let first = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri("/admin/request-ledger?limit=2")
                .to_request(),
        )
        .await;
        assert_eq!(first.status(), StatusCode::OK);
        let first_body: serde_json::Value = actix_test::read_body_json(first).await;
        assert_eq!(first_body["has_more"], true);
        assert_eq!(first_body["items"][0]["request_id"], "req-c");
        assert_eq!(first_body["items"][1]["request_id"], "req-b");
        let cursor = first_body["next_cursor"].as_str().expect("cursor");

        let second = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri(&format!("/admin/request-ledger?limit=2&cursor={cursor}"))
                .to_request(),
        )
        .await;
        let second_body: serde_json::Value = actix_test::read_body_json(second).await;
        assert_eq!(second_body["items"][0]["request_id"], "req-a");
        assert_eq!(second_body["has_more"], false);

        let filtered = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri("/admin/request-ledger?model=claude&terminal_status=failed")
                .to_request(),
        )
        .await;
        let filtered_body: serde_json::Value = actix_test::read_body_json(filtered).await;
        assert_eq!(filtered_body["items"].as_array().expect("items").len(), 1);
        assert_eq!(filtered_body["items"][0]["request_id"], "req-c");
        assert!(filtered_body["items"][0].get("body").is_none());
        assert!(filtered_body["items"][0].get("authorization").is_none());
    }
    #[actix_web::test]
    async fn reconciliation_requires_admin_and_preserves_unknown_charge() {
        let state = test_state(base_test_config(true)).await;
        let mut row = record("unknown", Utc::now());
        row.cost = None;
        row.billing = Some(RequestBilling {
            provider_reserved_amount: Some(1.0),
            provider_charge_amount: Some(1.0),
            provider_settlement: Some("settled".into()),
            unknown_reason: Some("stream_cancelled_before_usage".into()),
            awaiting_since: Some(Utc::now() - Duration::seconds(30)),
            ..Default::default()
        });
        state
            .storage
            .database
            .store_request_ledger(&row, 30)
            .await
            .unwrap();
        let unauthorized = admin_app(state.clone(), Some(make_test_user(UserRole::User))).await;
        let resp = actix_test::call_service(&unauthorized, actix_test::TestRequest::post()
            .uri("/admin/request-ledger/unknown/reconciliation")
            .set_json(serde_json::json!({"verified_actual_cost": 0.2, "evidence_reference": "invoice-line-42"})).to_request()).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let admin = admin_app(state, Some(make_test_user(UserRole::Admin))).await;
        let resp = actix_test::call_service(
            &admin,
            actix_test::TestRequest::get()
                .uri("/admin/request-ledger?request_id=unknown")
                .to_request(),
        )
        .await;
        let page: serde_json::Value = actix_test::read_body_json(resp).await;
        assert!(page["items"][0]["awaiting_duration_ms"].as_i64().unwrap() >= 30_000);
        for (id, amount, evidence, status) in [
            ("missing", 0.2, "invoice-line-42", StatusCode::NOT_FOUND),
            ("unknown", -0.2, "invoice-line-42", StatusCode::BAD_REQUEST),
            ("unknown", 0.2, "", StatusCode::BAD_REQUEST),
            ("unknown", 0.2, "invoice-line-42", StatusCode::OK),
            ("unknown", 0.2, "invoice-line-42", StatusCode::OK),
            ("unknown", 0.3, "invoice-line-43", StatusCode::CONFLICT),
        ] {
            let resp = actix_test::call_service(&admin, actix_test::TestRequest::post()
                .uri(&format!("/admin/request-ledger/{id}/reconciliation"))
                .set_json(serde_json::json!({"verified_actual_cost": amount, "evidence_reference": evidence})).to_request()).await;
            assert_eq!(resp.status(), status);
        }
        let resp = actix_test::call_service(
            &admin,
            actix_test::TestRequest::get()
                .uri("/admin/request-ledger?request_id=unknown")
                .to_request(),
        )
        .await;
        let page: serde_json::Value = actix_test::read_body_json(resp).await;
        assert!(page["items"][0]["cost"].is_null());
        assert_eq!(page["items"][0]["billing"]["provider_charge_amount"], 1.0);
        assert_eq!(
            page["items"][0]["reconciliation"]["verified_actual_cost"],
            0.2
        );
        assert_eq!(
            page["items"][0]["reconciliation"]["budget_review_required"],
            true
        );
        assert!(page["items"][0]["awaiting_duration_ms"].is_null());
    }
}
