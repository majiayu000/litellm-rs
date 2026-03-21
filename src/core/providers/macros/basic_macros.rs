//! Basic provider implementation macros
//!
//! Contains `impl_provider_basics!`, `impl_error_conversion!`, `provider_config!`,
//! `impl_health_check!`, `build_request!`, `not_implemented!`, `model_list!`,
//! `impl_streaming!`, `validate_response!`, `with_retry!`, and `extract_usage!`.

/// Macro to implement common provider methods
#[macro_export]
macro_rules! impl_provider_basics {
    ($provider_type:ty, $name:literal, $capabilities:expr) => {
        impl $provider_type {
            pub fn name(&self) -> &str {
                $name
            }

            pub fn capabilities(&self) -> &[ProviderCapability] {
                $capabilities
            }
        }
    };
}

/// Macro to implement error conversion for all provider errors
/// This eliminates the 15 repetitive From implementations
#[macro_export]
macro_rules! impl_error_conversion {
    // Generate all error conversions for a provider
    ($($provider_error:ty => $provider_name:expr),+ $(,)?) => {
        $(
            impl From<$provider_error> for ProviderError {
                fn from(error: $provider_error) -> Self {
                    use $crate::core::types::errors::ProviderErrorTrait;

                    match error.error_type() {
                        $crate::core::types::errors::ErrorType::Authentication => {
                            ProviderError::Authentication {
                                provider: $provider_name,
                                message: error.to_string(),
                            }
                        }
                        $crate::core::types::errors::ErrorType::RateLimit => {
                            ProviderError::RateLimit {
                                provider: $provider_name,
                                message: error.to_string(),
                                retry_after: error.retry_after(),
                            }
                        }
                        $crate::core::types::errors::ErrorType::InvalidRequest => {
                            ProviderError::InvalidRequest {
                                provider: $provider_name,
                                message: error.to_string(),
                            }
                        }
                        $crate::core::types::errors::ErrorType::ModelNotFound => {
                            ProviderError::ModelNotFound {
                                provider: $provider_name,
                                model: error.model().unwrap_or("").to_string(),
                                message: error.to_string(),
                            }
                        }
                        $crate::core::types::errors::ErrorType::ServiceUnavailable => {
                            ProviderError::ServiceUnavailable {
                                provider: $provider_name,
                                message: error.to_string(),
                            }
                        }
                        $crate::core::types::errors::ErrorType::Timeout => {
                            ProviderError::Timeout {
                                provider: $provider_name,
                                message: error.to_string(),
                            }
                        }
                        $crate::core::types::errors::ErrorType::Network => {
                            ProviderError::Network {
                                provider: $provider_name,
                                message: error.to_string(),
                            }
                        }
                        $crate::core::types::errors::ErrorType::Serialization => {
                            ProviderError::Serialization {
                                provider: $provider_name,
                                message: error.to_string(),
                            }
                        }
                        $crate::core::types::errors::ErrorType::NotSupported => {
                            ProviderError::NotSupported {
                                provider: $provider_name,
                                feature: error.to_string(),
                            }
                        }
                        _ => ProviderError::Other {
                            provider: $provider_name,
                            message: error.to_string(),
                        }
                    }
                }
            }
        )+
    };
}

