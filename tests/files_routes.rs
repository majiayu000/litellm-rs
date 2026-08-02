#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use actix_web::http::{StatusCode, header};
    use actix_web::{App, test, web};
    use litellm_rs::Config;
    use litellm_rs::core::models::user::types::{User, UserRole, UserStatus};
    use litellm_rs::core::models::{ApiKey, Metadata, UsageStats};
    use litellm_rs::server::http::HttpServer;
    use litellm_rs::server::middleware::AuthMiddleware;
    use litellm_rs::utils::auth::crypto::keys::{extract_api_key_prefix, hash_api_key};
    use serde_json::Value;
    use uuid::Uuid;

    async fn build_files_state(local_path: String) -> litellm_rs::server::state::AppState {
        build_files_state_with_auth(local_path, false).await
    }

    async fn build_files_state_with_auth(
        local_path: String,
        auth_enabled: bool,
    ) -> litellm_rs::server::state::AppState {
        let mut config = Config::default();
        config.gateway.auth.enable_jwt = auth_enabled;
        config.gateway.auth.enable_api_key = auth_enabled;
        config.gateway.auth.allow_anonymous = !auth_enabled;
        config.gateway.auth.jwt_secret = "AaaAaaAaaAaaAaaAaaAaaAaaAaaAaa1!".to_string();
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

    async fn seed_principal(
        state: &litellm_rs::server::state::AppState,
        name: &str,
        role: UserRole,
        team_id: Option<Uuid>,
        permissions: Vec<String>,
    ) -> (User, String) {
        let mut user = User::new(
            name.to_string(),
            format!("{name}@example.com"),
            "hashed-password".to_string(),
        );
        user.role = role;
        user.status = UserStatus::Active;
        let user = state.storage.db().create_user(&user).await.unwrap();

        let raw_key = format!("gw-files-{name}-{}", Uuid::new_v4());
        let api_key = ApiKey {
            metadata: Metadata::new(),
            name: format!("{name}-key"),
            key_hash: hash_api_key(&raw_key, None),
            key_prefix: extract_api_key_prefix(&raw_key),
            user_id: Some(user.id()),
            team_id,
            permissions,
            rate_limits: None,
            expires_at: None,
            is_active: true,
            last_used_at: None,
            usage_stats: UsageStats::default(),
        };
        state.storage.db().create_api_key(&api_key).await.unwrap();
        (user, raw_key)
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

    #[tokio::test]
    async fn list_files_fails_closed_when_metadata_cannot_be_authorized() {
        let tempdir = tempfile::tempdir().expect("temp dir should be created");
        let local_path = tempdir.path().join("files").to_string_lossy().into_owned();
        let state = build_files_state(local_path.clone()).await;
        let orphan_id = Uuid::new_v4().to_string();
        let orphan_path = std::path::Path::new(&local_path)
            .join(&orphan_id[..2])
            .join(&orphan_id);
        tokio::fs::create_dir_all(orphan_path.parent().expect("orphan parent"))
            .await
            .expect("orphan shard should be created");
        tokio::fs::write(&orphan_path, b"orphaned content")
            .await
            .expect("orphan content should be written");

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let response =
            test::call_service(&app, test::TestRequest::get().uri("/v1/files").to_request()).await;

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body: Value = test::read_body_json(response).await;
        assert_eq!(body["error"]["message"], "Internal server error");
        assert!(!body.to_string().contains(&orphan_id));
    }

    #[tokio::test]
    async fn gh1130_proofless_public_wrappers_stop_before_storage() {
        use litellm_rs::server::routes::ai::{
            create_file, delete_file, get_file, get_file_content, list_files,
        };

        let tempdir = tempfile::tempdir().unwrap();
        let local_path = tempdir.path().join("files").to_string_lossy().into_owned();
        let state = build_files_state_with_auth(local_path, true).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state.clone()))
                .route("/proofless/files", web::post().to(create_file))
                .route("/proofless/files", web::get().to(list_files))
                .route("/proofless/files/{file_id}", web::get().to(get_file))
                .route("/proofless/files/{file_id}", web::delete().to(delete_file))
                .route(
                    "/proofless/files/{file_id}/content",
                    web::get().to(get_file_content),
                ),
        )
        .await;
        let boundary = "gh1130-proofless-boundary";
        let upload = test::TestRequest::post()
            .uri("/proofless/files")
            .insert_header((
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(multipart_body(boundary, Some("batch"), b"{}\n"))
            .to_request();
        assert_eq!(
            test::call_service(&app, upload).await.status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );

        let missing = "00000000-0000-0000-0000-000000000000";
        for (method, uri) in [
            ("GET", "/proofless/files".to_string()),
            ("GET", format!("/proofless/files/{missing}")),
            ("GET", format!("/proofless/files/{missing}/content")),
            ("DELETE", format!("/proofless/files/{missing}")),
        ] {
            let request = test::TestRequest::default()
                .method(method.parse().unwrap())
                .uri(&uri)
                .to_request();
            assert_eq!(
                test::call_service(&app, request).await.status(),
                StatusCode::INTERNAL_SERVER_ERROR
            );
        }
        assert!(
            state
                .storage
                .files
                .list(None, None)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn gh1130_two_tenants_cannot_list_read_content_or_delete_each_others_file() {
        let tempdir = tempfile::tempdir().unwrap();
        let local_path = tempdir.path().join("files").to_string_lossy().into_owned();
        let state = build_files_state_with_auth(local_path, true).await;
        let team_a = Uuid::new_v4();
        let team_b = Uuid::new_v4();
        let (_, key_a) = seed_principal(
            &state,
            "tenant-a",
            UserRole::User,
            Some(team_a),
            vec!["files".to_string()],
        )
        .await;
        let (_, key_b) = seed_principal(
            &state,
            "tenant-b",
            UserRole::User,
            Some(team_b),
            vec!["files".to_string()],
        )
        .await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .wrap(AuthMiddleware)
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let boundary = "gh1130-tenant-boundary";
        let upload = test::TestRequest::post()
            .uri("/v1/files")
            .insert_header(("x-api-key", key_a.as_str()))
            .insert_header((
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(multipart_body(
                boundary,
                Some("batch"),
                b"{\"tenant\":\"a\"}",
            ))
            .to_request();
        let uploaded = test::call_service(&app, upload).await;
        assert_eq!(uploaded.status(), StatusCode::OK);
        let uploaded: Value = test::read_body_json(uploaded).await;
        assert!(uploaded.get("owner").is_none());
        let file_id = uploaded["id"].as_str().unwrap().to_string();

        let list_b = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/v1/files")
                .insert_header(("x-api-key", key_b.as_str()))
                .to_request(),
        )
        .await;
        assert_eq!(list_b.status(), StatusCode::OK);
        let list_b: Value = test::read_body_json(list_b).await;
        assert!(list_b["data"].as_array().unwrap().is_empty());

        let mut foreign_body = None;
        for (method, suffix) in [("GET", ""), ("GET", "/content"), ("DELETE", "")] {
            let request = test::TestRequest::default()
                .method(method.parse().unwrap())
                .uri(&format!("/v1/files/{file_id}{suffix}"))
                .insert_header(("x-api-key", key_b.as_str()))
                .to_request();
            let response = test::call_service(&app, request).await;
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            let body: Value = test::read_body_json(response).await;
            if let Some(expected) = foreign_body.as_ref() {
                assert_eq!(&body, expected);
            } else {
                foreign_body = Some(body);
            }
        }

        let missing = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/v1/files/00000000-0000-0000-0000-000000000000")
                .insert_header(("x-api-key", key_b))
                .to_request(),
        )
        .await;
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        let missing: Value = test::read_body_json(missing).await;
        assert_eq!(Some(&missing), foreign_body.as_ref());

        let owner_get = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/v1/files/{file_id}"))
                .insert_header(("x-api-key", key_a))
                .to_request(),
        )
        .await;
        assert_eq!(owner_get.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn gh1130_admin_capability_not_admin_owner_controls_legacy_visibility() {
        let tempdir = tempfile::tempdir().unwrap();
        let local_path = tempdir.path().join("files").to_string_lossy().into_owned();
        let state = build_files_state_with_auth(local_path, true).await;
        let legacy_id = state
            .storage
            .files
            .store_with_purpose("legacy.jsonl", b"{}\n", Some("batch"))
            .await
            .unwrap();
        let (_, restricted_admin_owner_key) = seed_principal(
            &state,
            "restricted-admin-owner",
            UserRole::Admin,
            None,
            vec!["files".to_string()],
        )
        .await;
        let (_, direct_admin_key) = seed_principal(
            &state,
            "direct-admin-key",
            UserRole::User,
            None,
            vec!["*".to_string()],
        )
        .await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .wrap(AuthMiddleware)
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let restricted = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/v1/files")
                .insert_header(("x-api-key", restricted_admin_owner_key))
                .to_request(),
        )
        .await;
        let restricted: Value = test::read_body_json(restricted).await;
        assert!(restricted["data"].as_array().unwrap().is_empty());

        let direct = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/v1/files")
                .insert_header(("x-api-key", direct_admin_key))
                .to_request(),
        )
        .await;
        let direct: Value = test::read_body_json(direct).await;
        assert!(
            direct["data"]
                .as_array()
                .unwrap()
                .iter()
                .any(|file| file["id"] == legacy_id)
        );
    }
}
