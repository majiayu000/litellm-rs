use super::*;
use crate::core::models::openai::responses_api::{
    ResponseFunctionCall, ResponseInputContent, ResponseInputItem, ResponseInputMessage,
    ResponseOutputMessage, ResponsesApiRequest,
};
use crate::core::types::codex::wire::{
    CodexCustomToolCall, CodexFunctionCallOutput, CodexToolOutput,
};
use actix_web::{HttpMessage, body::to_bytes, http::StatusCode, test as actix_test};
use serde_json::Value;

async fn database() -> Database {
    let config = crate::config::models::storage::DatabaseConfig {
        enabled: false,
        ..Default::default()
    };
    let database = Database::new(&config).await.unwrap();
    database.migrate().await.unwrap();
    database
}

async fn state() -> web::Data<AppState> {
    let mut config = crate::server::valid_test_config();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source = None;
    web::Data::new(
        crate::server::HttpServer::new(&config)
            .await
            .unwrap()
            .state()
            .clone(),
    )
}

fn owner(label: &str) -> ResponseOwner {
    ResponseOwner(format!("test:{label}"))
}

fn user_owner(user_id: &str) -> ResponseOwner {
    ResponseOwner(format!("user:{user_id}"))
}

fn req(input: &str) -> ResponsesApiRequest {
    ResponsesApiRequest {
        model: "gpt-4o".into(),
        input: ResponseInput::Text(input.into()),
        instructions: None,
        previous_response_id: None,
        store: None,
        tools: None,
        additional_tools: None,
        stream: None,
        background: None,
        max_output_tokens: None,
        temperature: None,
        top_p: None,
        user: None,
        reasoning: None,
        metadata: None,
        truncation: None,
    }
}

fn resp(id: &str, request: &ResponsesApiRequest, text: &str) -> ResponsesApiResponse {
    ResponsesApiResponse {
        id: id.into(),
        object: "response".into(),
        created_at: current_unix_ts(),
        status: "completed".into(),
        model: request.model.clone(),
        output: vec![ResponseOutputItem::Message(ResponseOutputMessage {
            id: format!("msg_test_{}", uuid_v4_hex()),
            role: "assistant".into(),
            status: "completed".into(),
            content: vec![ResponseOutputContent::OutputText {
                text: text.into(),
                annotations: None,
                logprobs: None,
            }],
        })],
        usage: None,
        error: None,
        previous_response_id: None,
        metadata: None,
    }
}

fn request_for_user(user_id: &str) -> HttpRequest {
    let req = actix_test::TestRequest::default().to_http_request();
    req.extensions_mut()
        .insert(crate::core::types::context::RequestContext::new().with_user_id(user_id));
    req
}

async fn read_json(response: HttpResponse) -> Value {
    let body = to_bytes(response.into_body())
        .await
        .expect("response body should be readable");
    serde_json::from_slice(&body).expect("response body should be json")
}

#[tokio::test]
async fn store_respects_owner_and_store_false() {
    let db = database().await;
    let owner_a = Some(owner("a"));
    let owner_b = Some(owner("b"));
    let request = req("hello");
    let response = resp(&format!("resp_test_{}", uuid_v4_hex()), &request, "world");
    store_response_if_requested(&db, &request, &response, owner_a.clone())
        .await
        .unwrap();

    assert!(
        get_owned_response(&db, &response.id, &owner_a)
            .await
            .is_ok()
    );
    assert!(
        get_owned_response(&db, &response.id, &owner_b)
            .await
            .is_err()
    );

    let mut unstored_request = req("hello");
    unstored_request.store = Some(false);
    let unstored = resp(
        &format!("resp_test_{}", uuid_v4_hex()),
        &unstored_request,
        "world",
    );
    store_response_if_requested(&db, &unstored_request, &unstored, owner_a)
        .await
        .unwrap();
    assert!(
        get_owned_response(&db, &unstored.id, &Some(owner("a")))
            .await
            .is_err()
    );
}

#[test]
fn storage_requires_authenticated_owner_unless_store_false() {
    let request = req("hello");
    assert!(validate_storage_owner(&request, &None).is_err());

    let mut unstored = req("hello");
    unstored.store = Some(false);
    assert!(validate_storage_owner(&unstored, &None).is_ok());
}

