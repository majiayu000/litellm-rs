//! HTTP provider macros with custom hooks
//!
//! - `define_http_provider_with_hooks!` — HTTP-based providers with custom request/response hooks
//! - `define_pooled_http_provider_with_hooks!` — Pooled HTTP providers using GlobalPoolManager

/// Macro to define HTTP-based providers with custom hooks for request/response handling.
#[macro_export]
macro_rules! define_http_provider_with_hooks {
    (
        provider: $provider_name:expr,
        struct_name: $struct_name:ident,
        config: $config_type:path,
        error_mapper: $error_mapper:path,
        model_info: $model_info:path,
        capabilities: $capabilities:expr,
        url_builder: $url_builder:expr,
        request_builder: $request_builder:expr,
        supported_params: [$($param:literal),* $(,)?],
        build_headers: $build_headers:expr,
        with_api_key: $with_api_key:expr,
        request_transform: $request_transform:expr,
        response_transform: $response_transform:expr,
        error_map: $error_map:expr,
        health_check: $health_check:expr,
        streaming_error: $streaming_error:expr,
        calculate_cost: $calculate_cost:expr $(,)?
    ) => {
        $crate::define_http_provider_with_hooks!(@impl
            provider: $provider_name,
            struct_name: $struct_name,
            config: $config_type,
            error_mapper: $error_mapper,
            model_info: $model_info,
            capabilities: $capabilities,
            url_builder: $url_builder,
            request_builder: $request_builder,
            supported_params: [$($param),*],
            build_headers: $build_headers,
            with_api_key: $with_api_key,
            request_transform: $request_transform,
            response_transform: $response_transform,
            error_map: $error_map,
            health_check: $health_check,
            streaming_error: $streaming_error,
            calculate_cost: $calculate_cost
        );
    };
    (
        provider: $provider_name:expr,
        struct_name: $struct_name:ident,
        config: $config_type:path,
        error_mapper: $error_mapper:path,
        model_info: $model_info:path,
        capabilities: $capabilities:expr,
        url_builder: $url_builder:expr,
        request_builder: $request_builder:expr,
        supported_params: [$($param:literal),* $(,)?],
        build_headers: $build_headers:expr,
        with_api_key: $with_api_key:expr,
        request_transform: $request_transform:expr,
        response_transform: $response_transform:expr,
        error_map: $error_map:expr,
        health_check: $health_check:expr,
        streaming_error: $streaming_error:expr $(,)?
    ) => {
        $crate::define_http_provider_with_hooks!(@impl
            provider: $provider_name,
            struct_name: $struct_name,
            config: $config_type,
            error_mapper: $error_mapper,
            model_info: $model_info,
            capabilities: $capabilities,
            url_builder: $url_builder,
            request_builder: $request_builder,
            supported_params: [$($param),*],
            build_headers: $build_headers,
            with_api_key: $with_api_key,
            request_transform: $request_transform,
            response_transform: $response_transform,
            error_map: $error_map,
            health_check: $health_check,
            streaming_error: $streaming_error,
            calculate_cost: |provider: &$struct_name,
                              model: &str,
                              input_tokens: u32,
                              output_tokens: u32|
             -> Result<f64, $crate::core::providers::unified_provider::ProviderError> {
                let model_info = provider
                    .supported_models
                    .iter()
                    .find(|m| m.id == model)
                    .ok_or_else(|| {
                        $crate::core::providers::unified_provider::ProviderError::model_not_found(
                            $provider_name,
                            model.to_string(),
                        )
                    })?;

                let input_cost = model_info
                    .input_cost_per_1k_tokens
                    .unwrap_or(0.0)
                    * input_tokens as f64
                    / 1000.0;
                let output_cost = model_info
                    .output_cost_per_1k_tokens
                    .unwrap_or(0.0)
                    * output_tokens as f64
                    / 1000.0;

                Ok(input_cost + output_cost)
            }
        );
    };
    (
        @impl
        provider: $provider_name:expr,
        struct_name: $struct_name:ident,
        config: $config_type:path,
        error_mapper: $error_mapper:path,
        model_info: $model_info:path,
        capabilities: $capabilities:expr,
        url_builder: $url_builder:expr,
        request_builder: $request_builder:expr,
        supported_params: [$($param:literal),* $(,)?],
        build_headers: $build_headers:expr,
        with_api_key: $with_api_key:expr,
        request_transform: $request_transform:expr,
        response_transform: $response_transform:expr,
        error_map: $error_map:expr,
        health_check: $health_check:expr,
        streaming_error: $streaming_error:expr,
        calculate_cost: $calculate_cost:expr $(,)?
    ) => {
        #[derive(Debug, Clone)]
        pub struct $struct_name {
            config: $config_type,
            http_client: std::sync::Arc<reqwest::Client>,
            supported_models: Vec<$crate::core::types::model::ModelInfo>,
        }

        impl $struct_name {
            pub fn new(config: $config_type) -> Result<Self, $crate::core::providers::unified_provider::ProviderError> {
                <$config_type as $crate::core::traits::provider::ProviderConfig>::validate(&config)
                    .map_err(|e| $crate::core::providers::unified_provider::ProviderError::configuration($provider_name, e))?;

                let http_client = if <$config_type as $crate::core::traits::provider::ProviderConfig>::use_ssrf_safe_client(&config) {
                    $crate::utils::net::http::get_ssrf_safe_client_with_timeout_fallible(
                        <$config_type as $crate::core::traits::provider::ProviderConfig>::timeout(&config),
                    )
                } else {
                    $crate::utils::net::http::get_client_with_timeout_fallible(
                        <$config_type as $crate::core::traits::provider::ProviderConfig>::timeout(&config),
                    )
                }
                .map_err(|e| {
                    $crate::core::providers::unified_provider::ProviderError::initialization(
                        $provider_name,
                        format!("Failed to create HTTP client: {}", e),
                    )
                })?;

                Ok(Self {
                    config,
                    http_client,
                    supported_models: ($model_info)(),
                })
            }

            pub async fn with_api_key(
                api_key: impl Into<String>,
            ) -> Result<Self, $crate::core::providers::unified_provider::ProviderError> {
                ($with_api_key)(api_key.into())
            }

            fn build_headers(&self) -> std::collections::HashMap<String, String> {
                let mut headers = std::collections::HashMap::new();
                ($build_headers)(self, &mut headers);
                headers
            }
        }
        #[async_trait::async_trait]
        impl $crate::core::traits::provider::llm_provider::trait_definition::LLMProvider for $struct_name {
            type Config = $config_type;
            type Error = $crate::core::providers::unified_provider::ProviderError;
            type ErrorMapper = $error_mapper;

            fn name(&self) -> &'static str {
                $provider_name
            }

            fn capabilities(&self) -> &'static [$crate::core::types::model::ProviderCapability] {
                $capabilities
            }

            fn models(&self) -> &[$crate::core::types::model::ModelInfo] {
                &self.supported_models
            }

            fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
                &[$($param),*]
            }

            async fn map_openai_params(
                &self,
                params: std::collections::HashMap<String, serde_json::Value>,
                _model: &str,
            ) -> Result<std::collections::HashMap<String, serde_json::Value>, Self::Error> {
                Ok(params)
            }

            async fn transform_request(
                &self,
                request: $crate::core::types::chat::ChatRequest,
                _context: $crate::core::types::context::RequestContext,
            ) -> Result<serde_json::Value, Self::Error> {
                ($request_transform)(self, request)
            }

            async fn transform_response(
                &self,
                raw_response: &[u8],
                _model: &str,
                _request_id: &str,
            ) -> Result<$crate::core::types::responses::ChatResponse, Self::Error> {
                ($response_transform)(self, raw_response, _model, _request_id)
            }

            fn get_error_mapper(&self) -> Self::ErrorMapper {
                $error_mapper
            }
            async fn chat_completion(
                &self,
                request: $crate::core::types::chat::ChatRequest,
                context: $crate::core::types::context::RequestContext,
            ) -> Result<$crate::core::types::responses::ChatResponse, Self::Error> {
                let url = ($url_builder)(self);

                let body = self.transform_request(request.clone(), context).await?;
                let headers = self.build_headers();

                let mut req_builder = ($request_builder)(self, &url);
                for (key, value) in headers {
                    req_builder = req_builder.header(key, value);
                }

                let response = req_builder
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| {
                        $crate::core::providers::unified_provider::ProviderError::network(
                            $provider_name,
                            e.to_string(),
                        )
                    })?;

                if !response.status().is_success() {
                    let status = response.status().as_u16();
                    let error_text = response.text().await.unwrap_or_default();
                    return Err(($error_map)(self, status, error_text, &request));
                }

                let response_bytes = response
                    .bytes()
                    .await
                    .map_err(|e| {
                        $crate::core::providers::unified_provider::ProviderError::network(
                            $provider_name,
                            e.to_string(),
                        )
                    })?;

                self.transform_response(&response_bytes, &request.model, "")
                    .await
            }

            async fn chat_completion_stream(
                &self,
                _request: $crate::core::types::chat::ChatRequest,
                _context: $crate::core::types::context::RequestContext,
            ) -> Result<
                std::pin::Pin<
                    Box<
                        dyn futures::Stream<
                                Item = Result<$crate::core::types::responses::ChatChunk, Self::Error>,
                            > + Send,
                    >,
                >,
                Self::Error,
            > {
                Err($crate::core::providers::unified_provider::ProviderError::not_implemented(
                    $provider_name,
                    $streaming_error,
                ))
            }

            async fn health_check(&self) -> $crate::core::types::health::HealthStatus {
                ($health_check)(self).await
            }

            async fn calculate_cost(
                &self,
                model: &str,
                input_tokens: u32,
                output_tokens: u32,
            ) -> Result<f64, Self::Error> {
                ($calculate_cost)(self, model, input_tokens, output_tokens)
            }
        }
    };
}