/// Macro to generate provider configuration struct
#[macro_export]
macro_rules! provider_config {
    (
        $(#[$meta:meta])*
        pub struct $name:ident {
            $(
                $(#[$field_meta:meta])*
                $field:ident: $field_type:ty $(= $default:expr)?
            ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
        pub struct $name {
            $(
                $(#[$field_meta])*
                $(#[serde(default = concat!("default_", stringify!($field)))])?
                pub $field: $field_type,
            )*
        }

        impl Default for $name {
            fn default() -> Self {
                Self {
                    $(
                        $field: provider_config!(@default $field_type $(, $default)?),
                    )*
                }
            }
        }

    };

    (@default $field_type:ty) => {
        <$field_type>::default()
    };

    (@default $field_type:ty, $default:expr) => {
        $default
    };
}

/// Macro to implement health check using a simple API call
#[macro_export]
macro_rules! impl_health_check {
    ($provider_type:ty, $endpoint:expr) => {
        async fn check_health(&self) -> Result<HealthStatus, ProviderError> {
            let url = format!("{}/{}", self.config.base_url, $endpoint);
            let response = self
                .client
                .get(&url)
                .header("Authorization", format!("Bearer {}", self.config.api_key))
                .send()
                .await;

            match response {
                Ok(res) if res.status().is_success() => Ok(HealthStatus::Healthy),
                Ok(res) if res.status() == 401 => {
                    Ok(HealthStatus::Unhealthy("Authentication failed".to_string()))
                }
                Ok(res) => Ok(HealthStatus::Unhealthy(format!("HTTP {}", res.status()))),
                Err(e) => Ok(HealthStatus::Unhealthy(format!("Connection failed: {}", e))),
            }
        }
    };
}

/// Macro to implement standard HTTP request builder
#[macro_export]
macro_rules! build_request {
    ($self:expr, $method:ident, $url:expr) => {{
        $self.client
            .$method($url)
            .header("Authorization", format!("Bearer {}", $self.config.api_key))
            .header("Content-Type", "application/json")
    }};

    ($self:expr, $method:ident, $url:expr, headers: {$($key:expr => $value:expr),* $(,)?}) => {{
        let mut request = $self.client
            .$method($url)
            .header("Authorization", format!("Bearer {}", $self.config.api_key))
            .header("Content-Type", "application/json");

        $(
            request = request.header($key, $value);
        )*

        request
    }};
}

/// Macro to implement not-implemented methods
#[macro_export]
macro_rules! not_implemented {
    ($provider:literal, $feature:literal) => {
        Err(ProviderError::NotImplemented {
            provider: $provider,
            feature: $feature.to_string(),
        })
    };
}

/// Macro to generate model list
#[macro_export]
macro_rules! model_list {
    ($provider:literal, $($model_id:literal),* $(,)?) => {
        vec![
            $(
                ModelInfo {
                    id: $model_id.to_string(),
                    name: $model_id.to_string(),
                    provider: $provider.to_string(),
                    max_context_length: 4096, // Default, should be overridden
                    max_output_length: None,
                    supports_streaming: true,
                    supports_tools: false, // Default
                    supports_multimodal: false, // Default
                    input_cost_per_1k_tokens: None,
                    output_cost_per_1k_tokens: None,
                    currency: "USD".to_string(),
                    capabilities: Vec::new(),
                    created_at: None,
                    updated_at: None,
                    metadata: std::collections::HashMap::new(),
                },
            )*
        ]
    };
}

/// Macro to implement streaming response handler
/// This eliminates the repetitive 20-line streaming handler pattern
#[macro_export]
macro_rules! impl_streaming {
    ($provider:literal, $response:expr) => {{ impl_streaming!($provider, $response, ChatChunk) }};

    ($provider:literal, $response:expr, $chunk_type:ty) => {{
        use futures::StreamExt;
        use std::pin::Pin;

        let stream = $response
            .bytes_stream()
            .map(move |chunk| {
                match chunk {
                    Ok(bytes) => {
                        let data = String::from_utf8_lossy(&bytes);

                        // Handle SSE format
                        if let Some(json_str) = data.strip_prefix("data: ") {
                            // Check for stream end
                            if json_str.trim() == "[DONE]" {
                                return Ok(None);
                            }

                            // Parse chunk
                            match serde_json::from_str::<$chunk_type>(json_str) {
                                Ok(chunk) => Ok(Some(chunk)),
                                Err(e) => Err(ProviderError::ResponseParsing {
                                    provider: $provider,
                                    message: format!("Failed to parse chunk: {}", e),
                                }),
                            }
                        } else if data.trim().is_empty() {
                            // Skip empty lines
                            Ok(None)
                        } else {
                            // Skip non-data lines (like comments)
                            Ok(None)
                        }
                    }
                    Err(e) => Err(ProviderError::Network {
                        provider: $provider,
                        message: format!("Stream error: {}", e),
                    }),
                }
            })
            .filter_map(|result| async move {
                match result {
                    Ok(Some(chunk)) => Some(Ok(chunk)),
                    Ok(None) => None,
                    Err(e) => Some(Err(e)),
                }
            });

        Ok(Box::pin(stream)
            as Pin<
                Box<dyn futures::Stream<Item = Result<$chunk_type, ProviderError>> + Send>,
            >)
    }};
}

/// Macro to validate required fields in a response
#[macro_export]
macro_rules! validate_response {
    ($response:expr, $provider:literal, required: [$($field:literal),* $(,)?]) => {{
        if !$response.is_object() {
            return Err(ProviderError::ResponseParsing {
                provider: $provider,
                message: "Response is not a JSON object".to_string(),
            });
        }

        $(
            if $response.get($field).is_none() {
                return Err(ProviderError::ResponseParsing {
                    provider: $provider,
                    message: format!("Missing required field: {}", $field),
                });
            }
        )*

        Ok(())
    }};
}

/// Macro to implement retry logic
#[macro_export]
macro_rules! with_retry {
    ($provider:literal, $max_retries:expr, $operation:expr) => {{
        let mut retries = 0;
        let mut delay = std::time::Duration::from_secs(1);

        loop {
            match $operation {
                Ok(result) => break Ok(result),
                Err(e) if retries < $max_retries => {
                    retries += 1;
                    tracing::warn!(
                        "Provider {} operation failed: {}, retrying ({}/{})",
                        $provider,
                        e,
                        retries,
                        $max_retries
                    );
                    tokio::time::sleep(delay).await;
                    delay *= 2; // Exponential backoff
                }
                Err(e) => break Err(e),
            }
        }
    }};
}

/// Macro to implement usage extraction from response
#[macro_export]
macro_rules! extract_usage {
    ($response:expr) => {{
        $response.get("usage").and_then(|u| {
            Some(Usage {
                prompt_tokens: u.get("prompt_tokens")?.as_u64()? as u32,
                completion_tokens: u.get("completion_tokens")?.as_u64()? as u32,
                total_tokens: u.get("total_tokens")?.as_u64()? as u32,
            })
        })
    }};
}
