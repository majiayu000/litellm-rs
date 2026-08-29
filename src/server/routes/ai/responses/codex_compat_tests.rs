use crate::core::models::openai::responses_api::{
    ResponseInputItem, ResponseOutputItem, ResponseTool, ResponsesApiRequest,
};
use crate::core::models::openai::{
    messages::{ChatMessage, MessageRole},
    responses::{ChatChoice, ChatCompletionResponse},
    tools::{FunctionCall, ToolCall},
};
use crate::core::types::anthropic_continuation::{
    AnthropicRedactedData, AnthropicSignature, AnthropicThinkingBlock, AnthropicThinkingContent,
    ChatMessageExtensions,
};
use crate::core::types::codex::domain::{CodexCallKind, CodexTurn, CodexTurnError, CodexTurnItem};
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

#[test]
fn responses_header_explicitly_opts_in_the_first_turn() {
    let valid = actix_test::TestRequest::post()
        .insert_header(("x-litellm-anthropic-continuation", "v1"))
        .to_http_request();
    assert!(super::super::chat::continuation_opt_in(&valid, false).unwrap());

    let invalid = actix_test::TestRequest::post()
        .insert_header(("x-litellm-anthropic-continuation", "future"))
        .to_http_request();
    assert!(super::super::chat::continuation_opt_in(&invalid, false).is_err());
}

#[test]
fn first_turn_response_preserves_signed_and_redacted_continuation() {
    let request = codex_request(json!({"model":"claude-opus-5","input":"run"}));
    let chat = ChatCompletionResponse {
        id: "chat_1".into(),
        object: "chat.completion".into(),
        created: 1,
        model: "claude-opus-5".into(),
        system_fingerprint: None,
        choices: vec![ChatChoice {
            index: 0,
            message: ChatMessage {
                role: MessageRole::Assistant,
                content: None,
                name: None,
                function_call: None,
                tool_calls: Some(vec![ToolCall {
                    id: "toolu_1".into(),
                    tool_type: "function".into(),
                    function: FunctionCall {
                        name: "lookup".into(),
                        arguments: "{}".into(),
                    },
                }]),
                tool_call_id: None,
                audio: None,
            },
            logprobs: None,
            finish_reason: Some("tool_calls".into()),
        }],
        usage: None,
    };
    let extension =
        ChatMessageExtensions::new().with_anthropic_thinking(AnthropicThinkingContent::new(vec![
            AnthropicThinkingBlock::Thinking {
                thinking: "plan".into(),
                signature: AnthropicSignature::try_from("opaque-signature").unwrap(),
            },
            AnthropicThinkingBlock::RedactedThinking {
                data: AnthropicRedactedData::try_from("opaque-redacted").unwrap(),
            },
        ]));

    let mut response = super::convert_to_responses_api(chat, &request);
    let extensions = crate::core::models::openai::continuation::attach_responses_choice_extensions(
        &mut response,
        vec![extension],
        super::empty_output_message(),
    )
    .expect("first turn continuation response");
    let response =
        crate::core::models::openai::continuation::ResponsesApiResponseWithExtensions::from_parts(
            response, extensions,
        )
        .expect("matching response extensions");
    let encoded = serde_json::to_value(response).unwrap();
    assert_eq!(encoded["output"][0]["type"], "message");
    assert_eq!(
        encoded["output"][0]["extensions"]["anthropic_thinking"][0]["signature"],
        "opaque-signature"
    );
    assert_eq!(
        encoded["output"][0]["extensions"]["anthropic_thinking"][1]["data"],
        "opaque-redacted"
    );
    assert_eq!(encoded["output"][1]["type"], "function_call");
}

