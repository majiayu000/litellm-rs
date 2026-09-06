//! Admin control-plane OpenAPI document.
//!
//! Served at `GET /admin/openapi.json` behind [`super::admin::require_admin`].
//! The stable inference contract at `GET /openapi.json` is independent.

use super::admin::require_admin;
use crate::server::state::AppState;
use actix_web::{HttpRequest, HttpResponse, web};

const ADMIN_OPENAPI: &str = include_str!("../../../docs/openapi/admin.json");
const ADMIN_ERROR: &str = "Admin role required for admin OpenAPI";

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AdminMethod {
    Delete,
    Get,
    Patch,
    Post,
    Put,
}

#[cfg(test)]
impl AdminMethod {
    const fn openapi_key(self) -> &'static str {
        match self {
            Self::Delete => "delete",
            Self::Get => "get",
            Self::Patch => "patch",
            Self::Post => "post",
            Self::Put => "put",
        }
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AdminControlPlaneRoute {
    path: &'static str,
    method: AdminMethod,
}

/// Live control-plane HTTP operations documented in `docs/openapi/admin.json`.
///
/// Dashboard static assets under `/admin/dashboard*` are excluded.
#[cfg(test)]
const ADMIN_CONTROL_PLANE_ROUTES: &[AdminControlPlaneRoute] = &[
    AdminControlPlaneRoute {
        path: "/admin/openapi.json",
        method: AdminMethod::Get,
    },
    AdminControlPlaneRoute {
        path: "/admin/cache",
        method: AdminMethod::Get,
    },
    AdminControlPlaneRoute {
        path: "/admin/cache/status",
        method: AdminMethod::Get,
    },
    AdminControlPlaneRoute {
        path: "/admin/cache/clear",
        method: AdminMethod::Post,
    },
    AdminControlPlaneRoute {
        path: "/admin/providers",
        method: AdminMethod::Get,
    },
    AdminControlPlaneRoute {
        path: "/admin/providers",
        method: AdminMethod::Post,
    },
    AdminControlPlaneRoute {
        path: "/admin/providers/{name}",
        method: AdminMethod::Patch,
    },
    AdminControlPlaneRoute {
        path: "/admin/providers/{name}",
        method: AdminMethod::Delete,
    },
    AdminControlPlaneRoute {
        path: "/admin/request-ledger",
        method: AdminMethod::Get,
    },
    AdminControlPlaneRoute {
        path: "/admin/routing/inventory",
        method: AdminMethod::Get,
    },
    AdminControlPlaneRoute {
        path: "/admin/routing/policy",
        method: AdminMethod::Get,
    },
    AdminControlPlaneRoute {
        path: "/admin/routing/policy",
        method: AdminMethod::Put,
    },
    AdminControlPlaneRoute {
        path: "/auth/register",
        method: AdminMethod::Post,
    },
    AdminControlPlaneRoute {
        path: "/auth/login",
        method: AdminMethod::Post,
    },
    AdminControlPlaneRoute {
        path: "/auth/logout",
        method: AdminMethod::Post,
    },
    AdminControlPlaneRoute {
        path: "/auth/refresh",
        method: AdminMethod::Post,
    },
    AdminControlPlaneRoute {
        path: "/auth/forgot-password",
        method: AdminMethod::Post,
    },
    AdminControlPlaneRoute {
        path: "/auth/reset-password",
        method: AdminMethod::Post,
    },
    AdminControlPlaneRoute {
        path: "/auth/verify-email",
        method: AdminMethod::Post,
    },
    AdminControlPlaneRoute {
        path: "/auth/change-password",
        method: AdminMethod::Post,
    },
    AdminControlPlaneRoute {
        path: "/auth/me",
        method: AdminMethod::Get,
    },
    AdminControlPlaneRoute {
        path: "/v1/keys",
        method: AdminMethod::Get,
    },
    AdminControlPlaneRoute {
        path: "/v1/keys",
        method: AdminMethod::Post,
    },
    AdminControlPlaneRoute {
        path: "/v1/keys/verify",
        method: AdminMethod::Post,
    },
    AdminControlPlaneRoute {
        path: "/v1/keys/{id}",
        method: AdminMethod::Get,
    },
    AdminControlPlaneRoute {
        path: "/v1/keys/{id}",
        method: AdminMethod::Put,
    },
    AdminControlPlaneRoute {
        path: "/v1/keys/{id}",
        method: AdminMethod::Delete,
    },
    AdminControlPlaneRoute {
        path: "/v1/keys/{id}/rotate",
        method: AdminMethod::Post,
    },
    AdminControlPlaneRoute {
        path: "/v1/keys/{id}/usage",
        method: AdminMethod::Get,
    },
    AdminControlPlaneRoute {
        path: "/v1/teams",
        method: AdminMethod::Get,
    },
    AdminControlPlaneRoute {
        path: "/v1/teams",
        method: AdminMethod::Post,
    },
    AdminControlPlaneRoute {
        path: "/v1/teams/{id}",
        method: AdminMethod::Get,
    },
    AdminControlPlaneRoute {
        path: "/v1/teams/{id}",
        method: AdminMethod::Put,
    },
    AdminControlPlaneRoute {
        path: "/v1/teams/{id}",
        method: AdminMethod::Delete,
    },
    AdminControlPlaneRoute {
        path: "/v1/teams/{id}/members",
        method: AdminMethod::Get,
    },
    AdminControlPlaneRoute {
        path: "/v1/teams/{id}/members",
        method: AdminMethod::Post,
    },
    AdminControlPlaneRoute {
        path: "/v1/teams/{id}/members/{user_id}",
        method: AdminMethod::Delete,
    },
    AdminControlPlaneRoute {
        path: "/v1/teams/{id}/members/{user_id}/role",
        method: AdminMethod::Put,
    },
    AdminControlPlaneRoute {
        path: "/v1/teams/{id}/usage",
        method: AdminMethod::Get,
    },
    AdminControlPlaneRoute {
        path: "/v1/budget/providers",
        method: AdminMethod::Get,
    },
    AdminControlPlaneRoute {
        path: "/v1/budget/providers",
        method: AdminMethod::Post,
    },
    AdminControlPlaneRoute {
        path: "/v1/budget/providers/{name}",
        method: AdminMethod::Get,
    },
    AdminControlPlaneRoute {
        path: "/v1/budget/providers/{name}",
        method: AdminMethod::Delete,
    },
    AdminControlPlaneRoute {
        path: "/v1/budget/providers/{name}/reset",
        method: AdminMethod::Post,
    },
    AdminControlPlaneRoute {
        path: "/v1/budget/models",
        method: AdminMethod::Get,
    },
    AdminControlPlaneRoute {
        path: "/v1/budget/models",
        method: AdminMethod::Post,
    },
    AdminControlPlaneRoute {
        path: "/v1/budget/models/{name}",
        method: AdminMethod::Get,
    },
    AdminControlPlaneRoute {
        path: "/v1/budget/models/{name}",
        method: AdminMethod::Delete,
    },
    AdminControlPlaneRoute {
        path: "/v1/budget/models/{name}/reset",
        method: AdminMethod::Post,
    },
    AdminControlPlaneRoute {
        path: "/v1/budget/summary",
        method: AdminMethod::Get,
    },
];

#[cfg(test)]
const fn admin_control_plane_routes() -> &'static [AdminControlPlaneRoute] {
    ADMIN_CONTROL_PLANE_ROUTES
}

