//! AI API endpoints (OpenAI compatible)
//!
//! This module provides OpenAI-compatible API endpoints for AI services.

// Module declarations
mod audio;
mod batches;
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
mod provider_config;
#[cfg(test)]
mod provider_selection;
mod rerank;
mod responses;
mod responses_stream;
mod spend;

// Public re-exports for backward compatibility
pub use audio::{audio_speech, audio_transcriptions, audio_translations};
pub use batches::{cancel_batch, create_batch, get_batch, list_batches};
pub use chat::chat_completions;
pub use completions::{completions, engine_completions};
pub use context::{
    check_permission, get_authenticated_api_key, get_authenticated_user, get_request_context,
    handle_ai_request, log_api_usage,
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
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};

/// Configure AI API routes
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/completions", web::post().to(completions))
        .route("/moderations", web::post().to(create_moderation))
        .route("/rerank", web::post().to(rerank))
        .route(
            "/engines/{model_id}/completions",
            web::post().to(engine_completions),
        )
        .route(
            "/openai/deployments/{model_id}/completions",
            web::post().to(engine_completions),
        );
    cfg.service(
        web::scope("/v1")
            // Legacy text completions
            .route("/completions", web::post().to(completions))
            .route(
                "/engines/{model_id}/completions",
                web::post().to(engine_completions),
            )
            // Chat completions
            .route("/chat/completions", web::post().to(chat_completions))
            // Responses API
            .route("/responses", web::post().to(create_response))
            .route("/responses/{response_id}", web::get().to(get_response))
            .route(
                "/responses/{response_id}",
                web::delete().to(delete_response),
            )
            .route(
                "/responses/{response_id}/cancel",
                web::post().to(cancel_response),
            )
            .route(
                "/responses/{response_id}/input_items",
                web::get().to(list_response_input_items),
            )
            // Embeddings
            .route("/embeddings", web::post().to(embeddings))
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
            .route("/files", web::post().to(create_file))
            .route("/files", web::get().to(list_files))
            .route("/files/{file_id}", web::get().to(get_file))
            .route("/files/{file_id}", web::delete().to(delete_file))
            .route("/files/{file_id}/content", web::get().to(get_file_content))
            // Image generation
            .route("/images/generations", web::post().to(image_generations))
            .route("/images/edits", web::post().to(image_edits))
            .route("/images/variations", web::post().to(image_variations))
            // Moderations
            .route("/moderations", web::post().to(create_moderation))
            // Rerank
            .route("/rerank", web::post().to(rerank))
            // Models
            .route("/models", web::get().to(list_models))
            .route("/models/{model_id}", web::get().to(get_model))
            .route(
                "/models/{model}:generateContent",
                web::post().to(gemini_generate_content_v1),
            )
            .route(
                "/models/{model}:streamGenerateContent",
                web::post().to(gemini_stream_generate_content_v1),
            )
            .route("/engines", web::get().to(list_models))
            .route("/engines/{model_id}", web::get().to(get_model))
            // Audio (future implementation)
            .route(
                "/audio/transcriptions",
                web::post().to(audio_transcriptions),
            )
            .route("/audio/translations", web::post().to(audio_translations))
            .route("/audio/speech", web::post().to(audio_speech)),
    );
    cfg.service(
        web::scope("/v1beta")
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
    use crate::Config;
    use crate::core::types::context::RequestContext;
    use crate::server::HttpServer as GatewayHttpServer;
    use actix_web::{App, http::StatusCode, test, web};
    use serde_json::Value;

    async fn build_no_provider_state() -> crate::server::state::AppState {
        let mut config = Config::default();
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
        assert_eq!(create_resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let create_body: Value = test::read_body_json(create_resp).await;
        assert!(
            create_body["error"]["message"]
                .as_str()
                .expect("error message")
                .contains("Batch API requires")
        );

        let list_req = test::TestRequest::get().uri("/v1/batches").to_request();
        let list_resp = test::call_service(&app, list_req).await;
        assert_eq!(list_resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let get_req = test::TestRequest::get()
            .uri("/v1/batches/batch_test")
            .to_request();
        let get_resp = test::call_service(&app, get_req).await;
        assert_eq!(get_resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let cancel_req = test::TestRequest::post()
            .uri("/v1/batches/batch_test/cancel")
            .to_request();
        let cancel_resp = test::call_service(&app, cancel_req).await;
        assert_eq!(cancel_resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
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
}
