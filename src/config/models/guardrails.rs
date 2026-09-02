//! Gateway-specific defaults and deserialization for content guardrails.

use crate::core::guardrails::config::CustomRuleConfig;
use crate::core::guardrails::{
    GuardrailAction, GuardrailConfig, OpenAIModerationConfig, PIIConfig, PromptInjectionConfig,
};
use serde::Deserialize;

pub(super) fn default_gateway_guardrails() -> GuardrailConfig {
    GuardrailConfig::default()
        .enable()
        .with_prompt_injection(PromptInjectionConfig::new())
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct GatewayGuardrailsWire {
    enabled: Option<bool>,
    openai_moderation: Option<OpenAIModerationConfig>,
    pii: Option<PIIConfig>,
    prompt_injection: Option<PromptInjectionConfig>,
    custom_rules: Option<Vec<CustomRuleConfig>>,
    default_action: Option<GuardrailAction>,
    check_input: Option<bool>,
    check_output: Option<bool>,
    stream_output_check_chars: Option<usize>,
    exclude_paths: Option<Vec<String>>,
    fail_open: Option<bool>,
}

pub(crate) fn validate_gateway_guardrails(config: &GuardrailConfig) -> Result<(), String> {
    if !config.custom_rules.is_empty() {
        return Err("guardrails.custom_rules is not supported by the gateway runtime".to_string());
    }
    if config.default_action != GuardrailAction::Block {
        return Err(
            "guardrails.default_action is not supported; configure each enabled policy action"
                .to_string(),
        );
    }
    if !config.exclude_paths.is_empty() {
        return Err(
            "guardrails.exclude_paths is not supported by route-level gateway enforcement"
                .to_string(),
        );
    }

    let unsupported_mask = config
        .openai_moderation
        .as_ref()
        .is_some_and(|policy| policy.enabled && policy.action == GuardrailAction::Mask)
        || config
            .prompt_injection
            .as_ref()
            .is_some_and(|policy| policy.enabled && policy.action == GuardrailAction::Mask);
    if unsupported_mask {
        return Err(
            "guardrail action 'mask' is only supported for the PII policy; use block, log, or allow"
                .to_string(),
        );
    }

    Ok(())
}

pub(super) fn deserialize_gateway_guardrails<'de, D>(
    deserializer: D,
) -> Result<GuardrailConfig, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let wire = GatewayGuardrailsWire::deserialize(deserializer)?;
    let mut config = default_gateway_guardrails();
    if let Some(value) = wire.enabled {
        config.enabled = value;
    }
    if let Some(value) = wire.openai_moderation {
        config.openai_moderation = Some(value);
    }
    if let Some(value) = wire.pii {
        config.pii = Some(value);
    }
    if let Some(value) = wire.prompt_injection {
        config.prompt_injection = Some(value);
    }
    if let Some(value) = wire.custom_rules {
        config.custom_rules = value;
    }
    if let Some(value) = wire.default_action {
        config.default_action = value;
    }
    if let Some(value) = wire.check_input {
        config.check_input = value;
    }
    if let Some(value) = wire.check_output {
        config.check_output = value;
    }
    if let Some(value) = wire.stream_output_check_chars {
        config.stream_output_check_chars = value;
    }
    if let Some(value) = wire.exclude_paths {
        config.exclude_paths = value;
    }
    if let Some(value) = wire.fail_open {
        config.fail_open = value;
    }
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::super::gateway::GatewayConfig;
    use crate::config::Validate;

    #[test]
    fn partial_gateway_guardrails_keep_secure_defaults() {
        let mut value = serde_json::to_value(GatewayConfig::default()).unwrap();
        value["guardrails"] = serde_json::json!({"check_output": false});

        let config: GatewayConfig = serde_json::from_value(value).unwrap();

        assert!(config.guardrails.enabled);
        assert!(
            config
                .guardrails
                .prompt_injection
                .as_ref()
                .is_some_and(|policy| policy.enabled)
        );
        assert!(!config.guardrails.check_output);
    }

    #[test]
    fn gateway_rejects_guardrail_knobs_without_runtime_semantics() {
        for guardrails in [
            serde_json::json!({"custom_rules": [{"name": "deny", "patterns": ["secret"]}]}),
            serde_json::json!({"default_action": "log"}),
            serde_json::json!({"exclude_paths": ["/v1/chat/completions"]}),
            serde_json::json!({"prompt_injection": {"enabled": true, "action": "mask"}}),
        ] {
            let mut value = serde_json::to_value(GatewayConfig::default()).unwrap();
            value["guardrails"] = guardrails;
            let config: GatewayConfig = serde_json::from_value(value).unwrap();

            assert!(Validate::validate(&config).is_err());
        }
    }

    #[test]
    fn gateway_accepts_pii_masking_at_the_canonical_dto_boundary() {
        let mut value = serde_json::to_value(GatewayConfig::default()).unwrap();
        value["guardrails"] = serde_json::json!({"pii": {"enabled": true, "action": "mask"}});
        let config: GatewayConfig = serde_json::from_value(value).unwrap();

        assert!(config.guardrails.validate().is_ok());
        assert!(super::validate_gateway_guardrails(&config.guardrails).is_ok());
    }

    #[test]
    fn gateway_security_config_rejects_unknown_fields() {
        let guardrail_typo = serde_json::json!({"guardrails": {"enabld": false}});
        let ip_typo = serde_json::json!({"ip_access": {"enable": true}});

        assert!(serde_json::from_value::<GatewayConfig>(guardrail_typo).is_err());
        assert!(serde_json::from_value::<GatewayConfig>(ip_typo).is_err());
    }

    #[test]
    fn gateway_stream_output_window_defaults_and_validates_bounds() {
        let config = GatewayConfig::default();
        assert_eq!(config.guardrails.stream_output_check_chars, 256);

        for valid in [1, 4096] {
            let mut value = serde_json::to_value(GatewayConfig::default()).unwrap();
            value["guardrails"] = serde_json::json!({"stream_output_check_chars": valid});
            let config: GatewayConfig = serde_json::from_value(value).unwrap();
            assert!(config.guardrails.validate().is_ok());
        }

        for invalid in [0, 4097] {
            let mut value = serde_json::to_value(GatewayConfig::default()).unwrap();
            value["guardrails"] = serde_json::json!({"stream_output_check_chars": invalid});
            let config: GatewayConfig = serde_json::from_value(value).unwrap();
            assert!(config.guardrails.validate().is_err());
        }
    }
}
