//! AI API endpoints (OpenAI compatible)
//!
//! This module provides OpenAI-compatible API endpoints for AI services.

// Module declarations
mod audio;
mod batches;
pub(crate) mod budgeted;
mod callbacks;
mod chat;
mod completions;
mod context;
mod embeddings;
mod execution;
mod files;
mod fine_tuning;
mod gemini;
mod images;
mod models;
mod moderations;
mod openai_errors;
#[cfg(test)]
mod openapi_contract_tests;
mod provider_config;
#[cfg(test)]
mod provider_selection;
mod rerank;
mod response_cache;
mod responses;
mod responses_stream;
mod route_http;
mod spend;
mod stable_routes;
mod stream_output_guardrail;
mod token_policy;

// Public re-exports for backward compatibility
pub use audio::{audio_speech, audio_transcriptions, audio_translations};
pub use batches::{cancel_batch, create_batch, get_batch, list_batches};
pub use chat::chat_completions;
pub use completions::{completions, engine_completions};
pub use context::{
    api_key_allows_endpoint, check_permission, get_authenticated_api_key, get_authenticated_user,
    get_request_context, handle_ai_request, log_api_usage,
};
pub use embeddings::embeddings;
pub use files::{create_file, delete_file, get_file, get_file_content, list_files};
pub use fine_tuning::{
    cancel_fine_tuning_job, create_fine_tuning_job, get_fine_tuning_job,
    list_fine_tuning_checkpoints, list_fine_tuning_events, list_fine_tuning_jobs,
};
pub use gemini::{
    gemini_generate_content_v1, gemini_generate_content_v1beta, gemini_stream_generate_content_v1,
    gemini_stream_generate_content_v1beta,
};
pub use images::{image_edits, image_generations, image_variations};
pub use models::{get_model, list_models};
pub use moderations::create_moderation;
pub use rerank::rerank;
pub use responses::{
    cancel_response, create_response, delete_response, get_response, list_response_input_items,
};

use crate::core::models::openai::EmbeddingRequest;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, error::InternalError, web};

const STABLE_INFERENCE_OPENAPI: &str = include_str!("../../../../docs/openapi/inference.json");

/// Configure AI API routes
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    configure_routes_impl(cfg, None);
}

/// Configure AI API routes with an explicit JSON body limit.
pub fn configure_routes_with_body_limit(cfg: &mut web::ServiceConfig, max_body_size: usize) {
    configure_routes_impl(cfg, Some(max_body_size));
}