async fn serve_admin_openapi(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> actix_web::Result<HttpResponse> {
    if let Some(forbidden) = require_admin(&req, &state, "inspect admin OpenAPI", ADMIN_ERROR) {
        return Ok(forbidden);
    }

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(ADMIN_OPENAPI))
}

pub(super) fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/admin/openapi.json", web::get().to(serve_admin_openapi));
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
    use crate::server::HttpServer;
    use actix_web::dev::Service;
    use actix_web::{App, HttpMessage, http::StatusCode, test as actix_test};
    use serde_json::Value;
    use std::collections::HashSet;
    use std::path::PathBuf;

    const HTTP_METHODS: &[&str] = &["delete", "get", "patch", "post", "put"];
    const FORBIDDEN_RESPONSE_FIELDS: &[&str] = &[
        "api_key",
        "Authorization",
        "authorization",
        "prompt",
        "body",
        "messages",
        "choices",
        "headers",
        "password_hash",
        "secret",
    ];

    fn contract() -> Value {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("docs")
            .join("openapi")
            .join("admin.json");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        serde_json::from_str(&text)
            .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
    }

    fn contract_operations(contract: &Value) -> HashSet<(String, String)> {
        let paths = contract["paths"]
            .as_object()
            .expect("OpenAPI paths must be an object");
        paths
            .iter()
            .flat_map(|(path, item)| {
                item.as_object().into_iter().flat_map(move |item| {
                    item.keys()
                        .filter(|method| HTTP_METHODS.contains(&method.as_str()))
                        .map(move |method| (path.clone(), method.clone()))
                })
            })
            .collect()
    }

    fn schema_by_ref<'a>(contract: &'a Value, reference: &str) -> &'a Value {
        let name = reference
            .strip_prefix("#/components/schemas/")
            .unwrap_or_else(|| panic!("expected schema ref, got {reference}"));
        &contract["components"]["schemas"][name]
    }

    fn collect_property_names(contract: &Value, schema: &Value, names: &mut HashSet<String>) {
        if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
            collect_property_names(contract, schema_by_ref(contract, reference), names);
            return;
        }
        if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
            for (name, child) in properties {
                names.insert(name.clone());
                collect_property_names(contract, child, names);
            }
        }
        if let Some(items) = schema.get("items") {
            collect_property_names(contract, items, names);
        }
        for key in ["oneOf", "anyOf", "allOf"] {
            if let Some(Value::Array(options)) = schema.get(key) {
                for option in options {
                    collect_property_names(contract, option, names);
                }
            }
        }
        if let Some(additional) = schema.get("additionalProperties")
            && additional.is_object()
        {
            collect_property_names(contract, additional, names);
        }
    }

    fn response_schema_property_names(contract: &Value) -> HashSet<String> {
        let mut names = HashSet::new();
        let paths = contract["paths"].as_object().expect("paths");
        for item in paths.values() {
            let Some(item) = item.as_object() else {
                continue;
            };
            for method in HTTP_METHODS {
                let Some(operation) = item.get(*method) else {
                    continue;
                };
                let Some(responses) = operation["responses"].as_object() else {
                    continue;
                };
                for response in responses.values() {
                    let resolved =
                        if let Some(reference) = response.get("$ref").and_then(Value::as_str) {
                            let name = reference
                                .strip_prefix("#/components/responses/")
                                .expect("response ref");
                            &contract["components"]["responses"][name]
                        } else {
                            response
                        };
                    let Some(content) = resolved.get("content").and_then(Value::as_object) else {
                        continue;
                    };
                    for media in content.values() {
                        if let Some(schema) = media.get("schema") {
                            collect_property_names(contract, schema, &mut names);
                        }
                    }
                }
            }
        }
        names
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

    #[test]
    fn admin_openapi_contract_matches_registered_route_surface() {
        let contract = contract();
        assert_eq!(contract["openapi"], "3.2.0");
        assert_eq!(
            contract["components"]["securitySchemes"]["bearerAuth"]["scheme"],
            "bearer"
        );
        assert_eq!(contract["security"][0]["bearerAuth"], serde_json::json!([]));

        let paths = contract["paths"]
            .as_object()
            .expect("OpenAPI paths must be an object");
        let mut operation_ids = HashSet::new();

        for route in admin_control_plane_routes() {
            let method = route.method.openapi_key();
            let operation = paths
                .get(route.path)
                .and_then(|item| item.get(method))
                .unwrap_or_else(|| {
                    panic!(
                        "missing {method} {} from admin OpenAPI contract",
                        route.path
                    )
                });
            let operation_id = operation["operationId"]
                .as_str()
                .unwrap_or_else(|| panic!("missing operationId for {method} {}", route.path));
            assert!(
                operation_ids.insert(operation_id),
                "duplicate operationId: {operation_id}"
            );
            assert!(
                operation.get("responses").is_some(),
                "missing responses for {method} {}",
                route.path
            );
        }

        let registered = admin_control_plane_routes()
            .iter()
            .map(|route| {
                (
                    route.path.to_string(),
                    route.method.openapi_key().to_string(),
                )
            })
            .collect::<HashSet<_>>();
        assert_eq!(
            contract_operations(&contract),
            registered,
            "admin contract operations must exactly match the control-plane inventory"
        );
        assert!(
            !paths
                .keys()
                .any(|path| path.starts_with("/admin/dashboard"))
        );
        assert!(!paths.contains_key("/mcp"));
        assert!(!paths.contains_key("/a2a"));
    }

    #[test]
    fn admin_openapi_declares_auth_errors_and_cursor_pagination() {
        let contract = contract();
        let error = &contract["components"]["schemas"]["AdminErrorBody"];
        assert_eq!(error["properties"]["success"]["const"], false);
        assert_eq!(error["properties"]["error"]["type"], "string");
        assert_eq!(
            contract["components"]["responses"]["Forbidden"]["content"]["application/json"]["schema"]
                ["$ref"],
            "#/components/schemas/AdminErrorBody"
        );

        let ledger = &contract["paths"]["/admin/request-ledger"]["get"];
        assert_eq!(
            ledger["responses"]["403"]["$ref"],
            "#/components/responses/Forbidden"
        );
        let names = ledger["parameters"]
            .as_array()
            .expect("ledger query parameters")
            .iter()
            .filter_map(|parameter| parameter["name"].as_str())
            .collect::<HashSet<_>>();
        assert_eq!(
            names,
            HashSet::from([
                "finished_after",
                "finished_before",
                "request_id",
                "model",
                "provider",
                "terminal_status",
                "cursor",
                "limit",
            ])
        );
        assert_eq!(
            contract["paths"]["/auth/login"]["post"]["security"],
            serde_json::json!([])
        );
        assert!(
            contract["paths"]["/admin/providers"]["get"]["security"].is_null()
                || contract["paths"]["/admin/providers"]["get"]
                    .get("security")
                    .is_none()
        );
    }

    #[test]
    fn admin_openapi_response_schemas_omit_sensitive_fields() {
        let contract = contract();
        let names = response_schema_property_names(&contract);
        for forbidden in FORBIDDEN_RESPONSE_FIELDS {
            assert!(
                !names.contains(*forbidden),
                "response schemas must not include {forbidden}: {names:?}"
            );
        }
        assert!(names.contains("api_key_ref"));
        assert!(names.contains("api_key_id"));
        assert!(names.contains("prompt_tokens"));
        assert!(names.contains("key_prefix"));
        let encoded = contract.to_string();
        assert!(!encoded.contains("sk-"));
        assert!(!encoded.contains("\"Authorization\""));
    }

    #[actix_web::test]
    async fn admin_openapi_requires_admin_identity() {
        let state = test_state(base_test_config(true)).await;
        let app = admin_app(state, None).await;
        let resp = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri("/admin/openapi.json")
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let body: Value = actix_test::read_body_json(resp).await;
        assert_eq!(body["success"], false);
        assert_eq!(body["error"], ADMIN_ERROR);
    }

    #[actix_web::test]
    async fn admin_openapi_rejects_non_admin_user() {
        let state = test_state(base_test_config(true)).await;
        let app = admin_app(state, Some(make_test_user(UserRole::User))).await;
        let resp = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri("/admin/openapi.json")
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    async fn admin_openapi_serves_document_for_admin() {
        let state = test_state(base_test_config(true)).await;
        let app = admin_app(state, Some(make_test_user(UserRole::Admin))).await;
        let resp = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri("/admin/openapi.json")
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get(actix_web::http::header::CONTENT_TYPE),
            Some(&actix_web::http::header::HeaderValue::from_static(
                "application/json"
            ))
        );
        let body: Value = actix_test::read_body_json(resp).await;
        assert_eq!(body["openapi"], "3.2.0");
        assert!(body["paths"]["/admin/routing/inventory"]["get"].is_object());
        assert!(body["paths"]["/v1/keys"]["post"].is_object());
        assert!(body["paths"].get("/admin/dashboard").is_none());
        assert!(body["paths"].get("/v1/chat/completions").is_none());
    }
}
