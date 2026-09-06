//! Compile-time embedded administrator dashboard assets.

use actix_web::http::header::{CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE};
use actix_web::{HttpResponse, web};

const INDEX_HTML: &str = include_str!("admin_dashboard/index.html");
const APP_CSS: &str = include_str!("admin_dashboard/app.css");
const APP_JS: &str = include_str!("admin_dashboard/app.js");
const BUDGET_JS: &str = include_str!("admin_dashboard/budget.js");
const PROVIDERS_JS: &str = include_str!("admin_dashboard/providers.js");
const ROUTING_POLICY_JS: &str = include_str!("admin_dashboard/routing_policy.js");
const PROVIDER_HEALTH_JS: &str = include_str!("admin_dashboard/provider_health.js");
const ROUTING_INVENTORY_JS: &str = include_str!("admin_dashboard/routing_inventory.js");
const REQUEST_LEDGER_JS: &str = include_str!("admin_dashboard/request_ledger.js");
const DASHBOARD_CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; \
    connect-src 'self'; img-src 'self' data:; font-src 'self'; base-uri 'none'; \
    form-action 'self'; frame-ancestors 'none'";

fn embedded_asset(content_type: &'static str, body: &'static str) -> HttpResponse {
    HttpResponse::Ok()
        .insert_header((CONTENT_TYPE, content_type))
        .insert_header((CACHE_CONTROL, "no-store"))
        .insert_header((CONTENT_SECURITY_POLICY, DASHBOARD_CSP))
        .body(body)
}

async fn dashboard() -> HttpResponse {
    embedded_asset("text/html; charset=utf-8", INDEX_HTML)
}

async fn stylesheet() -> HttpResponse {
    embedded_asset("text/css; charset=utf-8", APP_CSS)
}

async fn javascript() -> HttpResponse {
    embedded_asset("text/javascript; charset=utf-8", APP_JS)
}

async fn provider_health_javascript() -> HttpResponse {
    embedded_asset("text/javascript; charset=utf-8", PROVIDER_HEALTH_JS)
}

async fn routing_inventory_javascript() -> HttpResponse {
    embedded_asset("text/javascript; charset=utf-8", ROUTING_INVENTORY_JS)
}

async fn request_ledger_javascript() -> HttpResponse {
    embedded_asset("text/javascript; charset=utf-8", REQUEST_LEDGER_JS)
}

async fn budget_javascript() -> HttpResponse {
    embedded_asset("text/javascript; charset=utf-8", BUDGET_JS)
}

async fn providers_javascript() -> HttpResponse {
    embedded_asset("text/javascript; charset=utf-8", PROVIDERS_JS)
}

async fn routing_policy_javascript() -> HttpResponse {
    embedded_asset("text/javascript; charset=utf-8", ROUTING_POLICY_JS)
}

