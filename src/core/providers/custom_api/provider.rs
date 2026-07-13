//! Custom HTTPX Provider Implementation

crate::define_http_provider_with_hooks!(
    provider: super::PROVIDER_NAME,
    struct_name: CustomHttpxProvider,
    config: super::config::CustomHttpxConfig,
    error_mapper: super::error_mapper::CustomApiErrorMapper,
    model_info: super::model_info::get_supported_models,
    capabilities: &[
        crate::core::types::model::ProviderCapability::ChatCompletion,
        crate::core::types::model::ProviderCapability::ChatCompletionStream,
    ],
    url_builder: |provider: &CustomHttpxProvider| -> String { provider.config.endpoint_url.clone() },
    request_builder: |provider: &CustomHttpxProvider, url: &str| {
        match provider.config.http_method.to_uppercase().as_str() {
            "GET" => provider.http_client.get(url),
            "POST" => provider.http_client.post(url),
            "PUT" => provider.http_client.request(reqwest::Method::PUT, url),
            _ => provider.http_client.post(url),
        }
    },
    supported_params: ["temperature", "max_tokens", "top_p", "stream", "stop"],
    build_headers: |provider: &CustomHttpxProvider,
                    headers: &mut std::collections::HashMap<String, String>| {
        if let Some(api_key) = &provider.config.base.api_key {
            headers.insert("Authorization".to_string(), format!("Bearer {}", api_key));
        }

        headers.insert("Content-Type".to_string(), "application/json".to_string());
    },
    with_api_key: |api_key: String| -> Result<CustomHttpxProvider, crate::core::providers::unified_provider::ProviderError> {
        let _ = api_key;
        Err(crate::core::providers::unified_provider::ProviderError::not_supported(
            "custom_httpx",
            "with_api_key is not supported; use with_endpoint",
        ))
    },
    request_transform: |provider: &CustomHttpxProvider,
                        request: crate::core::types::chat::ChatRequest|
     -> Result<serde_json::Value, crate::core::providers::unified_provider::ProviderError> {
        if let Some(template) = &provider.config.request_template {
            let req_str = template.replace("{model}", &request.model).replace(
                "{messages}",
                &serde_json::to_string(&request.messages).map_err(|e| {
                    crate::core::providers::unified_provider::ProviderError::serialization(
                        "custom_httpx",
                        e.to_string(),
                    )
                })?,
            );

            serde_json::from_str(&req_str).map_err(|e| {
                crate::core::providers::unified_provider::ProviderError::serialization(
                    "custom_httpx",
                    e.to_string(),
                )
            })
        } else {
            let mut req = serde_json::json!({
                "model": request.model,
                "messages": request.messages,
            });

            if let Some(max_tokens) = request.max_tokens {
                req["max_tokens"] = serde_json::Value::Number(max_tokens.into());
            }

            if let Some(temperature) = request.temperature {
                req["temperature"] = serde_json::to_value(temperature).map_err(|e| {
                    crate::core::providers::unified_provider::ProviderError::serialization(
                        "custom_httpx",
                        e.to_string(),
                    )
                })?;
            }

            Ok(req)
        }
    },
    response_transform: |_provider: &CustomHttpxProvider,
                         raw_response: &[u8],
                         _model: &str,
                         _request_id: &str|
     -> Result<crate::core::types::responses::ChatResponse, crate::core::providers::unified_provider::ProviderError> {
        let response_text = String::from_utf8_lossy(raw_response);
        let response: crate::core::types::responses::ChatResponse =
            serde_json::from_str(&response_text).map_err(|e| {
                crate::core::providers::unified_provider::ProviderError::serialization(
                    "custom_httpx",
                    e.to_string(),
                )
            })?;
        Ok(response)
    },
    error_map: |_provider: &CustomHttpxProvider,
                status: u16,
                error_text: String,
                request: &crate::core::types::chat::ChatRequest|
     -> crate::core::providers::unified_provider::ProviderError {
        match status {
            401 => crate::core::providers::unified_provider::ProviderError::authentication(
                "custom_httpx",
                error_text,
            ),
            429 => crate::core::providers::unified_provider::ProviderError::rate_limit(
                "custom_httpx",
                None,
            ),
            404 => crate::core::providers::unified_provider::ProviderError::model_not_found(
                "custom_httpx",
                request.model.as_str(),
            ),
            _ => crate::core::providers::base::HttpErrorMapper::map_status_code(
                "custom_httpx",
                status,
                &error_text,
            ),
        }
    },
    health_check: |_provider: &CustomHttpxProvider| async {
        crate::core::types::health::HealthStatus::Healthy
    },
    streaming_error: "Streaming not yet implemented",
);

impl CustomHttpxProvider {
    pub fn with_endpoint(
        endpoint_url: impl Into<String>,
    ) -> Result<Self, crate::core::providers::unified_provider::ProviderError> {
        let config = super::config::CustomHttpxConfig::new(endpoint_url);
        Self::new(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::net::ProviderEndpointAccess;

    #[test]
    fn policy_client_construction_honors_custom_endpoint_access() {
        let public = super::super::config::CustomHttpxConfig::new("http://127.0.0.1:8080/endpoint");
        assert!(CustomHttpxProvider::new(public).is_err());

        let mut private =
            super::super::config::CustomHttpxConfig::new("http://127.0.0.1:8080/endpoint");
        private.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        assert!(CustomHttpxProvider::new(private).is_ok());
    }
}
