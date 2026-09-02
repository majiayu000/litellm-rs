use crate::core::guardrails::GuardrailEngine;
use crate::utils::error::gateway_error::GatewayError;

pub(super) fn scannable(url: &str) -> &str {
    let Some(comma) = url.find(',') else {
        return url;
    };
    let header = &url[..comma];
    if url
        .get(..5)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("data:"))
        && header[5..]
            .split(';')
            .any(|parameter| parameter.eq_ignore_ascii_case("base64"))
    {
        header
    } else {
        url
    }
}

pub(super) fn mask(engine: &GuardrailEngine, url: &mut String) -> Result<bool, GatewayError> {
    let scanned_len = scannable(url).len();
    if scanned_len == url.len() {
        return super::mask_text(engine, url);
    }
    let mut header = url[..scanned_len].to_string();
    let modified = super::mask_text(engine, &mut header)?;
    if modified {
        header.push_str(&url[scanned_len..]);
        *url = header;
    }
    Ok(modified)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::gateway::GatewayConfig;
    use crate::core::guardrails::{GuardrailAction, PIIConfig};

    #[test]
    fn base64_payload_is_excluded_from_scanning_and_masking() {
        let mut config = GatewayConfig::default().guardrails;
        config.pii = Some(PIIConfig {
            enabled: true,
            action: GuardrailAction::Mask,
            mask_pattern: Some("[MASKED]".to_string()),
            ..PIIConfig::default()
        });
        let engine = GuardrailEngine::new(config).expect("PII policy must compile");
        let mut url = "data:image/png;base64,2125551234==".to_string();

        assert_eq!(scannable(&url), "data:image/png;base64");
        assert!(!mask(&engine, &mut url).expect("data URL should mask"));
        assert_eq!(url, "data:image/png;base64,2125551234==");
        assert_eq!(
            scannable("https://example.com/2125551234"),
            "https://example.com/2125551234"
        );
    }
}
