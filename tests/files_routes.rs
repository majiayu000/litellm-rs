#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use actix_web::http::{StatusCode, header};
    use actix_web::{App, test, web};
    use litellm_rs::Config;
    use litellm_rs::server::http::HttpServer;
    use serde_json::Value;

    async fn build_files_state(local_path: String) -> litellm_rs::server::state::AppState {
        let mut config = Config::default();
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = false;
        config.gateway.auth.allow_anonymous = true;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.storage.files.local_path = Some(local_path);
        config.gateway.pricing.source = Some("config/model_prices_extended.json".to_string());

        let server = HttpServer::new(&config)
            .await
            .expect("gateway server should initialize for files route test");
        let state = server.state().clone();
        state
            .storage
            .migrate()
            .await
            .expect("in-memory database migrations should run");
        state
    }

    fn multipart_body(boundary: &str, purpose: Option<&str>, file_content: &[u8]) -> Vec<u8> {
        let mut body = Vec::new();
        if let Some(purpose) = purpose {
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            body.extend_from_slice(b"Content-Disposition: form-data; name=\"purpose\"\r\n\r\n");
            body.extend_from_slice(purpose.as_bytes());
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            b"Content-Disposition: form-data; name=\"file\"; filename=\"batch.jsonl\"\r\n",
        );
        body.extend_from_slice(b"Content-Type: application/jsonl\r\n\r\n");
        body.extend_from_slice(file_content);
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        body
    }

    #[tokio::test]
    async fn upload_list_retrieve_content_and_delete_file() {
        let tempdir = tempfile::tempdir().expect("temp dir should be created");
        let local_path = tempdir.path().join("files").to_string_lossy().into_owned();
        let state = build_files_state(local_path).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let boundary = "litellm-rs-files-boundary";
        let content = br#"{"custom_id":"1"}"#;

        let upload_req = test::TestRequest::post()
            .uri("/v1/files")
            .insert_header((
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(multipart_body(boundary, Some("batch"), content))
            .to_request();
        let upload_resp = test::call_service(&app, upload_req).await;

        assert_eq!(upload_resp.status(), StatusCode::OK);
        let uploaded: Value = test::read_body_json(upload_resp).await;
        let file_id = uploaded["id"].as_str().expect("file id").to_string();
        assert_eq!(uploaded["object"], "file");
        assert_eq!(uploaded["filename"], "batch.jsonl");
        assert_eq!(uploaded["purpose"], "batch");
        assert_eq!(uploaded["bytes"], content.len() as u64);

        let get_req = test::TestRequest::get()
            .uri(&format!("/v1/files/{file_id}"))
            .to_request();
        let get_resp = test::call_service(&app, get_req).await;
        assert_eq!(get_resp.status(), StatusCode::OK);
        let fetched: Value = test::read_body_json(get_resp).await;
        assert_eq!(fetched["id"], file_id);
        assert_eq!(fetched["purpose"], "batch");

        let list_req = test::TestRequest::get().uri("/v1/files").to_request();
        let list_resp = test::call_service(&app, list_req).await;
        assert_eq!(list_resp.status(), StatusCode::OK);
        let listed: Value = test::read_body_json(list_resp).await;
        assert!(
            listed["data"]
                .as_array()
                .expect("files list")
                .iter()
                .any(|file| file["id"] == file_id)
        );

        let content_req = test::TestRequest::get()
            .uri(&format!("/v1/files/{file_id}/content"))
            .to_request();
        let content_resp = test::call_service(&app, content_req).await;
        assert_eq!(content_resp.status(), StatusCode::OK);
        let body = test::read_body(content_resp).await;
        assert_eq!(body.as_ref(), content);

        let delete_req = test::TestRequest::delete()
            .uri(&format!("/v1/files/{file_id}"))
            .to_request();
        let delete_resp = test::call_service(&app, delete_req).await;
        assert_eq!(delete_resp.status(), StatusCode::OK);
        let deleted: Value = test::read_body_json(delete_resp).await;
        assert_eq!(deleted["id"], file_id);
        assert_eq!(deleted["deleted"], true);

        let get_deleted_req = test::TestRequest::get()
            .uri(&format!("/v1/files/{file_id}"))
            .to_request();
        let get_deleted_resp = test::call_service(&app, get_deleted_req).await;
        assert_eq!(get_deleted_resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn upload_requires_supported_purpose() {
        let tempdir = tempfile::tempdir().expect("temp dir should be created");
        let local_path = tempdir.path().join("files").to_string_lossy().into_owned();
        let state = build_files_state(local_path).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let boundary = "litellm-rs-files-boundary";

        let missing_req = test::TestRequest::post()
            .uri("/v1/files")
            .insert_header((
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(multipart_body(boundary, None, b"content"))
            .to_request();
        let missing_resp = test::call_service(&app, missing_req).await;
        assert_eq!(missing_resp.status(), StatusCode::BAD_REQUEST);
        let missing_body: Value = test::read_body_json(missing_resp).await;
        assert_eq!(missing_body["error"]["message"], "purpose is required");

        let invalid_req = test::TestRequest::post()
            .uri("/v1/files")
            .insert_header((
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(multipart_body(
                boundary,
                Some("invalid-purpose"),
                b"content",
            ))
            .to_request();
        let invalid_resp = test::call_service(&app, invalid_req).await;
        assert_eq!(invalid_resp.status(), StatusCode::BAD_REQUEST);
        let invalid_body: Value = test::read_body_json(invalid_resp).await;
        assert!(
            invalid_body["error"]["message"]
                .as_str()
                .expect("error message")
                .contains("Unsupported file purpose")
        );
    }

    #[tokio::test]
    async fn delete_missing_file_returns_404() {
        let tempdir = tempfile::tempdir().expect("temp dir should be created");
        let local_path = tempdir.path().join("files").to_string_lossy().into_owned();
        let state = build_files_state(local_path).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri("/v1/files/550e8400-e29b-41d4-a716-446655440000")
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["error"]["type"], "invalid_request_error");
        assert_eq!(body["error"]["code"], "not_found");
    }
}
