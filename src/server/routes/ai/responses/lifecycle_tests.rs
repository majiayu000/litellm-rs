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

#[test]
fn store_respects_owner_and_store_false() {
    let owner_a = Some(owner("a"));
    let owner_b = Some(owner("b"));
    let request = req("hello");
    let response = resp(&format!("resp_test_{}", uuid_v4_hex()), &request, "world");
    store_response_if_requested(&request, &response, owner_a.clone());

    assert!(get_owned_response(&response.id, &owner_a).is_ok());
    assert!(get_owned_response(&response.id, &owner_b).is_err());
    RESPONSE_STORE.remove(&response.id);

    let mut unstored_request = req("hello");
    unstored_request.store = Some(false);
    let unstored = resp(
        &format!("resp_test_{}", uuid_v4_hex()),
        &unstored_request,
        "world",
    );
    store_response_if_requested(&unstored_request, &unstored, owner_a);
    assert!(get_owned_response(&unstored.id, &Some(owner("a"))).is_err());
}

#[test]
fn storage_requires_authenticated_owner_unless_store_false() {
    let request = req("hello");
    assert!(validate_storage_owner(&request, &None).is_err());

    let mut unstored = req("hello");
    unstored.store = Some(false);
    assert!(validate_storage_owner(&unstored, &None).is_ok());
}

#[test]
fn previous_context_prepends_prior_input_output_and_current_input() {
    let owner = Some(owner("chain"));
    let previous_id = format!("resp_test_{}", uuid_v4_hex());
    let previous_req = req("first question");
    let previous_resp = resp(&previous_id, &previous_req, "first answer");
    store_response_if_requested(&previous_req, &previous_resp, owner.clone());

    let mut follow_up = req("follow up");
    follow_up.previous_response_id = Some(previous_id.clone());
    let resolved = resolve_previous_response_context(follow_up, &owner).unwrap();
    let ResponseInput::Items(items) = resolved.input else {
        panic!("previous context should produce item input");
    };
    assert_eq!(items.len(), 3);

    RESPONSE_STORE.remove(&previous_id);
    let mut missing = req("follow up");
    missing.previous_response_id = Some(previous_id);
    assert!(resolve_previous_response_context(missing, &owner).is_err());
}

#[test]
fn previous_context_realigns_item_sidecars_with_prepended_history() {
    let owner = Some(owner("sidecar-chain"));
    let previous_id = format!("resp_test_{}", uuid_v4_hex());
    let previous_req = req("first question");
    let previous_resp = resp(&previous_id, &previous_req, "first answer");
    store_response_if_requested(&previous_req, &previous_resp, owner.clone());

    let mut follow_up = req("unused");
    follow_up.previous_response_id = Some(previous_id.clone());
    follow_up.input =
        ResponseInput::Items(vec![ResponseInputItem::Message(ResponseInputMessage {
            id: None,
            phase: None,
            internal_chat_message_metadata_passthrough: None,
            role: "user".to_string(),
            content: ResponseInputContent::Text("follow up".to_string()),
        })]);
    let (resolved, extensions) =
        resolve_previous_response_context_with_extensions(follow_up, vec![None], &owner).unwrap();
    let ResponseInput::Items(items) = resolved.input else {
        panic!("previous context should produce item input");
    };

    assert_eq!(items.len(), 3);
    assert_eq!(extensions.len(), items.len());
    assert!(extensions.iter().all(Option::is_none));
    RESPONSE_STORE.remove(&previous_id);
}

#[actix_web::test]
async fn previous_context_realigns_empty_sidecars_before_provider_dispatch() {
    let _guard = super::super::CODEX_DISPATCH_TEST_LOCK.lock().await;
    let user_id = "previous-context-sidecar-owner";
    let previous_id = format!("resp_test_{}", uuid_v4_hex());
    let previous_request = req("first question");
    let previous_response = resp(&previous_id, &previous_request, "first answer");
    store_response_if_requested(
        &previous_request,
        &previous_response,
        Some(user_owner(user_id)),
    );

    let mut follow_up = req("follow up");
    follow_up.model = "m".to_string();
    follow_up.previous_response_id = Some(previous_id.clone());
    follow_up.store = Some(false);
    let payload =
        crate::core::models::openai::continuation::ResponsesApiRequestWithExtensions::from_parts(
            follow_up,
            vec![],
        )
        .unwrap();

    let mut config = crate::server::valid_test_config();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    let server = crate::server::HttpServer::new(&config).await.unwrap();
    let state = web::Data::new(server.state().clone());
    super::super::PROVIDER_DISPATCH_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);
    let request = actix_test::TestRequest::post()
        .insert_header(("x-codex-upstream-counter", "1"))
        .to_http_request();
    request
        .extensions_mut()
        .insert(crate::core::types::context::RequestContext::new().with_user_id(user_id));

    let _response = super::super::create_response(state, request, web::Json(payload))
        .await
        .unwrap();
    assert_eq!(
        super::super::PROVIDER_DISPATCH_COUNT.load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    RESPONSE_STORE.remove(&previous_id);
}