fn configure_routes_impl(cfg: &mut web::ServiceConfig, max_body_size: Option<usize>) {
    cfg.route(
        "/openapi.json",
        web::get().to(|| async {
            HttpResponse::Ok()
                .content_type("application/json")
                .body(STABLE_INFERENCE_OPENAPI)
        }),
    )
    .service(
        web::resource("/completions")
            .app_data(openai_json_error_config(max_body_size))
            .app_data(openai_query_error_config())
            .app_data(openai_path_error_config())
            .route(web::post().to(completions)),
    )
    .service(
        web::resource("/moderations")
            .app_data(openai_json_error_config(max_body_size))
            .app_data(openai_query_error_config())
            .app_data(openai_path_error_config())
            .route(web::post().to(create_moderation)),
    )
    .service(
        web::resource("/rerank")
            .app_data(openai_json_error_config(max_body_size))
            .app_data(openai_query_error_config())
            .app_data(openai_path_error_config())
            .route(web::post().to(rerank)),
    )
    .service(
        web::resource("/engines/{model_id}/completions")
            .app_data(openai_json_error_config(max_body_size))
            .app_data(openai_query_error_config())
            .app_data(openai_path_error_config())
            .route(web::post().to(engine_completions)),
    )
    .service(
        web::resource("/openai/deployments/{model_id}/completions")
            .app_data(openai_json_error_config(max_body_size))
            .app_data(openai_query_error_config())
            .app_data(openai_path_error_config())
            .route(web::post().to(engine_completions)),
    );
    cfg.service(
        web::scope("/v1")
            .app_data(openai_json_error_config(max_body_size))
            .app_data(openai_query_error_config())
            .app_data(openai_path_error_config())
            .configure(stable_routes::configure)
            // Legacy text completions
            .route("/completions", web::post().to(completions))
            .route(
                "/engines/{model_id}/completions",
                web::post().to(engine_completions),
            )
            .route(
                "/engines/{model_id}/embeddings",
                web::post().to(engine_embeddings),
            )
            // Batch processing
            .route("/batches", web::post().to(create_batch))
            .route("/batches", web::get().to(list_batches))
            .route("/batches/{batch_id}", web::get().to(get_batch))
            .route("/batches/{batch_id}/cancel", web::post().to(cancel_batch))
            // Fine-tuning
            .route("/fine_tuning/jobs", web::post().to(create_fine_tuning_job))
            .route("/fine_tuning/jobs", web::get().to(list_fine_tuning_jobs))
            .route(
                "/fine_tuning/jobs/{job_id}",
                web::get().to(get_fine_tuning_job),
            )
            .route(
                "/fine_tuning/jobs/{job_id}/cancel",
                web::post().to(cancel_fine_tuning_job),
            )
            .route(
                "/fine_tuning/jobs/{job_id}/events",
                web::get().to(list_fine_tuning_events),
            )
            .route(
                "/fine_tuning/jobs/{job_id}/checkpoints",
                web::get().to(list_fine_tuning_checkpoints),
            )
            // Files
            .route("/files", web::post().to(files::create_file_http))
            .route("/files", web::get().to(files::list_files_http))
            .route("/files/{file_id}", web::get().to(files::get_file_http))
            .route(
                "/files/{file_id}",
                web::delete().to(files::delete_file_http),
            )
            .route(
                "/files/{file_id}/content",
                web::get().to(files::get_file_content_http),
            )
            .route(
                "/models/{model}:generateContent",
                web::post().to(gemini_generate_content_v1),
            )
            .route(
                "/models/{model}:streamGenerateContent",
                web::post().to(gemini_stream_generate_content_v1),
            )
            .route("/engines", web::get().to(list_models))
            .route("/engines/{model_id}", web::get().to(get_model)),
    );
    cfg.service(
        web::scope("/v1beta")
            .app_data(openai_json_error_config(max_body_size))
            .app_data(openai_query_error_config())
            .app_data(openai_path_error_config())
            .route(
                "/models/{model}:generateContent",
                web::post().to(gemini_generate_content_v1beta),
            )
            .route(
                "/models/{model}:streamGenerateContent",
                web::post().to(gemini_stream_generate_content_v1beta),
            ),
    );
    cfg.service(
        web::scope("/gemini/v1beta")
            .app_data(openai_json_error_config(max_body_size))
            .app_data(openai_query_error_config())
            .app_data(openai_path_error_config())
            .route(
                "/models/{model}:generateContent",
                web::post().to(gemini_generate_content_v1beta),
            )
            .route(
                "/models/{model}:streamGenerateContent",
                web::post().to(gemini_stream_generate_content_v1beta),
            ),
    );
    cfg.service(
        web::scope("/gemini/v1")
            .app_data(openai_json_error_config(max_body_size))
            .app_data(openai_query_error_config())
            .app_data(openai_path_error_config())
            .route(
                "/models/{model}:generateContent",
                web::post().to(gemini_generate_content_v1),
            )
            .route(
                "/models/{model}:streamGenerateContent",
                web::post().to(gemini_stream_generate_content_v1),
            ),
    );
}

pub(crate) fn operation_for_path(path: &str) -> Option<&'static str> {
    let normalized = path.trim_end_matches('/');

    if normalized == "/v1/chat/completions"
        || (normalized.starts_with("/v1/engines/") && normalized.ends_with("/chat/completions"))
        || (normalized.starts_with("/openai/deployments/")
            && normalized.ends_with("/chat/completions"))
    {
        return Some("chat");
    }
    if normalized == "/completions"
        || normalized == "/v1/completions"
        || (normalized.starts_with("/engines/") && normalized.ends_with("/completions"))
        || (normalized.starts_with("/v1/engines/") && normalized.ends_with("/completions"))
        || (normalized.starts_with("/openai/deployments/") && normalized.ends_with("/completions"))
    {
        return Some("completions");
    }
    if normalized == "/v1/embeddings"
        || (normalized.starts_with("/v1/engines/") && normalized.ends_with("/embeddings"))
        || (normalized.starts_with("/openai/deployments/") && normalized.ends_with("/embeddings"))
    {
        return Some("embeddings");
    }
    if normalized.starts_with("/v1/images/") {
        return Some("images");
    }
    if normalized.starts_with("/v1/audio/") {
        return Some("audio");
    }
    if normalized == "/moderations" || normalized == "/v1/moderations" {
        return Some("moderations");
    }
    if normalized == "/rerank" || normalized == "/v1/rerank" {
        return Some("rerank");
    }
    if normalized == "/v1/files" || normalized.starts_with("/v1/files/") {
        return Some("files");
    }
    if normalized == "/v1/fine_tuning/jobs" || normalized.starts_with("/v1/fine_tuning/jobs/") {
        return Some("fine_tuning");
    }
    if normalized == "/v1/models" || normalized == "/v1/engines" {
        return Some("models");
    }
    if normalized.starts_with("/v1/models/") || normalized.starts_with("/v1/engines/") {
        if normalized.contains(":generateContent") || normalized.contains(":streamGenerateContent")
        {
            return Some("chat");
        }
        return Some("models");
    }
    if normalized == "/v1/responses" || normalized.starts_with("/v1/responses/") {
        return Some("responses");
    }
    if normalized == "/v1/batches" || normalized.starts_with("/v1/batches/") {
        return Some("batches");
    }
    if normalized.starts_with("/v1beta/models/")
        || normalized.starts_with("/gemini/v1beta/models/")
        || normalized.starts_with("/gemini/v1/models/")
    {
        return Some("chat");
    }

    None
}

