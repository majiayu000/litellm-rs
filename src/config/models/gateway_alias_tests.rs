use super::GatewayConfig;
use serde_json::{Map, Value};
use std::collections::HashMap;

fn gateway_from_alias_value(value: Option<Value>) -> GatewayConfig {
    let mut gateway = serde_json::to_value(GatewayConfig::default())
        .expect("default gateway should serialize for fixture construction");
    let object = gateway
        .as_object_mut()
        .expect("serialized gateway should be an object");
    match value {
        Some(value) => {
            object.insert("model_aliases".to_string(), value);
        }
        None => {
            object.remove("model_aliases");
        }
    }
    serde_json::from_value(gateway).expect("gateway alias fixture should deserialize")
}

fn alias_map(entries: &[(&str, &str)]) -> HashMap<String, String> {
    entries
        .iter()
        .map(|(alias, target)| ((*alias).to_string(), (*target).to_string()))
        .collect()
}

#[test]
fn model_aliases_default_and_serde_are_backward_compatible() {
    assert!(GatewayConfig::default().model_aliases.is_empty());
    assert!(gateway_from_alias_value(None).model_aliases.is_empty());
    assert!(
        gateway_from_alias_value(Some(Value::Object(Map::new())))
            .model_aliases
            .is_empty()
    );

    let configured =
        gateway_from_alias_value(Some(serde_json::json!({"public-chat": "provider-model"})));
    assert_eq!(
        configured
            .model_aliases
            .get("public-chat")
            .map(String::as_str),
        Some("provider-model")
    );
}

#[test]
fn model_alias_merge_is_key_wise_with_overlay_wins() {
    let base = GatewayConfig {
        model_aliases: alias_map(&[("stable", "model-v1"), ("base-only", "base-model")]),
        ..Default::default()
    };
    let overlay = GatewayConfig {
        model_aliases: alias_map(&[("stable", "model-v2"), ("overlay-only", "overlay-model")]),
        ..Default::default()
    };

    let merged = base.clone().merge(overlay);
    assert_eq!(
        merged.model_aliases,
        alias_map(&[
            ("stable", "model-v2"),
            ("base-only", "base-model"),
            ("overlay-only", "overlay-model"),
        ])
    );

    assert_eq!(
        base.clone()
            .merge(gateway_from_alias_value(None))
            .model_aliases,
        base.model_aliases
    );
    assert_eq!(
        base.clone()
            .merge(gateway_from_alias_value(Some(Value::Object(Map::new()))))
            .model_aliases,
        base.model_aliases
    );
}

#[test]
fn layered_yaml_alias_with_surrounding_whitespace_fails_validation() {
    let base = GatewayConfig {
        model_aliases: alias_map(&[("stable", "model-v1")]),
        ..Default::default()
    };
    let yaml_aliases: HashMap<String, String> =
        serde_yml::from_str("' public ': gpt-4o").expect("alias YAML should deserialize");
    let overlay = GatewayConfig {
        model_aliases: yaml_aliases,
        ..Default::default()
    };

    let merged = base.merge(overlay);
    assert!(merged.model_aliases.contains_key(" public "));
    let error = GatewayConfig::validate_model_alias_map(&merged.model_aliases)
        .expect_err("layered YAML aliases must fail closed on surrounding whitespace");
    assert!(
        error.contains("cannot contain leading or trailing whitespace"),
        "{error}"
    );
}

#[test]
fn gateway_unknown_fields_remain_rejected() {
    let mut gateway =
        serde_json::to_value(GatewayConfig::default()).expect("default gateway should serialize");
    gateway
        .as_object_mut()
        .expect("gateway should serialize as an object")
        .insert("unknown_alias_policy".to_string(), Value::Bool(true));

    assert!(serde_json::from_value::<GatewayConfig>(gateway).is_err());
}
