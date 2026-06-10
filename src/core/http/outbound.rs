//! Shared outbound HTTP client configuration.

use reqwest::{Client, ClientBuilder};
use std::sync::OnceLock;
use std::time::Duration;

static DEFAULT_CLIENT: OnceLock<Client> = OnceLock::new();
static STREAMING_CLIENT: OnceLock<Client> = OnceLock::new();

/// Return the process-wide default outbound HTTP client.
///
/// `reqwest::Client` is cheap to clone, so callers that need an owned client
/// should use `default_outbound_client().clone()`.
pub fn default_outbound_client() -> &'static Client {
    DEFAULT_CLIENT.get_or_init(|| {
        build_outbound_client(OutboundProfile::default())
            .expect("default outbound client must build")
    })
}

/// Return the process-wide outbound HTTP client for streaming responses.
///
/// This client keeps connect and pool limits from the default profile, but does
/// not set reqwest's total request timeout. Streaming routes enforce their own
/// idle timeout while bytes are being read; a total response-lifetime timeout
/// would cut off valid long generations.
pub fn streaming_outbound_client() -> &'static Client {
    STREAMING_CLIENT.get_or_init(|| {
        build_streaming_outbound_client(OutboundProfile::default())
            .expect("streaming outbound client must build")
    })
}

/// Tunable outbound HTTP profile shared by providers and integrations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutboundProfile {
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub pool_idle_timeout: Duration,
    pub pool_idle_per_host: usize,
    pub user_agent: String,
}

impl Default for OutboundProfile {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(120),
            pool_idle_timeout: Duration::from_secs(90),
            pool_idle_per_host: 32,
            user_agent: format!("litellm-rs/{}", crate::version::VERSION),
        }
    }
}

/// Build an outbound client from a profile.
pub fn build_outbound_client(profile: OutboundProfile) -> reqwest::Result<Client> {
    outbound_client_builder(&profile)
        .timeout(profile.request_timeout)
        .build()
}

/// Build an outbound streaming client from a profile.
///
/// Intentionally omits `ClientBuilder::timeout`, because reqwest treats that as
/// a total request deadline rather than an idle/read timeout.
pub fn build_streaming_outbound_client(profile: OutboundProfile) -> reqwest::Result<Client> {
    outbound_client_builder(&profile).build()
}

fn outbound_client_builder(profile: &OutboundProfile) -> ClientBuilder {
    ClientBuilder::new()
        .connect_timeout(profile.connect_timeout)
        .pool_idle_timeout(Some(profile.pool_idle_timeout))
        .pool_max_idle_per_host(profile.pool_idle_per_host)
        .user_agent(profile.user_agent.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_has_expected_timeouts() {
        let profile = OutboundProfile::default();

        assert_eq!(profile.connect_timeout, Duration::from_secs(5));
        assert_eq!(profile.request_timeout, Duration::from_secs(120));
        assert_eq!(profile.pool_idle_timeout, Duration::from_secs(90));
        assert_eq!(profile.pool_idle_per_host, 32);
        assert!(profile.user_agent.starts_with("litellm-rs/"));
    }

    #[test]
    fn build_outbound_client_accepts_default_profile() {
        let client = build_outbound_client(OutboundProfile::default());
        assert!(client.is_ok());
    }

    #[test]
    fn build_streaming_outbound_client_accepts_default_profile_without_total_timeout() {
        let client = build_streaming_outbound_client(OutboundProfile::default());
        assert!(client.is_ok());
    }

    #[test]
    fn default_outbound_client_is_singleton() {
        let first = default_outbound_client();
        let second = default_outbound_client();

        assert!(std::ptr::eq(first, second));
    }

    #[test]
    fn streaming_outbound_client_is_separate_singleton() {
        let first = streaming_outbound_client();
        let second = streaming_outbound_client();

        assert!(std::ptr::eq(first, second));
        assert!(!std::ptr::eq(first, default_outbound_client()));
    }
}
