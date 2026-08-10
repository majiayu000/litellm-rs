use crate::core::models::openai::responses_api::{
    ResponseInputItem, ResponseTool, ResponsesApiRequest,
};
use crate::core::types::codex::domain::{
    CodexCallKind, CodexCallState, CodexTurn, CodexTurnError, CodexTurnItem,
};
use crate::core::types::codex::wire::CODEX_PROTOCOL_BASELINE;
use actix_web::{body::to_bytes, http::StatusCode, test as actix_test, web};
use serde_json::{Value, json};
fn codex_request(value: Value) -> ResponsesApiRequest {
    serde_json::from_value(value).unwrap()
}
fn codex_turn_json(input: &str) -> Result<CodexTurn, CodexTurnError> {
    let input: Value = serde_json::from_str(input).unwrap();
    CodexTurn::try_from(&codex_request(json!({"model":"m","input":input})))
}
fn tier_two_items() -> [Value; 10] {
    serde_json::from_str(r#"[{"type":"additional_tools","role":"developer","tools":[]},{"type":"local_shell_call","call_id":"c1","status":"completed","action":{}},{"type":"mcp_tool_call_output","call_id":"c1","output":{"content":[]}},{"type":"tool_search_call","call_id":"c1","status":"completed","execution":"client","arguments":{}},{"type":"tool_search_output","call_id":"c1","status":"completed","execution":"client","tools":[]},{"type":"web_search_call","id":"i1","status":"completed"},{"type":"image_generation_call","id":"i1","status":"completed","result":"data"},{"type":"compaction","id":"i1","encrypted_content":"opaque"},{"type":"compaction_trigger"},{"type":"context_compaction","id":"i1","encrypted_content":"opaque"}]"#).unwrap()
}
#[test]
fn codex_wire_round_trips_every_tier_one_field() {
    let input: Value = serde_json::from_str(r#"[{"type":"message","id":"msg_1","phase":"commentary","role":"user","content":[{"type":"input_text","text":"run"},{"type":"input_audio","audio_url":"audio"}],"internal_chat_message_metadata_passthrough":{"turn_id":"turn_1"}},{"type":"function_call","id":"fc_1","call_id":"c1","name":"lookup","namespace":"demo","arguments":"{}","status":"completed","internal_chat_message_metadata_passthrough":{"turn_id":"turn_1"}},{"type":"function_call_output","id":"out_1","call_id":"c1","output":"ok","internal_chat_message_metadata_passthrough":{"turn_id":"turn_1"}},{"type":"custom_tool_call","id":"ct_1","call_id":"c2","name":"shell","namespace":"tools","input":"pwd","status":"completed","internal_chat_message_metadata_passthrough":{"turn_id":"turn_1"}},{"type":"custom_tool_call_output","id":"out_2","call_id":"c2","name":"shell","output":[{"type":"input_text","text":"/tmp"},{"type":"input_image","image_url":"image","detail":"high"},{"type":"input_audio","audio_url":"audio"},{"type":"encrypted_content","encrypted_content":"opaque"}],"internal_chat_message_metadata_passthrough":{"turn_id":"turn_1"}}]"#).unwrap();
    assert_eq!(input.as_array().unwrap().len(), 5, "fixture count drifted");
    let encoded = serde_json::to_value(codex_request(json!({"model":"m","input":input}))).unwrap();
    assert_eq!(encoded["input"], input);
    assert_eq!(
        CODEX_PROTOCOL_BASELINE,
        "6e5a2d6b8d148a5554fdceb6f399ca45bd1c78d9"
    );
}
#[test]
fn codex_wire_handles_missing_null_and_empty_optional_fields() {
    for item in [
        json!({"type":"message","role":"user","content":""}),
        json!({"type":"message","id":null,"phase":null,"role":"user","content":"x"}),
        json!({"type":"custom_tool_call","id":"","call_id":"","name":"","namespace":"","input":""}),
    ] {
        let encoded =
            serde_json::to_value(serde_json::from_value::<ResponseInputItem>(item).unwrap())
                .unwrap();
        assert!(encoded["id"].is_null() || encoded["id"] == "");
    }
}
#[test]
fn codex_wire_accepts_flat_and_legacy_function_tools() {
    for value in [
        json!({"type":"function","name":"flat","parameters":{"type":"object"},"strict":true,"defer_loading":false}),
        json!({"type":"function","function":{"name":"nested"}}),
    ] {
        let expected = value.clone();
        let tool: ResponseTool = serde_json::from_value(value).unwrap();
        assert!(matches!(
            tool,
            ResponseTool::Function(_) | ResponseTool::CodexFunction(_)
        ));
        assert_eq!(serde_json::to_value(tool).unwrap(), expected);
    }
}
#[test]
fn codex_wire_distinguishes_tier_two_and_redacts_unknown_payload() {
    for value in tier_two_items() {
        assert!(matches!(
            serde_json::from_value::<ResponseInputItem>(value).unwrap(),
            ResponseInputItem::Unsupported(_)
        ));
    }
    let known: ResponseInputItem = serde_json::from_value(json!({"type":"local_shell_call","id":"i1","call_id":"c1","status":"completed","action":{"secret":"drop"}})).unwrap();
    assert_eq!(
        serde_json::to_value(known).unwrap(),
        json!({"type":"local_shell_call","id":"i1","call_id":"c1","status":"completed"})
    );
    let unknown: ResponseInputItem = serde_json::from_value(json!({"type":"future_item","id":"i2","namespace":"demo","secret":"drop","payload":{"token":"drop"}})).unwrap();
    assert!(matches!(unknown, ResponseInputItem::Unknown(_)));
    assert_eq!(
        serde_json::to_value(unknown).unwrap(),
        json!({"type":"future_item","id":"i2","namespace":"demo"})
    );
}
#[actix_web::test]
async fn codex_wire_http_rejects_before_provider_dispatch() {
    let mut config = crate::config::Config::default();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    let server = crate::server::HttpServer::new(&config).await.unwrap();
    let state = web::Data::new(server.state().clone());
    super::PROVIDER_DISPATCH_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);
    let mut fixtures = vec![
        json!({"model":"m","input":[{"type":"function_call","call_id":"c","name":"f","arguments":"{}"}]}),
        json!({"model":"m","input":[{"type":"future\nsecret=abcdefghijklmnop","secret":"drop"}]}),
        json!({"model":"m","input":"x","additional_tools":[{"type":"function","name":"f"}]}),
        json!({"model":"m","input":"x","additional_tools":[]}),
        json!({"model":"m","input":[{"type":"message","role":"user","content":[{"type":"input_audio","audio_url":"audio"}]}]}),
    ];
    fixtures.extend(tier_two_items().map(|item| json!({"model":"m","input":[item]})));
    for tool in [
        json!({"type":"custom","name":"shell","description":"d","format":{}}),
        json!({"type":"function","name":"f"}),
        json!({"type":"function","name":"f","defer_loading":true}),
        json!({"type":"function","name":"f","strict":true}),
        json!({"type":"namespace"}),
        json!({"type":"tool_search"}),
        json!({"type":"image_generation"}),
        json!({"type":"web_search"}),
        json!({"type":"file_search"}),
        json!({"type":"code_interpreter"}),
        json!({"type":"computer_use_preview","display_width":1,"display_height":1,"environment":"browser"}),
        json!({"type":"mcp","server_label":"s","server_url":"https://example.com"}),
    ] {
        assert!(!matches!(
            serde_json::from_value::<ResponseTool>(tool.clone()).unwrap(),
            ResponseTool::Unknown(_)
        ));
        fixtures.push(json!({"model":"m","input":"x","tools":[tool]}));
    }
    for value in fixtures {
        let mut payload = codex_request(value);
        payload.store = Some(false);
        let req = actix_test::TestRequest::post()
            .insert_header(("x-codex-upstream-counter", "1"))
            .to_http_request();
        let response = super::create_response(state.clone(), req, web::Json(payload))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body()).await.unwrap()).unwrap();
        assert_eq!(body["error"]["type"], "invalid_request_error");
        assert_eq!(body["error"]["code"], "unsupported_codex_feature");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("provider=unselected")
        );
        assert!(!body.to_string().contains("abcdefghijklmnop"));
    }
    assert_eq!(
        super::PROVIDER_DISPATCH_COUNT.load(std::sync::atomic::Ordering::Relaxed),
        0
    );
}