pub(crate) fn is_openai_compatible_path(path: &str) -> bool {
    operation_for_path(path).is_some()
}

pub(crate) fn openai_gateway_error_response(error: &GatewayError) -> HttpResponse {
    openai_errors::gateway_error_response(error)
}

pub(crate) fn openai_internal_error_response(message: impl Into<String>) -> HttpResponse {
    openai_errors::internal_error(message)
}

fn openai_json_error_config(max_body_size: Option<usize>) -> web::JsonConfig {
    let config = web::JsonConfig::default().error_handler(|error, _req| {
        let response =
            openai_errors::validation_error(format!("Invalid JSON request body: {error}"));
        InternalError::from_response(error, response).into()
    });
    match max_body_size {
        Some(limit) => config.limit(limit),
        None => config,
    }
}

fn openai_query_error_config() -> web::QueryConfig {
    web::QueryConfig::default().error_handler(|error, _req| {
        let response =
            openai_errors::validation_error(format!("Invalid query parameters: {error}"));
        InternalError::from_response(error, response).into()
    })
}

fn openai_path_error_config() -> web::PathConfig {
    web::PathConfig::default().error_handler(|error, _req| {
        let response = openai_errors::validation_error(format!("Invalid path parameters: {error}"));
        InternalError::from_response(error, response).into()
    })
}

async fn engine_embeddings(
    state: web::Data<AppState>,
    req: HttpRequest,
    model_id: web::Path<String>,
    request: web::Json<EmbeddingRequest>,
) -> ActixResult<HttpResponse> {
    let mut request = request.into_inner();
    request.model = model_id.into_inner();
    embeddings(state, req, web::Json(request)).await
}

#[cfg(test)]
mod tests {
    use crate::core::types::context::RequestContext;
    use crate::server::HttpServer as GatewayHttpServer;
    use crate::server::middleware::RequestIdMiddleware;
    use actix_web::{App, HttpResponse, http::StatusCode, test, web};
    use serde::Deserialize;
    use serde_json::Value;

    #[actix_web::test]
    async fn public_configure_routes_keeps_actix_callback_shape() {
        let _: fn(&mut web::ServiceConfig) = super::configure_routes;
        let _: fn(&mut web::ServiceConfig, usize) = super::configure_routes_with_body_limit;
    }

    #[actix_web::test]
    async fn public_configure_routes_keeps_actix_default_json_limit() {
        let state = build_no_provider_state().await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(super::configure_routes),
        )
        .await;
        fn payload_with_size(target_size: usize) -> String {
            const PREFIX: &str =
                "{\"model\":\"test-model\",\"messages\":[{\"role\":\"user\",\"content\":\"";
            const SUFFIX: &str = "\"}]}";
            format!(
                "{PREFIX}{}{SUFFIX}",
                "x".repeat(target_size - PREFIX.len() - SUFFIX.len())
            )
        }

        let accepted_request = test::TestRequest::post()
            .uri("/v1/chat/completions")
            .insert_header(("content-type", "application/json"))
            .set_payload(payload_with_size(2 * 1024 * 1024 - 1024))
            .to_request();
        let accepted_response = test::call_service(&app, accepted_request).await;
        let accepted_body = test::read_body(accepted_response).await;
        assert!(!String::from_utf8_lossy(&accepted_body).contains("Invalid JSON request body"));

