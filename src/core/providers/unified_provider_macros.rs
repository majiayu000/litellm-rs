/// Generate standard provider error helper functions.
///
/// Creates 9 free functions for a provider: `{prefix}_config_error`, `{prefix}_auth_error`,
/// `{prefix}_api_error`, `{prefix}_network_error`, `{prefix}_parse_error`,
/// `{prefix}_stream_error`, `{prefix}_rate_limit_error`, `{prefix}_model_error`,
/// `{prefix}_validation_error`.
///
/// # Usage
/// ```ignore
/// define_provider_error_helpers!("gemini", gemini);
/// // Generates: gemini_config_error, gemini_auth_error, etc.
/// ```
///
/// Provider-specific helpers (e.g. `gemini_safety_error`) should be defined manually.
#[macro_export]
macro_rules! define_provider_error_helpers {
    ($provider:expr, $prefix:ident) => {
        ::pastey::paste! {
            /// Create configuration error
            pub fn [<$prefix _config_error>](msg: impl Into<String>) -> $crate::core::providers::unified_provider::ProviderError {
                $crate::core::providers::unified_provider::ProviderError::configuration($provider, msg.into())
            }

            /// Create authentication error
            pub fn [<$prefix _auth_error>](msg: impl Into<String>) -> $crate::core::providers::unified_provider::ProviderError {
                $crate::core::providers::unified_provider::ProviderError::authentication($provider, msg.into())
            }

            /// Create API error with status code
            pub fn [<$prefix _api_error>](status: u16, msg: impl Into<String>) -> $crate::core::providers::unified_provider::ProviderError {
                $crate::core::providers::unified_provider::ProviderError::api_error($provider, status, msg.into())
            }

            /// Create network error
            pub fn [<$prefix _network_error>](msg: impl Into<String>) -> $crate::core::providers::unified_provider::ProviderError {
                $crate::core::providers::unified_provider::ProviderError::network($provider, msg.into())
            }

            /// Create parsing/serialization error
            pub fn [<$prefix _parse_error>](msg: impl Into<String>) -> $crate::core::providers::unified_provider::ProviderError {
                $crate::core::providers::unified_provider::ProviderError::serialization($provider, msg.into())
            }

            /// Create streaming error
            pub fn [<$prefix _stream_error>](msg: impl Into<String>) -> $crate::core::providers::unified_provider::ProviderError {
                $crate::core::providers::unified_provider::ProviderError::streaming_error($provider, "chat", None, None, msg.into())
            }

            /// Create rate limit error
            pub fn [<$prefix _rate_limit_error>](retry_after: Option<u64>) -> $crate::core::providers::unified_provider::ProviderError {
                $crate::core::providers::unified_provider::ProviderError::rate_limit($provider, retry_after)
            }

            /// Create model not found error
            pub fn [<$prefix _model_error>](model: impl Into<String>) -> $crate::core::providers::unified_provider::ProviderError {
                $crate::core::providers::unified_provider::ProviderError::model_not_found($provider, model.into())
            }

            /// Create validation/invalid request error
            pub fn [<$prefix _validation_error>](msg: impl Into<String>) -> $crate::core::providers::unified_provider::ProviderError {
                $crate::core::providers::unified_provider::ProviderError::invalid_request($provider, msg.into())
            }
        }
    };
}

// Legacy methods and ProviderErrorTrait implementation are in provider_error_conversions.rs