#[test]
fn codex_turn_preserves_order_and_correlates_mixed_calls() {
    let request = codex_request(serde_json::from_str(r#"{"model":"m","stream":true,"input":[{"type":"message","role":"user","content":"run both"},{"type":"function_call","call_id":"function-1","name":"lookup","arguments":"{}"},{"type":"custom_tool_call","call_id":"custom-1","name":"shell","namespace":"tools","input":"pwd"},{"type":"custom_tool_call_output","call_id":"custom-1","name":"shell","output":"/tmp"},{"type":"function_call_output","call_id":"function-1","output":"ok"}],"tools":[{"type":"function","name":"lookup","description":"lookup","parameters":{"type":"object"},"strict":false},{"type":"custom","name":"shell","description":"shell","format":{}}]}"#).unwrap());

    let turn = CodexTurn::try_from(&request).unwrap();

    assert_eq!(turn.protocol_version, CODEX_PROTOCOL_BASELINE);
    assert_eq!(turn.tools.len(), 2);
    assert_eq!(turn.store, None);
    assert!(!turn.background);
    assert!(turn.requirements.streaming);
    assert!(turn.requirements.function_tools);
    assert!(turn.requirements.custom_tools);
    assert!(turn.requirements.call_outputs);
    assert!(matches!(
        turn.items[0],
        CodexTurnItem::Item(ResponseInputItem::Message(_))
    ));
    assert!(matches!(
        turn.items[1],
        CodexTurnItem::Item(ResponseInputItem::FunctionCall(_))
    ));
    assert!(matches!(
        turn.items[2],
        CodexTurnItem::Item(ResponseInputItem::CustomToolCall(_))
    ));
    assert!(matches!(
        turn.items[3],
        CodexTurnItem::Item(ResponseInputItem::CustomToolCallOutput(_))
    ));
    assert!(matches!(
        turn.items[4],
        CodexTurnItem::Item(ResponseInputItem::FunctionCallOutput(_))
    ));

    let calls = &turn.ledger.calls;
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].call_id, "function-1");
    assert_eq!(calls[0].name, "lookup");
    assert_eq!(calls[0].kind, CodexCallKind::Function);
    assert_eq!(calls[0].state, CodexCallState::OutputReceived);
    assert_eq!(
        (calls[0].declaration_index, calls[0].output_index),
        (1, Some(4))
    );
    assert_eq!(calls[1].call_id, "custom-1");
    assert_eq!(calls[1].name, "shell");
    assert_eq!(calls[1].kind, CodexCallKind::Custom);
    assert_eq!(calls[1].namespace.as_deref(), Some("tools"));
    assert_eq!(calls[1].state, CodexCallState::OutputReceived);
    assert_eq!(
        (calls[1].declaration_index, calls[1].output_index),
        (2, Some(3))
    );
}