#[actix_web::test]
async fn first_turn_opt_in_rejects_unsupported_responses_modes_before_dispatch() {
    let _guard = super::CODEX_DISPATCH_TEST_LOCK.lock().await;
    let mut config = crate::server::valid_test_config();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    let server = crate::server::HttpServer::new(&config).await.unwrap();
    let state = web::Data::new(server.state().clone());

    for (mode, explicit_store_false) in [
        (json!({}), false),
        (json!({"stream":true}), true),
        (json!({"background":true}), true),
        (json!({"store":true}), false),
        (json!({"previous_response_id":"resp_previous"}), true),
    ] {
        super::PROVIDER_DISPATCH_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);
        let mut value = json!({"model":"m","input":"run"});
        if explicit_store_false {
            value["store"] = json!(false);
        }
        value
            .as_object_mut()
            .unwrap()
            .extend(mode.as_object().unwrap().clone());
        let payload = crate::core::models::openai::continuation::ResponsesApiRequestWithExtensions::from_parts(
            codex_request(value),
            vec![],
        )
        .unwrap();
        let req = actix_test::TestRequest::post()
            .insert_header(("x-litellm-anthropic-continuation", "v1"))
            .insert_header(("x-codex-upstream-counter", "1"))
            .to_http_request();
        let response = super::create_response(state.clone(), req, web::Json(payload))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body()).await.unwrap()).unwrap();
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("continuation does not yet support")
        );
        assert_eq!(
            super::PROVIDER_DISPATCH_COUNT.load(std::sync::atomic::Ordering::Relaxed),
            0
        );
    }
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
    let _guard = super::CODEX_DISPATCH_TEST_LOCK.lock().await;
    let mut config = crate::server::valid_test_config();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    let server = crate::server::HttpServer::new(&config).await.unwrap();
    let state = web::Data::new(server.state().clone());
    super::PROVIDER_DISPATCH_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);
    let mut fixtures = vec![
        json!({"model":"m","input":[{"type":"future\nsecret=abcdefghijklmnop","secret":"drop"}]}),
        json!({"model":"m","input":"x","additional_tools":[{"type":"function","name":"f"}]}),
        json!({"model":"m","input":"x","additional_tools":[]}),
        json!({"model":"m","input":[{"type":"message","role":"user","content":[{"type":"input_audio","audio_url":"audio"}]}]}),
    ];
    fixtures.extend(tier_two_items().map(|item| json!({"model":"m","input":[item]})));
    for tool in [
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
        let extensions = match &payload.input {
            super::ResponseInput::Text(_) => vec![],
            super::ResponseInput::Items(items) => vec![None; items.len()],
        };
        let payload = crate::core::models::openai::continuation::ResponsesApiRequestWithExtensions::from_parts(payload, extensions).unwrap();
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

#[actix_web::test]
async fn codex_tier_one_reaches_provider_dispatch() {
    let _guard = super::CODEX_DISPATCH_TEST_LOCK.lock().await;
    let mut config = crate::server::valid_test_config();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    let server = crate::server::HttpServer::new(&config).await.unwrap();
    let state = web::Data::new(server.state().clone());
    super::PROVIDER_DISPATCH_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);
    let payload = codex_request(json!({
        "model":"m",
        "store":false,
        "input":[{"type":"message","role":"user","content":"run"}],
        "tools":[
            {"type":"function","name":"lookup","parameters":{"type":"object"}},
            {"type":"custom","name":"shell","description":"shell","format":{"type":"text"}}
        ]
    }));
    let req = actix_test::TestRequest::post()
        .insert_header(("x-codex-upstream-counter", "1"))
        .to_http_request();
    let extensions = match &payload.input {
        super::ResponseInput::Text(_) => vec![],
        super::ResponseInput::Items(items) => vec![None; items.len()],
    };
    let payload =
        crate::core::models::openai::continuation::ResponsesApiRequestWithExtensions::from_parts(
            payload, extensions,
        )
        .unwrap();
    let _ = super::create_response(state, req, web::Json(payload))
        .await
        .unwrap();
    assert_eq!(
        super::PROVIDER_DISPATCH_COUNT.load(std::sync::atomic::Ordering::Relaxed),
        1
    );
}

