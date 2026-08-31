use crate::core::providers::unified_provider::ProviderError;
use crate::core::providers::{Provider, ProviderType, bfl, stability};

pub(crate) fn build_image_provider(
    provider_type: ProviderType,
    mut config: serde_json::Value,
) -> Result<Provider, ProviderError> {
    normalize_native_image_endpoint(&mut config);
    match provider_type {
        ProviderType::Stability => {
            let mut typed: stability::StabilityConfig = serde_json::from_value(config.clone())
                .map_err(|error| ProviderError::configuration("stability", error.to_string()))?;
            merge_custom_headers(&mut typed.base.headers, &config);
            Ok(Provider::Stability(stability::StabilityProvider::new(
                typed,
            )?))
        }
        ProviderType::BlackForestLabs => {
            let mut typed: bfl::BflConfig =
                serde_json::from_value(config.clone()).map_err(|error| {
                    ProviderError::configuration("black_forest_labs", error.to_string())
                })?;
            merge_custom_headers(&mut typed.base.headers, &config);
            Ok(Provider::BlackForestLabs(Box::new(bfl::BflProvider::new(
                typed,
            )?)))
        }
        _ => Err(ProviderError::configuration(
            "media",
            "unsupported native image provider type",
        )),
    }
}

fn normalize_native_image_endpoint(config: &mut serde_json::Value) {
    let Some(config) = config.as_object_mut() else {
        return;
    };
    let endpoint = config
        .get("base_url")
        .and_then(serde_json::Value::as_str)
        .or_else(|| config.get("api_base").and_then(serde_json::Value::as_str))
        .map(str::to_string);
    if let Some(endpoint) = endpoint {
        config.insert("api_base".to_string(), endpoint.into());
    }
    config.remove("base_url");
}

fn merge_custom_headers(
    headers: &mut std::collections::HashMap<String, String>,
    config: &serde_json::Value,
) {
    let Some(custom_headers) = config
        .get("custom_headers")
        .and_then(serde_json::Value::as_object)
    else {
        return;
    };
    for (key, value) in custom_headers {
        if let Some(value) = value.as_str() {
            headers.insert(key.clone(), value.to_string());
        }
    }
}