#[test]
fn codex_call_ledger_accepts_each_parallel_output_order() {
    for input in [
        r#"[{"type":"function_call","call_id":"function-1","name":"lookup","arguments":"{}"},{"type":"custom_tool_call","call_id":"custom-1","name":"shell","input":"pwd"},{"type":"function_call_output","call_id":"function-1","output":"ok"},{"type":"custom_tool_call_output","call_id":"custom-1","name":"shell","output":"ok"}]"#,
        r#"[{"type":"function_call","call_id":"function-1","name":"lookup","arguments":"{}"},{"type":"custom_tool_call","call_id":"custom-1","name":"shell","input":"pwd"},{"type":"custom_tool_call_output","call_id":"custom-1","name":"shell","output":"ok"},{"type":"function_call_output","call_id":"function-1","output":"ok"}]"#,
    ] {
        let turn = codex_turn_json(input).unwrap();
        assert_eq!(turn.ledger.calls[0].call_id, "function-1");
        assert_eq!(turn.ledger.calls[1].call_id, "custom-1");
        assert!(
            turn.ledger
                .calls
                .iter()
                .all(|call| call.state == CodexCallState::OutputReceived)
        );
    }
}

#[test]
fn codex_call_ledger_rejects_invalid_input_without_leaking_identity() {
    use CodexCallKind::{Custom, Function};
    let secret = "abcdefghijklmnop";
    let inputs = [
        r#"[{"type":"function_call","call_id":"abcdefghijklmnop","name":"lookup","arguments":"{}"},{"type":"function_call","call_id":"abcdefghijklmnop","name":"lookup","arguments":"{}"}]"#,
        r#"[{"type":"function_call_output","call_id":"c1","output":"ok"}]"#,
        r#"[{"type":"function_call","call_id":"c1","name":"lookup","arguments":"{}"},{"type":"function_call_output","call_id":"c1","output":"ok"},{"type":"function_call_output","call_id":"c1","output":"again"}]"#,
        r#"[{"type":"function_call","call_id":"c1","name":"lookup","arguments":"{}"},{"type":"custom_tool_call_output","call_id":"c1","name":"lookup","output":"ok"}]"#,
        r#"[{"type":"function_call","call_id":" ","name":"lookup","arguments":"{}"}]"#,
        r#"[{"type":"custom_tool_call","call_id":"c1","name":" ","input":"pwd"}]"#,
        r#"[{"type":"custom_tool_call","call_id":"c1","name":"shell","input":"pwd"},{"type":"custom_tool_call_output","call_id":"c1","name":" ","output":"ok"}]"#,
        r#"[{"type":"custom_tool_call","call_id":"c1","name":"shell","input":"pwd"},{"type":"custom_tool_call_output","call_id":"c1","name":"other","output":"ok"}]"#,
        r#"[{"type":"function_call","call_id":"c1","name":"lookup","arguments":"not-json"}]"#,
        r#"[{"type":"custom_tool_call","call_id":"c1","name":"shell","input":""}]"#,
        r#"[{"type":"function_call","call_id":"c1","name":"lookup","arguments":"{}"},{"type":"function_call_output","call_id":"c1","output":""}]"#,
        r#"[{"type":"message","role":"intruder","content":"x"}]"#,
        r#"[{"type":"future_item","id":"i1","secret":"drop"}]"#,
    ];
    let expected = [
        CodexTurnError::DuplicateCallId(1),
        CodexTurnError::UnknownCallId(0),
        CodexTurnError::DuplicateCallOutput(2),
        CodexTurnError::CallKindMismatch(Function, Custom, 1),
        CodexTurnError::EmptyCallId { item_index: 0 },
        CodexTurnError::InvalidCallName { item_index: 0 },
        CodexTurnError::InvalidCallName { item_index: 1 },
        CodexTurnError::InvalidCallName { item_index: 1 },
        CodexTurnError::InvalidFunctionArguments { item_index: 0 },
        CodexTurnError::EmptyCallPayload { item_index: 0 },
        CodexTurnError::EmptyCallPayload { item_index: 1 },
        CodexTurnError::InvalidMessageRole { item_index: 0 },
        CodexTurnError::UnsupportedFeature("future_item".into()),
    ];
    for (input, expected) in inputs.into_iter().zip(expected) {
        let actual = codex_turn_json(input).unwrap_err();
        assert_eq!(actual, expected);
        assert!(!format!("{actual:?} {actual}").contains(secret));
    }
}

#[test]
fn codex_turn_rejects_invalid_tool_definitions() {
    let requests = [
        r#"{"model":"m","input":"x","tools":[{"type":"function","name":" ","parameters":{}}]}"#,
        r#"{"model":"m","input":"x","tools":[{"type":"custom","name":" ","description":"d","format":{}}]}"#,
        r#"{"model":"m","input":"x","tools":[{"type":"function","name":"lookup","defer_loading":true}]}"#,
    ];
    let expected = [
        CodexTurnError::EmptyToolName { tool_index: 0 },
        CodexTurnError::EmptyToolName { tool_index: 0 },
        CodexTurnError::UnsupportedFeature("defer_loading".into()),
    ];
    for (request, expected) in requests.into_iter().zip(expected) {
        let request = codex_request(serde_json::from_str(request).unwrap());
        assert_eq!(CodexTurn::try_from(&request).unwrap_err(), expected);
    }
}
