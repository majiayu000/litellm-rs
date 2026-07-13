//! Policy-aware HTTP client used by configurable AI route proxies.

use reqwest::IntoUrl;

use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::base::{BaseConfig, BaseHttpClient, ProviderRequestBuilder};
use crate::utils::error::gateway_error::GatewayError;

#[derive(Debug, Clone)]
pub(super) struct RouteHttpClient {
    ordinary: BaseHttpClient,
    streaming: BaseHttpClient,
}

impl RouteHttpClient {
    pub(super) fn new(
        provider: &'static str,
        base_url: String,
        endpoint_access: ProviderEndpointAccess,
        timeout: u64,
    ) -> Result<Self, GatewayError> {
        let config = BaseConfig {
            api_base: Some(base_url),
            endpoint_access,
            timeout,
            ..BaseConfig::default()
        };
        let ordinary = BaseHttpClient::new_for_provider(provider, config.clone())?;
        let streaming = BaseHttpClient::new_for_provider_streaming(provider, config)?;
        Ok(Self {
            ordinary,
            streaming,
        })
    }

    pub(super) fn ordinary_get<U: IntoUrl>(
        &self,
        url: U,
    ) -> Result<ProviderRequestBuilder, GatewayError> {
        self.ordinary.get(url).map_err(GatewayError::from)
    }

    pub(super) fn ordinary_post<U: IntoUrl>(
        &self,
        url: U,
    ) -> Result<ProviderRequestBuilder, GatewayError> {
        self.ordinary.post(url).map_err(GatewayError::from)
    }

    pub(super) fn streaming_post<U: IntoUrl>(
        &self,
        url: U,
    ) -> Result<ProviderRequestBuilder, GatewayError> {
        self.streaming.post(url).map_err(GatewayError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_route_client_rejects_loopback_base() {
        let error = RouteHttpClient::new(
            "route_test",
            "http://127.0.0.1:11434/v1".to_string(),
            ProviderEndpointAccess::PublicOnly,
            30,
        )
        .expect_err("public-only route client must reject loopback");

        assert!(error.to_string().contains("SSRF protection"));
    }

    #[test]
    fn private_route_client_pins_ordinary_and_streaming_authority() {
        let client = RouteHttpClient::new(
            "route_test",
            "http://127.0.0.1:11434/v1".to_string(),
            ProviderEndpointAccess::PrivateNetwork,
            30,
        )
        .expect("private route client should build");

        for error in [
            client
                .ordinary_post("http://127.0.0.1:11435/v1/request")
                .err()
                .unwrap_or_else(|| panic!("ordinary request must reject cross-authority URL")),
            client
                .streaming_post("http://127.0.0.1:11435/v1/request")
                .err()
                .unwrap_or_else(|| panic!("streaming request must reject cross-authority URL")),
        ] {
            assert!(error.to_string().contains("does not match"));
        }
    }

    #[test]
    fn private_route_client_still_rejects_metadata() {
        let error = RouteHttpClient::new(
            "route_test",
            "http://169.254.169.254/latest".to_string(),
            ProviderEndpointAccess::PrivateNetwork,
            30,
        )
        .expect_err("private route client must reject metadata");

        assert!(error.to_string().contains("SSRF protection"));
    }
}