        let rejected_request = test::TestRequest::post()
            .uri("/v1/chat/completions")
            .insert_header(("content-type", "application/json"))
            .set_payload(payload_with_size(2 * 1024 * 1024 + 1024))
            .to_request();
        let rejected_response = test::call_service(&app, rejected_request).await;
        assert_eq!(rejected_response.status(), StatusCode::BAD_REQUEST);
        let rejected_body = test::read_body(rejected_response).await;
        assert!(String::from_utf8_lossy(&rejected_body).contains("Invalid JSON request body"));
    }

    async fn build_no_provider_state() -> crate::server::state::AppState {
        let mut config = crate::server::valid_test_config();
        config.gateway.providers[0].enabled = false;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;

        GatewayHttpServer::new(&config)
            .await
            .expect("gateway server should initialize")
            .state()
            .clone()
    }

    #[actix_web::test]
    async fn test_get_request_context() {
        // This test would need a mock HttpRequest in a real implementation
        // For now, we'll test the basic functionality
        let context = RequestContext::new();
        assert!(!context.request_id.is_empty());
        assert!(context.user_agent.is_none());
    }

    #[actix_web::test]
    async fn test_batch_routes_mounted_with_expected_methods() {
        let state = build_no_provider_state().await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(super::configure_routes),
        )
        .await;

        let create_req = test::TestRequest::post()
            .uri("/v1/batches")
            .set_json(serde_json::json!({
                "input_file_id": "file_123",
                "endpoint": "/v1/chat/completions",
                "completion_window": "24h"
            }))
            .to_request();
        let create_resp = test::call_service(&app, create_req).await;
        assert_eq!(create_resp.status(), StatusCode::BAD_REQUEST);
        let create_body: Value = test::read_body_json(create_resp).await;
        assert!(
            create_body["error"]["message"]
                .as_str()
                .expect("error message")
                .contains("Batch API requires")
        );
        assert_eq!(create_body["error"]["type"], "invalid_request_error");
        assert_eq!(create_body["error"]["code"], "invalid_request");

        let list_req = test::TestRequest::get().uri("/v1/batches").to_request();
        let list_resp = test::call_service(&app, list_req).await;
        assert_eq!(list_resp.status(), StatusCode::BAD_REQUEST);

        let get_req = test::TestRequest::get()
            .uri("/v1/batches/batch_test")
            .to_request();
        let get_resp = test::call_service(&app, get_req).await;
        assert_eq!(get_resp.status(), StatusCode::BAD_REQUEST);

        let cancel_req = test::TestRequest::post()
            .uri("/v1/batches/batch_test/cancel")
            .to_request();
        let cancel_resp = test::call_service(&app, cancel_req).await;
        assert_eq!(cancel_resp.status(), StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_response_lifecycle_routes_mounted_with_expected_methods() {
        let state = build_no_provider_state().await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(super::configure_routes),
        )
        .await;

        let create_req = test::TestRequest::post()
            .uri("/v1/responses")
            .set_json(serde_json::json!({
                "model": "gpt-4o",
                "input": "hello"
            }))
            .to_request();
        let create_resp = test::call_service(&app, create_req).await;
        assert_eq!(create_resp.status(), StatusCode::BAD_REQUEST);
        let create_body: Value = test::read_body_json(create_resp).await;
        assert!(
            create_body["error"]["message"]
                .as_str()
                .expect("error message")
                .contains("store=false")
        );

        let get_req = test::TestRequest::get()
            .uri("/v1/responses/resp_missing")
            .to_request();
        let get_resp = test::call_service(&app, get_req).await;
        assert_eq!(get_resp.status(), StatusCode::NOT_FOUND);

        let delete_req = test::TestRequest::delete()
            .uri("/v1/responses/resp_missing")
            .to_request();
        let delete_resp = test::call_service(&app, delete_req).await;
        assert_eq!(delete_resp.status(), StatusCode::NOT_FOUND);

        let cancel_req = test::TestRequest::post()
            .uri("/v1/responses/resp_missing/cancel")
            .to_request();
        let cancel_resp = test::call_service(&app, cancel_req).await;
        assert_eq!(cancel_resp.status(), StatusCode::NOT_FOUND);

        let input_items_req = test::TestRequest::get()
            .uri("/v1/responses/resp_missing/input_items?limit=1&order=asc")
            .to_request();
        let input_items_resp = test::call_service(&app, input_items_req).await;
        assert_eq!(input_items_resp.status(), StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn test_engine_alias_routes_are_mounted_with_expected_methods() {
        let state = build_no_provider_state().await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(super::configure_routes),
        )
        .await;

        let list_req = test::TestRequest::get().uri("/v1/engines").to_request();
        let list_resp = test::call_service(&app, list_req).await;
        assert_eq!(list_resp.status(), StatusCode::OK);

        let get_missing_req = test::TestRequest::get()
            .uri("/v1/engines/missing-model")
            .to_request();
        let get_missing_resp = test::call_service(&app, get_missing_req).await;
        assert_eq!(get_missing_resp.status(), StatusCode::NOT_FOUND);

        let embeddings_req = test::TestRequest::post()
            .uri("/v1/engines/text-embedding-3-small/embeddings")
            .set_json(serde_json::json!({
                "model": "body-model",
                "input": "Hello"
            }))
            .to_request();
        let embeddings_resp = test::call_service(&app, embeddings_req).await;
        let embeddings_status = embeddings_resp.status();
        let embeddings_body: Value = test::read_body_json(embeddings_resp).await;
        assert_eq!(embeddings_status, StatusCode::NOT_FOUND);
        assert!(
            embeddings_body
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .is_some()
        );
    }

    #[actix_web::test]
    async fn test_root_engine_aliases_remain_unmounted() {
        let state = build_no_provider_state().await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(super::configure_routes),
        )
        .await;

        let req = test::TestRequest::get().uri("/engines").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn openai_routes_use_openai_shape_for_json_extractor_errors() {
        let state = build_no_provider_state().await;
        let app = test::init_service(
            App::new()
                .wrap(RequestIdMiddleware)
                .app_data(web::Data::new(state))
                .configure(super::configure_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/chat/completions")
            .insert_header(("x-request-id", "req-json-error"))
            .insert_header(("content-type", "application/json"))
            .set_payload(r#"{"model":"gpt-4o","#)
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.headers()
                .get("x-request-id")
                .and_then(|value| value.to_str().ok()),
            Some("req-json-error")
        );
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["error"]["type"], "invalid_request_error");
        assert_eq!(body["error"]["code"], "invalid_request");
        assert_eq!(body["error"]["request_id"], "req-json-error");
        assert!(body.get("success").is_none());
    }

    #[actix_web::test]
    async fn openai_routes_use_openai_shape_for_query_extractor_errors() {
        let state = build_no_provider_state().await;
        let app = test::init_service(
            App::new()
                .wrap(RequestIdMiddleware)
                .app_data(web::Data::new(state))
                .configure(super::configure_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/v1/batches?limit=not-a-number")
            .insert_header(("x-request-id", "req-query-error"))
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["error"]["type"], "invalid_request_error");
        assert_eq!(body["error"]["code"], "invalid_request");
        assert_eq!(body["error"]["request_id"], "req-query-error");
        assert!(
            body["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("Invalid query parameters"))
        );
    }

    #[actix_web::test]
    async fn openai_path_config_uses_openai_shape_for_path_extractor_errors() {
        let app = test::init_service(
            App::new()
                .wrap(RequestIdMiddleware)
                .app_data(super::openai_path_error_config())
                .route(
                    "/v1/test/{id}",
                    web::get().to(|_: web::Path<u32>| async { HttpResponse::Ok().finish() }),
                ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/v1/test/not-a-number")
            .insert_header(("x-request-id", "req-path-error"))
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["error"]["type"], "invalid_request_error");
        assert_eq!(body["error"]["code"], "invalid_request");
        assert_eq!(body["error"]["request_id"], "req-path-error");
        assert!(
            body["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("Invalid path parameters"))
        );
    }

    #[allow(dead_code)]
    #[derive(Deserialize)]
    struct NonAiJsonPayload {
        value: u32,
    }

    async fn non_ai_json_route(_payload: web::Json<NonAiJsonPayload>) -> HttpResponse {
        HttpResponse::Ok().finish()
    }

    #[actix_web::test]
    async fn openai_extractor_configs_do_not_leak_to_later_non_ai_routes() {
        let state = build_no_provider_state().await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(super::configure_routes)
                .route("/non-ai-json", web::post().to(non_ai_json_route)),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/non-ai-json")
            .insert_header(("content-type", "application/json"))
            .set_payload(r#"{"value":"not-a-number"}"#)
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body = test::read_body(resp).await;
        let body = String::from_utf8_lossy(&body);
        assert!(!body.contains("invalid_request_error"));
        assert!(!body.contains("invalid_request"));
    }
}
