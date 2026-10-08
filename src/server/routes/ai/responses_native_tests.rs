use super::*;
use serde_json::json;

#[test]
fn native_usage_checks_totals_and_details_without_double_counting_reasoning() {
    let valid = json!({"usage":{"input_tokens":12,"output_tokens":3,"total_tokens":15,"input_tokens_details":{"cached_tokens":4},"output_tokens_details":{"reasoning_tokens":2}}});
    let usage = response_usage(&valid).unwrap();
    assert_eq!(usage.total_tokens, 15);
    assert_eq!(usage.completion_tokens, 3);
    assert_eq!(usage.prompt_tokens_details.unwrap().cached_tokens, Some(4));
    assert_eq!(
        usage.completion_tokens_details.unwrap().reasoning_tokens,
        Some(2)
    );
    let zero = json!({"usage":{"input_tokens":0,"output_tokens":0,"total_tokens":0}});
    let usage = response_usage(&zero).expect("explicit zero usage is known");
    assert_eq!(
        (
            usage.prompt_tokens,
            usage.completion_tokens,
            usage.total_tokens
        ),
        (0, 0, 0)
    );
    let details = usage.prompt_tokens_details.unwrap();
    assert_eq!(details.cached_tokens, Some(0));
    assert_eq!(details.cache_creation_tokens, Some(0));
    assert_eq!(
        usage.completion_tokens_details.unwrap().reasoning_tokens,
        Some(0)
    );
    for field in ["input_tokens", "output_tokens", "total_tokens"] {
        let mut missing = zero.clone();
        missing["usage"].as_object_mut().unwrap().remove(field);
        assert!(
            response_usage(&missing).is_none(),
            "missing {field} is unknown"
        );
    }
    for invalid in [
        json!({"usage":{"input_tokens":0,"output_tokens":0,"total_tokens":1}}),
        json!({"usage":{"input_tokens":0,"output_tokens":0,"total_tokens":0,"input_tokens_details":{"cached_tokens":1}}}),
        json!({"usage":{"input_tokens":0,"output_tokens":0,"total_tokens":0,"input_tokens_details":{"cache_write_tokens":1}}}),
        json!({"usage":{"input_tokens":0,"output_tokens":0,"total_tokens":0,"output_tokens_details":{"reasoning_tokens":1}}}),
    ] {
        assert!(response_usage(&invalid).is_none(), "{invalid}");
    }
    for (path, value) in [
        ("/usage/total_tokens", json!(14)),
        ("/usage/input_tokens", json!(-1)),
        ("/usage/input_tokens_details/cached_tokens", json!(13)),
        ("/usage/output_tokens_details/reasoning_tokens", json!(4)),
    ] {
        let mut invalid = valid.clone();
        *invalid.pointer_mut(path).unwrap() = value;
        assert!(response_usage(&invalid).is_none(), "{invalid}");
    }
}

#[test]
fn budget_projection_includes_image_overhead_and_native_fields() {
    let body = json!({"input":[{"role":"user","content":[{"type":"input_image","image_url":"https://example.com/image.png"}]}],"tools":[{"type":"web_search"}],"max_output_tokens":10});
    let projected = budget_request(&body, "gpt-4o-mini");
    let Some(MessageContent::Parts(parts)) = &projected.messages[0].content else {
        panic!("missing parts");
    };
    assert!(matches!(&parts[1], ContentPart::ImageUrl { .. }));
    assert_eq!(projected.max_tokens, Some(10));
}
#[test]
fn budget_projection_does_not_count_image_data_as_text() {
    let image_data = format!("data:image/png;base64,{}", "A".repeat(100_000));
    let body = json!({"instructions":"follow the schema", "input":[{"role":"user","content":[{"type":"input_text","text":"describe this"},{"type":"input_image","image_url":image_data,"detail":"low"}]}],"tools":[{"type":"function","name":"lookup","parameters":{"type":"object"}}]});
    let projected = budget_request(&body, "gpt-4o-mini");
    let Some(MessageContent::Parts(parts)) = &projected.messages[0].content else {
        panic!("missing parts")
    };
    let ContentPart::Text { text } = &parts[0] else {
        panic!("missing text")
    };
    assert!(text.len() < 1_000);
    for expected in ["follow the schema", "describe this", "lookup", "parameters"] {
        assert!(text.contains(expected));
    }
    let ContentPart::ImageUrl { image_url } = &parts[1] else {
        panic!("missing image")
    };
    assert_eq!(image_url.url, image_data);
    assert_eq!(image_url.detail.as_deref(), Some("low"));
    assert_eq!(
        body.pointer("/input/0/content/1/image_url").unwrap(),
        &json!(image_data)
    );
}
