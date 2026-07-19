//! Compile-time embedded administrator dashboard assets.

use actix_web::http::header::{CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE};
use actix_web::{HttpResponse, web};

const INDEX_HTML: &str = include_str!("admin_dashboard/index.html");
const APP_CSS: &str = include_str!("admin_dashboard/app.css");
const APP_JS: &str = include_str!("admin_dashboard/app.js");
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

/// Register the exact dashboard asset routes.
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/admin/dashboard", web::get().to(dashboard))
        .route("/admin/dashboard/app.css", web::get().to(stylesheet))
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
        for forbidden in [
            "localStorage",
            "sessionStorage",
            "document.cookie",
            "innerHTML",
            "eval(",
            "http://",
            "https://",
        ] {
            assert!(
                !APP_JS.contains(forbidden),
                "unsafe asset token {forbidden}"
            );
        }
        assert!(APP_JS.contains("AbortController"));
        assert!(APP_JS.contains("session.generation !== state.generation"));
        assert!(APP_JS.contains("requestVersion !== state.keyRequestVersion"));
        assert!(APP_JS.contains("requestVersion !== state.teamRequestVersion"));
        assert!(APP_JS.contains("requestVersion !== state.usageRequestVersion"));
        assert!(APP_JS.contains("teams.some((team, index)"));
        assert!(APP_JS.contains("async function logoutRequest"));
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
        ] {
            assert!(INDEX_HTML.contains(marker), "missing HTML marker {marker}");
        }
        assert!(APP_CSS.contains(":focus-visible"));
        assert!(APP_CSS.contains("@media"));
    }
}