/// Register the exact dashboard asset routes.
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/admin/dashboard", web::get().to(dashboard))
        .route("/admin/dashboard/app.css", web::get().to(stylesheet))
        .route(
            "/admin/dashboard/provider-health.js",
            web::get().to(provider_health_javascript),
        )
        .route(
            "/admin/dashboard/routing-inventory.js",
            web::get().to(routing_inventory_javascript),
        )
        .route(
            "/admin/dashboard/request-ledger.js",
            web::get().to(request_ledger_javascript),
        )
        .route(
            "/admin/dashboard/budget.js",
            web::get().to(budget_javascript),
        )
        .route(
            "/admin/dashboard/providers.js",
            web::get().to(providers_javascript),
        )
        .route(
            "/admin/dashboard/routing-policy.js",
            web::get().to(routing_policy_javascript),
        )
        .route("/admin/dashboard/app.js", web::get().to(javascript));
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http::{StatusCode, header};
    use actix_web::{App, test as actix_test};

    #[actix_web::test]
    async fn serves_exact_dashboard_assets_with_security_headers() {
        let app = actix_test::init_service(App::new().configure(configure_routes)).await;
        let cases = [
            (
                "/admin/dashboard",
                "text/html; charset=utf-8",
                "LiteLLM-RS Admin",
            ),
            (
                "/admin/dashboard/app.css",
                "text/css; charset=utf-8",
                ":focus-visible",
            ),
            (
                "/admin/dashboard/app.js",
                "text/javascript; charset=utf-8",
                "\"/auth/login\"",
            ),
            (
                "/admin/dashboard/provider-health.js",
                "text/javascript; charset=utf-8",
                "createProviderHealthView",
            ),
            (
                "/admin/dashboard/routing-inventory.js",
                "text/javascript; charset=utf-8",
                "createRoutingInventoryView",
            ),
            (
                "/admin/dashboard/request-ledger.js",
                "text/javascript; charset=utf-8",
                "createRequestLedgerView",
            ),
            (
                "/admin/dashboard/budget.js",
                "text/javascript; charset=utf-8",
                "createBudgetView",
            ),
            (
                "/admin/dashboard/providers.js",
                "text/javascript; charset=utf-8",
                "createProviderEditorView",
            ),
            (
                "/admin/dashboard/routing-policy.js",
                "text/javascript; charset=utf-8",
                "createRoutingPolicyView",
            ),
        ];

        for (path, content_type, marker) in cases {
            let response = actix_test::call_service(
                &app,
                actix_test::TestRequest::get().uri(path).to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers().get(header::CONTENT_TYPE).unwrap(),
                content_type
            );
            assert_eq!(
                response.headers().get(header::CACHE_CONTROL).unwrap(),
                "no-store"
            );
            assert_eq!(
                response
                    .headers()
                    .get(header::CONTENT_SECURITY_POLICY)
                    .unwrap(),
                DASHBOARD_CSP
            );
            let body = actix_test::read_body(response).await;
            let body = std::str::from_utf8(&body).unwrap();
            assert!(body.contains(marker), "{path} missing {marker}");
        }
    }

    #[actix_web::test]
    async fn does_not_serve_dashboard_prefix_fallbacks() {
        let app = actix_test::init_service(App::new().configure(configure_routes)).await;

        for path in [
            "/admin/dashboard/",
            "/admin/dashboard/app.js.map",
            "/admin/dashboard/budget.js.map",
            "/admin/dashboard/routing-inventory.js.map",
            "/admin/dashboard/request-ledger.js.map",
            "/admin/dashboard/providers.js.map",
            "/admin/dashboard/routing-policy.js.map",
            "/admin/dashboard/private",
        ] {
            let response = actix_test::call_service(
                &app,
                actix_test::TestRequest::get().uri(path).to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        }
    }

    #[test]
    fn assets_preserve_auth_api_and_safe_rendering_contracts() {
        for path in [
            "\"/auth/login\"",
            "\"/auth/logout\"",
            "\"/v1/keys",
            "\"/v1/teams",
            "/usage",
        ] {
            assert!(APP_JS.contains(path), "missing API path {path}");
        }
        assert!(PROVIDER_HEALTH_JS.contains("\"/health/detailed\""));
        assert!(ROUTING_INVENTORY_JS.contains("\"/admin/routing/inventory\""));
        assert!(REQUEST_LEDGER_JS.contains("\"/admin/request-ledger\""));
        assert!(BUDGET_JS.contains("\"/v1/budget/providers\""));
        assert!(BUDGET_JS.contains("\"/v1/budget/models\""));
        assert!(PROVIDERS_JS.contains("\"/admin/providers\""));
        assert!(ROUTING_POLICY_JS.contains("\"/admin/routing/policy\""));
        for asset in [
            APP_JS,
            PROVIDER_HEALTH_JS,
            ROUTING_INVENTORY_JS,
            REQUEST_LEDGER_JS,
            BUDGET_JS,
            PROVIDERS_JS,
            ROUTING_POLICY_JS,
        ] {
            for forbidden in [
                "localStorage",
                "sessionStorage",
                "document.cookie",
                "innerHTML",
                "eval(",
                "http://",
                "https://",
            ] {
                assert!(!asset.contains(forbidden), "unsafe asset token {forbidden}");
            }
        }
        assert!(APP_JS.contains("AbortController"));
        assert!(APP_JS.contains("session.generation !== state.generation"));
        assert!(APP_JS.contains("requestVersion !== state.keyRequestVersion"));
        assert!(APP_JS.contains("requestVersion !== state.teamRequestVersion"));
        assert!(APP_JS.contains("requestVersion !== state.usageRequestVersion"));
        assert!(APP_JS.contains("teams.some((team, index)"));
        assert!(APP_JS.contains("async function logoutRequest"));
        assert!(APP_JS.contains("response.status === 401"));
        assert!(APP_JS.contains("Protected dashboard data was cleared"));
        assert!(APP_JS.contains("payload?.error?.message"));
        assert!(APP_JS.contains("text.trim()"));
        assert!(APP_JS.contains("navigator.clipboard?.writeText"));
        assert!(APP_JS.contains("is_admin: false"));
        assert!(APP_JS.contains("value === \"*\""));
        assert!(APP_JS.contains("user_id: state.adminId"));
        assert!(APP_JS.contains("team_id: teamId"));
        assert!(APP_JS.contains("value == null || value === \"\""));
        assert!(!APP_JS.contains("keyTotal + teamTotal"));
    }

    #[test]
    fn html_exposes_accessibility_and_one_time_secret_hooks() {
        for marker in [
            "<main",
            "<nav",
            "<label",
            "aria-live=\"polite\"",
            "role=\"alert\"",
            "<dialog",
            "id=\"raw-key-value\"",
            "id=\"sign-out\"",
            "id=\"key-spend-empty\"",
            "id=\"team-spend-empty\"",
            "aria-describedby=\"team-name-help\"",
            "id=\"team-name-help\"",
            "id=\"budgets-panel\"",
            "id=\"budget-form\"",
            "id=\"provider-budgets-body\"",
            "id=\"model-budgets-body\"",
            "id=\"routing-panel\"",
            "id=\"routing-body\"",
            "id=\"routing-unavailable-body\"",
            "id=\"request-logs-panel\"",
            "id=\"request-logs-body\"",
            "id=\"request-logs-empty\"",
            "id=\"request-logs-detail\"",
            "id=\"providers-panel\"",
            "id=\"create-provider-form\"",
            "id=\"edit-provider-form\"",
            "id=\"providers-body\"",
            "id=\"providers-empty\"",
            "id=\"provider-create-api-key\"",
            "id=\"provider-edit-api-key\"",
            "id=\"provider-edit-api-key-ref\"",
            "id=\"providers-notice\"",
            "id=\"routing-policy-panel\"",
            "id=\"routing-policy-form\"",
            "id=\"routing-policy-generation\"",
            "id=\"routing-policy-strategy\"",
            "id=\"routing-policy-aliases-body\"",
            "id=\"routing-policy-providers-body\"",
            "id=\"routing-policy-notice\"",
            "id=\"routing-policy-add-alias\"",
        ] {
            assert!(INDEX_HTML.contains(marker), "missing HTML marker {marker}");
        }
        assert!(APP_CSS.contains(":focus-visible"));
        assert!(APP_CSS.contains("@media"));
    }
}