#[tokio::test]
async fn previous_context_prepends_prior_input_output_and_current_input() {
    let db = database().await;
    let owner = Some(owner("chain"));
    let previous_id = format!("resp_test_{}", uuid_v4_hex());
    let previous_req = req("first question");
    let previous_resp = resp(&previous_id, &previous_req, "first answer");
    store_response_if_requested(&db, &previous_req, &previous_resp, owner.clone())
        .await
        .unwrap();

    let mut follow_up = req("follow up");
    follow_up.previous_response_id = Some(previous_id.clone());
    let resolved = resolve_previous_response_context(&db, follow_up, &owner)
        .await
        .unwrap();
    let ResponseInput::Items(items) = resolved.input else {
        panic!("previous context should produce item input");
    };
    assert_eq!(items.len(), 3);

    db.delete_owned_response(&previous_id, &owner.as_ref().unwrap().0, current_unix_ts())
        .await
        .unwrap();
    let mut missing = req("follow up");
    missing.previous_response_id = Some(previous_id);
    assert!(
        resolve_previous_response_context(&db, missing, &owner)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn previous_context_preserves_codex_calls_for_correlated_outputs() {
    let db = database().await;
    let owner = Some(owner("codex-chain"));
    let previous_id = format!("resp_test_{}", uuid_v4_hex());
    let previous_req = req("run tools");
    let mut previous_resp = resp(&previous_id, &previous_req, "");
    previous_resp.output = vec![
        ResponseOutputItem::FunctionCall(ResponseFunctionCall {
            id: "fc_1".into(),
            name: "lookup".into(),
            arguments: "{}".into(),
            status: "completed".into(),
            call_id: Some("function-1".into()),
        }),
        ResponseOutputItem::CustomToolCall(CodexCustomToolCall {
            id: Some("ct_1".into()),
            call_id: "custom-1".into(),
            name: "shell".into(),
            namespace: None,
            input: "pwd".into(),
            status: Some("completed".into()),
            internal_chat_message_metadata_passthrough: None,
        }),
    ];
    store_response_if_requested(&db, &previous_req, &previous_resp, owner.clone())
        .await
        .unwrap();

    let mut follow_up = req("ignored");
    follow_up.previous_response_id = Some(previous_id.clone());
    follow_up.input = ResponseInput::Items(vec![
        ResponseInputItem::FunctionCallOutput(CodexFunctionCallOutput {
            id: None,
            call_id: "function-1".into(),
            output: CodexToolOutput::Text("found".into()),
            internal_chat_message_metadata_passthrough: None,
        }),
        ResponseInputItem::CustomToolCallOutput(
            crate::core::types::codex::wire::CodexCustomToolCallOutput {
                id: None,
                call_id: "custom-1".into(),
                name: Some("shell".into()),
                output: CodexToolOutput::Text("/tmp".into()),
                internal_chat_message_metadata_passthrough: None,
            },
        ),
    ]);
    let resolved = resolve_previous_response_context(&db, follow_up, &owner)
        .await
        .unwrap();
    assert!(crate::core::types::codex::domain::CodexTurn::try_from(&resolved).is_ok());
    let ResponseInput::Items(items) = resolved.input else {
        panic!("item input")
    };
    assert!(matches!(items[1], ResponseInputItem::FunctionCall(_)));
    assert!(matches!(items[2], ResponseInputItem::CustomToolCall(_)));
    assert!(matches!(items[3], ResponseInputItem::FunctionCallOutput(_)));
    assert!(matches!(
        items[4],
        ResponseInputItem::CustomToolCallOutput(_)
    ));
}

#[tokio::test]
async fn background_cancel_is_owner_scoped() {
    let db = database().await;
    let background_owner = owner("background");
    let other_owner = Some(owner("other"));
    let request = req("run later");
    let queued = queued_background_response(&request);
    let id = queued.id.clone();
    let record = response_record(&request, &queued, &background_owner, true).unwrap();
    db.insert_response(record.clone(), current_unix_ts())
        .await
        .unwrap();
    assert!(
        cancel_stored_background_response(&db, &id, &other_owner)
            .await
            .is_err()
    );
    assert_eq!(
        cancel_stored_background_response(&db, &id, &Some(background_owner.clone()))
            .await
            .unwrap()
            .status,
        "cancelled"
    );
    assert!(
        !finish_background_response(&db, &record, &resp(&id, &request, "done"))
            .await
            .unwrap()
    );
    assert_eq!(
        get_owned_response(&db, &id, &Some(background_owner))
            .await
            .unwrap()
            .response
            .status,
        "cancelled"
    );
}

#[actix_web::test]
async fn lifecycle_handlers_enforce_owner_delete_and_input_items_shape() {
    let state = state().await;
    let db = &state.storage.database;
    let route_owner = Some(user_owner("route-owner"));
    let other = request_for_user("other-owner");
    let request = req("hello");
    let response = resp(&format!("resp_test_{}", uuid_v4_hex()), &request, "world");
    let id = response.id.clone();
    store_response_if_requested(db, &request, &response, route_owner)
        .await
        .unwrap();

    let cross_owner = get_response(state.clone(), other, web::Path::from(id.clone()))
        .await
        .unwrap();
    assert_eq!(cross_owner.status(), StatusCode::NOT_FOUND);

    let list = list_response_input_items(
        state.clone(),
        request_for_user("route-owner"),
        web::Path::from(id.clone()),
        web::Query(InputItemsQuery {
            after: None,
            include: None,
            limit: Some(1),
            order: Some("asc".to_string()),
        }),
    )
    .await
    .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let list_body = read_json(list).await;
    assert_eq!(list_body["object"], "list");
    assert_eq!(list_body["data"].as_array().unwrap().len(), 1);
    assert!(list_body["first_id"].as_str().unwrap().starts_with("item_"));
    assert_eq!(list_body["first_id"], list_body["last_id"]);
    assert_eq!(list_body["first_id"], list_body["data"][0]["id"]);
    assert_eq!(list_body["data"][0]["type"], "message");
    assert_eq!(list_body["has_more"], false);

    let deleted = delete_response(
        state.clone(),
        request_for_user("route-owner"),
        web::Path::from(id.clone()),
    )
    .await
    .unwrap();
    assert_eq!(deleted.status(), StatusCode::OK);
    let deleted_body = read_json(deleted).await;
    assert_eq!(deleted_body["object"], "response.deleted");
    assert_eq!(deleted_body["deleted"], true);
    assert!(
        get_owned_response(db, &id, &Some(user_owner("route-owner")))
            .await
            .is_err()
    );
}

#[test]
fn input_items_page_defaults_desc_and_rejects_unsupported_include() {
    let input = ResponseInput::Items(vec![
        ResponseInputItem::Message(ResponseInputMessage {
            id: Some("msg_1:1".to_string()),
            phase: None,
            internal_chat_message_metadata_passthrough: None,
            role: "user".to_string(),
            content: ResponseInputContent::Text("collision".to_string()),
        }),
        ResponseInputItem::Message(ResponseInputMessage {
            id: Some("msg_1".to_string()),
            phase: None,
            internal_chat_message_metadata_passthrough: None,
            role: "user".to_string(),
            content: ResponseInputContent::Text("first".to_string()),
        }),
        ResponseInputItem::Message(ResponseInputMessage {
            id: Some("msg_1".to_string()),
            phase: None,
            internal_chat_message_metadata_passthrough: None,
            role: "user".to_string(),
            content: ResponseInputContent::Text("second".to_string()),
        }),
    ]);

    let first_page = input_items_page(
        &input,
        &InputItemsQuery {
            after: None,
            include: None,
            limit: Some(1),
            order: None,
        },
    )
    .unwrap();
    assert_eq!(first_page.data.len(), 1);
    assert_eq!(first_page.first_id.as_deref(), Some("msg_1"));
    assert_eq!(first_page.last_id.as_deref(), Some("msg_1"));
    assert_eq!(
        serde_json::to_string(&first_page.data[0])
            .unwrap()
            .matches("\"id\":")
            .count(),
        1
    );
    assert_eq!(
        serde_json::to_value(&first_page.data[0].item).unwrap(),
        serde_json::to_value(&input_items_from_response_input(&input)[2]).unwrap()
    );
    assert!(first_page.has_more);

    let second_page = input_items_page(
        &input,
        &InputItemsQuery {
            after: first_page.last_id,
            include: None,
            limit: Some(2),
            order: None,
        },
    )
    .unwrap();
    assert_eq!(second_page.data.len(), 2);
    assert_ne!(second_page.first_id, second_page.last_id);
    assert_eq!(
        serde_json::to_value(&second_page.data[0].item).unwrap(),
        serde_json::to_value(&input_items_from_response_input(&input)[1]).unwrap()
    );
    assert!(!second_page.has_more);

    let include_error = input_items_page(
        &input,
        &InputItemsQuery {
            after: None,
            include: Some(vec!["file_search_call.results".to_string()]),
            limit: None,
            order: None,
        },
    )
    .unwrap_err();
    assert!(matches!(include_error, GatewayError::Validation(_)));
}

#[tokio::test]
async fn reads_enforce_expiry_without_insert_and_fail_abandoned_workers() {
    let db = database().await;
    let owner = owner("worker");
    let request = req("old");
    let queued = queued_background_response(&request);
    let mut record = response_record(&request, &queued, &owner, true).unwrap();
    record.lease_until = Some(current_unix_ts() - 1);
    db.insert_response(record.clone(), current_unix_ts())
        .await
        .unwrap();
    let stored = get_owned_response(&db, &record.id, &Some(owner.clone()))
        .await
        .unwrap();
    assert_eq!(stored.response.status, "failed");
    assert!(
        stored
            .response
            .error
            .unwrap()
            .message
            .contains("worker stopped")
    );
    assert!(
        !db.renew_response_lease(&record.id, &record.owner, current_unix_ts())
            .await
            .unwrap()
    );
    assert!(
        !finish_background_response(&db, &record, &resp(&record.id, &request, "too late"))
            .await
            .unwrap()
    );

    record.id = "expired".into();
    record.expires_at = current_unix_ts();
    db.insert_response(record.clone(), current_unix_ts())
        .await
        .unwrap();
    assert!(
        get_owned_response(&db, &record.id, &Some(owner))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn cancellation_and_deletion_stop_leases_and_sync_cancel_conflicts() {
    let db = database().await;
    let owner = owner("worker");
    let request = req("run later");
    let queued = queued_background_response(&request);
    let record = response_record(&request, &queued, &owner, true).unwrap();
    db.insert_response(record.clone(), current_unix_ts())
        .await
        .unwrap();
    assert!(
        db.renew_response_lease(&record.id, &record.owner, current_unix_ts())
            .await
            .unwrap()
    );
    cancel_stored_background_response(&db, &record.id, &Some(owner.clone()))
        .await
        .unwrap();
    assert!(
        !db.renew_response_lease(&record.id, &record.owner, current_unix_ts())
            .await
            .unwrap()
    );
    let queued = queued_background_response(&request);
    let record = response_record(&request, &queued, &owner, true).unwrap();
    db.insert_response(record.clone(), current_unix_ts())
        .await
        .unwrap();
    assert!(
        db.delete_owned_response(&record.id, &record.owner, current_unix_ts())
            .await
            .unwrap()
    );
    assert!(
        !db.renew_response_lease(&record.id, &record.owner, current_unix_ts())
            .await
            .unwrap()
    );

    let response = resp("sync-response", &request, "world");
    store_response_if_requested(&db, &request, &response, Some(owner.clone()))
        .await
        .unwrap();
    let error = cancel_stored_background_response(&db, &response.id, &Some(owner))
        .await
        .unwrap_err();
    assert!(matches!(error, GatewayError::Conflict(_)));
}

#[cfg(feature = "sqlite")]
#[actix_web::test]
async fn separate_gateway_states_share_records_and_restart_preserves_them() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = crate::server::valid_test_config();
    config.gateway.storage.database.enabled = true;
    config.gateway.storage.database.auto_migrate = true;
    config.gateway.storage.database.url = format!(
        "sqlite://{}?mode=rwc",
        dir.path().join("responses.db").display()
    );
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source = None;
    let first = crate::server::HttpServer::new(&config).await.unwrap();
    let second = crate::server::HttpServer::new(&config).await.unwrap();
    let request = req("persist this input");
    let response = resp("persisted-response", &request, "persist this output");
    store_response_if_requested(
        &first.state().storage.database,
        &request,
        &response,
        Some(user_owner("alice")),
    )
    .await
    .unwrap();
    let data = web::Data::new(second.state().clone());
    assert_eq!(
        get_response(
            data.clone(),
            request_for_user("bob"),
            web::Path::from(response.id.clone())
        )
        .await
        .unwrap()
        .status(),
        StatusCode::NOT_FOUND
    );
    let retrieved = get_response(
        data,
        request_for_user("alice"),
        web::Path::from(response.id.clone()),
    )
    .await
    .unwrap();
    assert_eq!(
        read_json(retrieved).await["output"][0]["content"][0]["text"],
        "persist this output"
    );
    drop(first);
    drop(second);
    let restarted = crate::server::HttpServer::new(&config).await.unwrap();
    let data = web::Data::new(restarted.state().clone());
    let mut follow_up = req("second input");
    follow_up.previous_response_id = Some(response.id.clone());
    let resolved = resolve_previous_response_context(
        &data.storage.database,
        follow_up,
        &Some(user_owner("alice")),
    )
    .await
    .unwrap();
    assert!(matches!(resolved.input, ResponseInput::Items(items) if items.len() == 3));
    assert_eq!(
        delete_response(
            data.clone(),
            request_for_user("alice"),
            web::Path::from(response.id.clone())
        )
        .await
        .unwrap()
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        get_response(
            data,
            request_for_user("alice"),
            web::Path::from(response.id)
        )
        .await
        .unwrap()
        .status(),
        StatusCode::NOT_FOUND
    );
}