#[test]
fn previous_context_preserves_codex_calls_for_correlated_outputs() {
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
    store_response_if_requested(&previous_req, &previous_resp, owner.clone());

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
    let resolved = resolve_previous_response_context(follow_up, &owner).unwrap();
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
    RESPONSE_STORE.remove(&previous_id);
}

#[test]
fn background_cancel_is_owner_scoped() {
    let background_owner = owner("background");
    let other_owner = Some(owner("other"));
    let request = req("run later");
    let queued = queued_background_response(&request);
    let id = queued.id.clone();
    insert_stored_response(
        id.clone(),
        StoredResponse {
            response: queued,
            input: request.input.clone(),
            background: true,
            owner: background_owner.clone(),
        },
    );

    assert!(cancel_stored_background_response(&id, &other_owner).is_err());
    assert_eq!(
        cancel_stored_background_response(&id, &Some(background_owner))
            .unwrap()
            .status,
        "cancelled"
    );
    finish_background_response(&id, request.input.clone(), resp(&id, &request, "done"));
    assert_eq!(
        RESPONSE_STORE
            .get(&id)
            .map(|stored| stored.response.status.clone()),
        Some("cancelled".to_string())
    );
    remove_response_and_task(&id);
}

#[actix_web::test]
async fn lifecycle_handlers_enforce_owner_delete_and_input_items_shape() {
    let route_owner = Some(user_owner("route-owner"));
    let other = request_for_user("other-owner");
    let request = req("hello");
    let response = resp(&format!("resp_test_{}", uuid_v4_hex()), &request, "world");
    let id = response.id.clone();
    store_response_if_requested(&request, &response, route_owner);

    let cross_owner = get_response(other, web::Path::from(id.clone()))
        .await
        .unwrap();
    assert_eq!(cross_owner.status(), StatusCode::NOT_FOUND);

    let list = list_response_input_items(
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

    let deleted = delete_response(request_for_user("route-owner"), web::Path::from(id.clone()))
        .await
        .unwrap();
    assert_eq!(deleted.status(), StatusCode::OK);
    let deleted_body = read_json(deleted).await;
    assert_eq!(deleted_body["object"], "response");
    assert_eq!(deleted_body["deleted"], true);
    assert!(get_owned_response(&id, &Some(user_owner("route-owner"))).is_err());
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

#[actix_web::test]
async fn response_store_cleanup_removes_expired_entries_and_tasks() {
    let cleanup_owner = owner("cleanup");
    let mut request = req("old");
    let mut response = resp(&format!("resp_test_{}", uuid_v4_hex()), &request, "old");
    response.created_at = current_unix_ts() - RESPONSE_STORE_TTL_SECS - 1;
    let id = response.id.clone();
    request.input = ResponseInput::Text("old".to_string());
    insert_stored_response(
        id.clone(),
        StoredResponse {
            response,
            input: request.input.clone(),
            background: true,
            owner: cleanup_owner,
        },
    );
    let handle = tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    });
    BACKGROUND_TASKS.insert(id.clone(), handle);

    let fresh_request = req("fresh");
    let fresh_response = resp(
        &format!("resp_test_{}", uuid_v4_hex()),
        &fresh_request,
        "fresh",
    );
    insert_stored_response(
        fresh_response.id.clone(),
        StoredResponse {
            response: fresh_response.clone(),
            input: fresh_request.input.clone(),
            background: false,
            owner: owner("cleanup-fresh"),
        },
    );

    assert!(RESPONSE_STORE.get(&id).is_none());
    assert!(BACKGROUND_TASKS.get(&id).is_none());
    remove_response_and_task(&fresh_response.id);
}

#[actix_web::test]
async fn cancel_aborts_background_task_and_non_background_conflicts() {
    let background_owner = owner("abort-owner");
    let request = req("run later");
    let queued = queued_background_response(&request);
    let id = queued.id.clone();
    insert_stored_response(
        id.clone(),
        StoredResponse {
            response: queued,
            input: request.input.clone(),
            background: true,
            owner: background_owner.clone(),
        },
    );
    let handle = tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    });
    BACKGROUND_TASKS.insert(id.clone(), handle);

    let cancelled = cancel_stored_background_response(&id, &Some(background_owner)).unwrap();
    assert_eq!(cancelled.status, "cancelled");
    assert!(BACKGROUND_TASKS.get(&id).is_none());
    remove_response_and_task(&id);

    let owner = Some(owner("sync-owner"));
    let sync_request = req("hello");
    let sync_response = resp(
        &format!("resp_test_{}", uuid_v4_hex()),
        &sync_request,
        "world",
    );
    store_response_if_requested(&sync_request, &sync_response, owner.clone());
    let err = cancel_stored_background_response(&sync_response.id, &owner).unwrap_err();
    assert!(matches!(err, GatewayError::Conflict(_)));
    remove_response_and_task(&sync_response.id);
}
