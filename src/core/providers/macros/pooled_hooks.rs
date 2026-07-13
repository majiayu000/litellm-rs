//! Shared HTTP provider macro
//!
//! `define_pooled_http_provider_with_hooks!` targets providers that:
//! - Reuse policy-aware clients for ordinary and streaming requests
//! - Have custom request/response transforms
//! - Optionally implement streaming via a custom async hook

/// Macro to define pooled HTTP providers that use GlobalPoolManager with custom hooks.
#[macro_export]
macro_rules! define_pooled_http_provider_with_hooks {
    (
        provider: $provider_name:expr,
        struct_name: $struct_name:ident,
        config: $config_type:path,
        error_mapper: $error_mapper:path,
        model_info: $model_info:path,
        capabilities: $capabilities:expr,
        url_builder: $url_builder:expr,
        http_method: $http_method:expr,
        supported_params: [$($param:literal),* $(,)?],
        build_headers: $build_headers:expr,
        with_api_key: $with_api_key:expr,
        map_openai_params: $map_openai_params:expr,
        request_transform: $request_transform:expr,
        response_transform: $response_transform:expr,
        error_map: $error_map:expr,
        health_check: $health_check:expr,
        streaming: $streaming:expr,
        calculate_cost: $calculate_cost:expr $(,)?
    ) => {
        #[derive(Debug, Clone)]
        pub struct $struct_name {
            config: $config_type,
            http_client: $crate::core::providers::base::BaseHttpClient,
            streaming_client: $crate::core::providers::base::BaseHttpClient,
            supported_models: Vec<$crate::core::types::model::ModelInfo>,
        }

        impl $struct_name {
            pub fn new(
                config: $config_type,
            ) -> Result<Self, $crate::core::providers::unified_provider::ProviderError> {
                <$config_type as $crate::core::traits::provider::ProviderConfig>::validate(&config)
                    .map_err(|e| {
                        $crate::core::providers::unified_provider::ProviderError::configuration(
                            $provider_name,
                            e,
                        )
                    })?;
                let base_config = $crate::core::providers::base::BaseConfig {
                    api_base: <$config_type as $crate::core::traits::provider::ProviderConfig>::api_base(&config)
                        .map(std::borrow::ToOwned::to_owned),
                    endpoint_access: <$config_type as $crate::core::traits::provider::ProviderConfig>::endpoint_access(&config),
                    timeout: <$config_type as $crate::core::traits::provider::ProviderConfig>::timeout(&config).as_secs(),
                    ..Default::default()
                };
                let http_client = $crate::core::providers::base::BaseHttpClient::new_for_provider(
                    $provider_name,
                    base_config.clone(),
                )?;
                let streaming_client = $crate::core::providers::base::BaseHttpClient::new_for_provider_streaming(
                    $provider_name,
                    base_config,
                )?;

                Ok(Self {
                    config,
                    http_client,
                    streaming_client,
                    supported_models: ($model_info)(),
                })
            }

            pub async fn with_api_key(
                api_key: impl Into<String>,
            ) -> Result<Self, $crate::core::providers::unified_provider::ProviderError> {
                ($with_api_key)(api_key.into())
            }

            fn build_headers(&self) -> Vec<$crate::core::providers::base::HeaderPair> {
                ($build_headers)(self)
            }
        }

                impl $crate::core::traits::provider::llm_provider::trait_definition::LLMProvider for $struct_name {

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
                model: &str,
            ) -> Result<std::collections::HashMap<String, serde_json::Value>, $crate::core::providers::unified_provider::ProviderError> {
                ($map_openai_params)(self, params, model)
            }

            async fn transform_request(
                &self,
                request: $crate::core::types::chat::ChatRequest,
                _context: $crate::core::types::context::RequestContext,
            ) -> Result<serde_json::Value, $crate::core::providers::unified_provider::ProviderError> {
                ($request_transform)(self, request)
            }

            async fn transform_response(
                &self,
                raw_response: &[u8],
                _model: &str,
                _request_id: &str,
            ) -> Result<$crate::core::types::responses::ChatResponse, $crate::core::providers::unified_provider::ProviderError> {
                ($response_transform)(self, raw_response, _model, _request_id)
            }

            fn get_error_mapper(&self) -> Box<dyn $crate::core::traits::error_mapper::trait_def::ErrorMapper<$crate::core::providers::unified_provider::ProviderError>> {
                Box::new($error_mapper)
            }

            async fn chat_completion(
                &self,
                request: $crate::core::types::chat::ChatRequest,
                context: $crate::core::types::context::RequestContext,
            ) -> Result<$crate::core::types::responses::ChatResponse, $crate::core::providers::unified_provider::ProviderError> {
                let url = ($url_builder)(self);
                let body = self.transform_request(request.clone(), context.clone()).await?;
                let headers = self.build_headers();

                let method = match $http_method {
                    $crate::core::providers::base::HttpMethod::GET => reqwest::Method::GET,
                    $crate::core::providers::base::HttpMethod::POST => reqwest::Method::POST,
                    $crate::core::providers::base::HttpMethod::PUT => reqwest::Method::PUT,
                    $crate::core::providers::base::HttpMethod::DELETE => reqwest::Method::DELETE,
                };
                let request_builder = $crate::core::providers::base::apply_provider_headers(
                    self.http_client.request(method, &url)?,
                    headers,
                )
                .json(&body);
                let response = request_builder.send().await.map_err(|error| {
                    $crate::core::providers::unified_provider::ProviderError::network(
                        $provider_name,
                        error.to_string(),
                    )
                })?;

                let status = response.status();
                let response_bytes = response
                    .bytes()
                    .await
                    .map_err(|e| {
                        $crate::core::providers::unified_provider::ProviderError::network(
                            $provider_name,
                            e.to_string(),
                        )
                    })?;
                if !status.is_success() {
                    let error_text = String::from_utf8_lossy(&response_bytes).to_string();
                    return Err(($error_map)(self, status.as_u16(), error_text, &request));
                }

                self.transform_response(&response_bytes, &request.model, &context.request_id)
                    .await
            }

            async fn chat_completion_stream(
                &self,
                request: $crate::core::types::chat::ChatRequest,
                context: $crate::core::types::context::RequestContext,
            ) -> Result<
                std::pin::Pin<
                    Box<
                        dyn futures::Stream<
                                Item = Result<$crate::core::types::responses::ChatChunk, $crate::core::providers::unified_provider::ProviderError>,
                            > + Send,
                    >,
                >,
                $crate::core::providers::unified_provider::ProviderError,
            > {
                ($streaming)(self, request, context).await
            }

            async fn health_check(&self) -> $crate::core::types::health::HealthStatus {
                ($health_check)(self).await
            }

            async fn calculate_cost(
                &self,
                model: &str,
                input_tokens: u32,
                output_tokens: u32,
            ) -> Result<f64, $crate::core::providers::unified_provider::ProviderError> {
                ($calculate_cost)(self, model, input_tokens, output_tokens)
            }
        }
    };
}
