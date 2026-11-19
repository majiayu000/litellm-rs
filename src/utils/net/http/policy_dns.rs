//! Connection-time DNS policy for policy-bound provider clients.
//!
//! This mitigates DNS-rebinding attacks: even if a hostname resolves to a public IP
//! at config-validation time, every actual connection re-validates the resolved
//! addresses against the same [`EndpointAddressRules`] (access level plus
//! `endpoint_allow_cidrs`) that validated the configured endpoint.

use std::error::Error as _;
use std::io;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::Arc;

use crate::core::net::{EndpointAddressRules, SsrfError};

pub(super) trait HostResolver: Send + Sync {
    fn resolve(&self, host: &str) -> io::Result<Vec<SocketAddr>>;
}

pub(super) struct SystemHostResolver;

impl HostResolver for SystemHostResolver {
    fn resolve(&self, host: &str) -> io::Result<Vec<SocketAddr>> {
        (host, 0u16)
            .to_socket_addrs()
            .map(|addresses| addresses.collect())
    }
}

pub(super) struct PolicyDnsResolver {
    pub(super) rules: EndpointAddressRules,
    pub(super) resolver: Arc<dyn HostResolver>,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum ProviderEndpointPolicyRequestError {
    #[error("Host resolution returned no addresses (SSRF protection)")]
    EmptyDnsAnswer,
    /// Carries the hostname, the rejected address, and the setting that would allow it.
    #[error("{0}")]
    DisallowedDnsAddress(SsrfError),
    #[error("Redirect target failed SSRF validation")]
    RedirectTarget,
}

fn endpoint_policy_error(error: &reqwest::Error) -> Option<&ProviderEndpointPolicyRequestError> {
    let mut source = error.source();
    while let Some(current) = source {
        if let Some(policy) = current
            .downcast_ref::<ProviderEndpointPolicyRequestError>()
            .or_else(|| {
                current
                    .downcast_ref::<io::Error>()
                    .and_then(io::Error::get_ref)
                    .and_then(|inner| inner.downcast_ref::<ProviderEndpointPolicyRequestError>())
            })
        {
            return Some(policy);
        }
        source = current.source();
    }
    None
}

pub(super) fn reqwest_error_is_endpoint_policy(error: &reqwest::Error) -> bool {
    endpoint_policy_error(error).is_some()
}

/// Safe-to-surface detail for a connection-time DNS rejection.
///
/// Only the DNS rejection carries detail: it names the hostname (never a path,
/// query or credential) and the resolved address. Redirect rejections stay
/// generic because a redirect URL can embed secrets.
pub(super) fn reqwest_error_dns_policy_detail(error: &reqwest::Error) -> Option<&SsrfError> {
    match endpoint_policy_error(error)? {
        ProviderEndpointPolicyRequestError::DisallowedDnsAddress(detail) => Some(detail),
        _ => None,
    }
}

impl reqwest::dns::Resolve for PolicyDnsResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_owned();
        let resolver = Arc::clone(&self.resolver);
        let rules = self.rules.clone();
        Box::pin(async move {
            let lookup_host = host.clone();
            let addrs = tokio::task::spawn_blocking(move || resolver.resolve(&lookup_host))
                .await
                .map_err(io::Error::other)??;
            let safe = validate_resolved_socket_addrs(&rules, &host, addrs)?;

            Ok(Box::new(safe.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

pub(super) fn validate_resolved_socket_addrs(
    rules: &EndpointAddressRules,
    host: &str,
    addrs: Vec<SocketAddr>,
) -> io::Result<Vec<SocketAddr>> {
    if addrs.is_empty() {
        return Err(io::Error::other(
            ProviderEndpointPolicyRequestError::EmptyDnsAnswer,
        ));
    }
    let ips = addrs.iter().map(SocketAddr::ip).collect::<Vec<IpAddr>>();
    rules
        .validate_resolved_addresses(host, &ips)
        .map_err(|error| {
            io::Error::other(ProviderEndpointPolicyRequestError::DisallowedDnsAddress(
                error,
            ))
        })?;
    Ok(addrs)
}

#[cfg(test)]
pub(super) fn validate_provider_addresses(
    rules: impl Into<EndpointAddressRules>,
    addrs: Vec<SocketAddr>,
) -> io::Result<Vec<SocketAddr>> {
    validate_resolved_socket_addrs(&rules.into(), "test.invalid", addrs)
}