/// Generate standard provider error methods on `ProviderError`.
///
/// Creates methods like `{prefix}_authentication`, `{prefix}_rate_limit`, etc.
/// as `impl ProviderError` associated functions.
///
/// # Usage
/// ```ignore
/// impl_provider_error_helpers!("huggingface", huggingface);
/// // Generates: ProviderError::huggingface_authentication(...), etc.
/// ```
///
/// Provider-specific methods should be defined in a separate `impl` block.
#[macro_export]
macro_rules! impl_provider_error_helpers {
    ($provider:expr, $prefix:ident) => {
        ::pastey::paste! {
            impl $crate::core::providers::unified_provider::ProviderError {
                /// Create authentication error
                pub fn [<$prefix _authentication>](message: impl Into<String>) -> Self {
                    Self::authentication($provider, message)
                }

                /// Create rate limit error
                pub fn [<$prefix _rate_limit>](retry_after: Option<u64>) -> Self {
                    Self::rate_limit($provider, retry_after)
                }

                /// Create model not found error
                pub fn [<$prefix _model_not_found>](model: impl Into<String>) -> Self {
                    Self::model_not_found($provider, model)
                }

                /// Create invalid request error
                pub fn [<$prefix _invalid_request>](message: impl Into<String>) -> Self {
                    Self::invalid_request($provider, message)
                }

                /// Create network error
                pub fn [<$prefix _network_error>](message: impl Into<String>) -> Self {
                    Self::network($provider, message)
                }

                /// Create timeout error
                pub fn [<$prefix _timeout>](message: impl Into<String>) -> Self {
                    Self::Timeout {
                        provider: $provider,
                        message: message.into(),
                    }
                }

                /// Create response parsing error
                pub fn [<$prefix _response_parsing>](message: impl Into<String>) -> Self {
                    Self::response_parsing($provider, message)
                }

                /// Create configuration error
                pub fn [<$prefix _configuration>](message: impl Into<String>) -> Self {
                    Self::configuration($provider, message)
                }

                /// Create API error with status code
                pub fn [<$prefix _api_error>](status: u16, message: impl Into<String>) -> Self {
                    Self::api_error($provider, status, message)
                }

                /// Check if this is a provider-specific error
                pub fn [<is_ $prefix _error>](&self) -> bool {
                    self.provider() == $provider
                }
            }
        }
    };
}

/// Generate a standard `ErrorMapper` implementation for a provider.
///
/// Creates:
/// - `{Name}ErrorMapper` struct
/// - `ErrorMapper<ProviderError>` impl delegating to `default_http_error_mapper`
///
/// # Usage
/// ```ignore
/// define_standard_error_mapper!("deepseek", DeepSeek);
/// // Generates: pub struct DeepSeekErrorMapper; + impl ErrorMapper<ProviderError>
/// ```
#[macro_export]
macro_rules! define_standard_error_mapper {
    ($provider:expr, $name:ident) => {
        ::pastey::paste! {
            /// Error mapper for the provider, using the standard HTTP status code mapping.
            #[derive(Debug)]
            pub struct [<$name ErrorMapper>];

            impl $crate::core::traits::error_mapper::trait_def::ErrorMapper<
                $crate::core::providers::unified_provider::ProviderError,
            > for [<$name ErrorMapper>]
            {
                fn map_http_error(
                    &self,
                    status_code: u16,
                    response_body: &str,
                ) -> $crate::core::providers::unified_provider::ProviderError {
                    $crate::core::providers::unified_provider::default_http_error_mapper(
                        $provider,
                        status_code,
                        response_body,
                    )
                }
            }
        }
    };
}

/// Generate an extended `ErrorMapper` implementation for a provider.
///
/// Like `define_standard_error_mapper!` but includes 402, 413, 408/504, 502/503.
#[macro_export]
macro_rules! define_extended_error_mapper {
    ($provider:expr, $name:ident) => {
        ::pastey::paste! {
            /// Error mapper for the provider, using the extended HTTP status code mapping.
            #[derive(Debug)]
            pub struct [<$name ErrorMapper>];

            impl $crate::core::traits::error_mapper::trait_def::ErrorMapper<
                $crate::core::providers::unified_provider::ProviderError,
            > for [<$name ErrorMapper>]
            {
                fn map_http_error(
                    &self,
                    status_code: u16,
                    response_body: &str,
                ) -> $crate::core::providers::unified_provider::ProviderError {
                    $crate::core::providers::unified_provider::extended_http_error_mapper(
                        $provider,
                        status_code,
                        response_body,
                    )
                }
            }
        }
    };
}