#[test]
fn codex_turn_preserves_order_and_correlates_mixed_calls() {
    let request = codex_request(serde_json::from_str(r#"{"model":"m","stream":true,"input":[{"type":"message","role":"user","content":"run both"},{"type":"function_call","call_id":"function-1","name":"lookup","arguments":"{}"},{"type":"custom_tool_call","call_id":"custom-1","name":"shell","namespace":"tools","input":"pwd"},{"type":"custom_tool_call_output","call_id":"custom-1","name":"shell","output":"/tmp"},{"type":"function_call_output","call_id":"function-1","output":"ok"}],"tools":[{"type":"function","name":"lookup","description":"lookup","parameters":{"type":"object"},"strict":false},{"type":"custom","name":"shell","description":"shell","format":{}}]}"#).unwrap());

    let turn = CodexTurn::try_from(&request).unwrap();

    assert_eq!(turn.tools.len(), 2);
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

    let chat = super::build_chat_request(&request).unwrap();
    let encoded = serde_json::to_value(chat).unwrap();
    assert_eq!(encoded["messages"][1]["tool_calls"][0]["id"], "function-1");
    assert_eq!(encoded["messages"][1]["tool_calls"][1]["id"], "custom-1");
    assert_eq!(encoded["messages"][2]["tool_call_id"], "custom-1");
    assert_eq!(encoded["messages"][3]["tool_call_id"], "function-1");
    assert_eq!(encoded["tools"][0]["function"]["name"], "lookup");
    assert_eq!(encoded["tools"][1]["function"]["name"], "shell");
    assert_eq!(
        encoded["tools"][1]["function"]["parameters"]["required"][0],
        "input"
    );
}

#[test]
fn codex_non_streaming_reconstructs_custom_tool_output() {
    let request = codex_request(json!({
        "model":"m",
        "input":"run",
        "tools":[{"type":"custom","name":"shell","description":"shell","format":{"type":"text"}}]
    }));
    let chat = ChatCompletionResponse {
        id: "chat_1".into(),
        object: "chat.completion".into(),
        created: 1,
        model: "m".into(),
        system_fingerprint: None,
        choices: vec![ChatChoice {
            index: 0,
            message: ChatMessage {
                role: MessageRole::Assistant,
                content: None,
                name: None,
                function_call: None,
                tool_calls: Some(vec![ToolCall {
                    id: "call_1".into(),
                    tool_type: "function".into(),
                    function: FunctionCall {
                        name: "shell".into(),
                        arguments: r#"{"input":"pwd"}"#.into(),
                    },
                }]),
                tool_call_id: None,
                audio: None,
            },
            logprobs: None,
            finish_reason: Some("tool_calls".into()),
        }],
        usage: None,
    };

    let response = super::convert_to_responses_api(chat, &request);
    let ResponseOutputItem::CustomToolCall(call) = &response.output[0] else {
        panic!("custom tool must remain custom in Responses output");
    };
    assert_eq!(call.call_id, "call_1");
    assert_eq!(call.name, "shell");
    assert_eq!(call.input, "pwd");
    assert_eq!(call.status.as_deref(), Some("completed"));
}

#[test]
fn portable_codex_turn_has_one_contract_for_supported_adapter_families() {
    for model in [
        "openai-compatible/test",
        "anthropic/claude-test",
        "gemini/gemini-test",
    ] {
        let request = codex_request(json!({
            "model":model,
            "input":[
                {"type":"message","role":"user","content":"run"},
                {"type":"custom_tool_call","call_id":"call_1","name":"shell","input":"pwd"},
                {"type":"custom_tool_call_output","call_id":"call_1","name":"shell","output":"/tmp"}
            ],
            "tools":[{"type":"custom","name":"shell","description":"shell","format":{"type":"text"}}]
        }));
        let chat = super::build_chat_request(&request).unwrap();
        assert_eq!(chat.model, model);
        assert_eq!(
            chat.messages[1].tool_calls.as_ref().unwrap()[0].id,
            "call_1"
        );
        assert_eq!(chat.messages[2].tool_call_id.as_deref(), Some("call_1"));
        assert_eq!(chat.tools.as_ref().unwrap()[0].function.name, "shell");
    }
}

#[test]
fn codex_call_ledger_accepts_each_parallel_output_order() {
    for input in [
        r#"[{"type":"function_call","call_id":"function-1","name":"lookup","arguments":"{}"},{"type":"custom_tool_call","call_id":"custom-1","name":"shell","input":"pwd"},{"type":"function_call_output","call_id":"function-1","output":"ok"},{"type":"custom_tool_call_output","call_id":"custom-1","name":"shell","output":"ok"}]"#,
        r#"[{"type":"function_call","call_id":"function-1","name":"lookup","arguments":"{}"},{"type":"custom_tool_call","call_id":"custom-1","name":"shell","input":"pwd"},{"type":"custom_tool_call_output","call_id":"custom-1","name":"shell","output":"ok"},{"type":"function_call_output","call_id":"function-1","output":"ok"}]"#,
    ] {
        let turn = codex_turn_json(input).unwrap();
        assert_eq!(turn.items.len(), 4);
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
        r#"{"model":"m","input":"x","tools":[{"type":"custom","name":"shell","description":"d","format":{"type":"grammar","syntax":"regex","definition":".*"}}]}"#,
    ];
    let expected = [
        CodexTurnError::EmptyToolName { tool_index: 0 },
        CodexTurnError::EmptyToolName { tool_index: 0 },
        CodexTurnError::UnsupportedFeature("defer_loading".into()),
        CodexTurnError::UnsupportedFeature("custom.format".into()),
    ];
    for (request, expected) in requests.into_iter().zip(expected) {
        let request = codex_request(serde_json::from_str(request).unwrap());
        assert_eq!(CodexTurn::try_from(&request).unwrap_err(), expected);
    }
}
