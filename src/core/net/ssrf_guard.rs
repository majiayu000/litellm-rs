//! SSRF guard helpers for outbound URLs.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};
use std::str::FromStr;
use url::Url;

use super::endpoint_allow::EndpointAllowCidrs;

/// Network scope allowed for a configured provider endpoint.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderEndpointAccess {
    /// Only globally routable addresses are allowed.
    #[default]
    PublicOnly,
    /// The configured authority may also resolve to loopback, RFC1918, or IPv6 ULA.
    PrivateNetwork,
}

impl fmt::Display for ProviderEndpointAccess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PublicOnly => f.write_str("public_only"),
            Self::PrivateNetwork => f.write_str("private_network"),
        }
    }
}

impl FromStr for ProviderEndpointAccess {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "public_only" => Ok(Self::PublicOnly),
            "private_network" => Ok(Self::PrivateNetwork),
            _ => Err(format!(
                "endpoint access must be 'public_only' or 'private_network', got '{value}'"
            )),
        }
    }
}

/// Error returned when an outbound URL is not safe to use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SsrfError {
    InvalidUrl {
        url: String,
        message: String,
    },
    UnsupportedScheme {
        scheme: String,
    },
    MissingHost {
        url: String,
    },
    /// `address` is the rejected IP (literal or resolved); `None` for blocked hostnames.
    PrivateOrReservedHost {
        host: String,
        address: Option<IpAddr>,
    },
    HostResolutionFailed {
        host: String,
        message: String,
    },
    AuthorityMismatch {
        expected: String,
        actual: String,
    },
}

impl fmt::Display for SsrfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SsrfError::InvalidUrl { url, message } => {
                write!(f, "Outbound URL '{url}' is invalid: {message}")
            }
            SsrfError::UnsupportedScheme { scheme } => {
                write!(f, "Outbound URL scheme '{scheme}' is not allowed")
            }
            SsrfError::MissingHost { url } => {
                write!(f, "Outbound URL has an invalid or missing host: {url}")
            }
            SsrfError::PrivateOrReservedHost {
                host,
                address: None,
            } => write!(
                f,
                "Outbound URL targets a private or reserved address '{host}', which is not allowed (SSRF protection)"
            ),
            SsrfError::PrivateOrReservedHost {
                host,
                address: Some(address),
            } => write_disallowed_address(f, host, address),
            SsrfError::HostResolutionFailed { host, message } => write!(
                f,
                "Outbound URL host '{host}' could not be resolved: {message} (SSRF protection)"
            ),
            SsrfError::AuthorityMismatch { expected, actual } => write!(
                f,
                "Outbound URL authority '{actual}' does not match the private-network authority '{expected}' (SSRF protection)"
            ),
        }
    }
}

/// Name the rejected address. Which setting would allow it depends on the
/// caller, so provider-endpoint callers append [`SsrfError::provider_setting_hint`].
fn write_disallowed_address(
    f: &mut fmt::Formatter<'_>,
    host: &str,
    address: &IpAddr,
) -> fmt::Result {
    let target = if host.parse::<IpAddr>().ok() == Some(*address) {
        format!("'{host}'")
    } else {
        format!("'{host}' (resolved to {address})")
    };
    if is_metadata_ip(address) {
        return write!(
            f,
            "Outbound URL targets a private or reserved address {target}; cloud metadata addresses are always blocked (SSRF protection)"
        );
    }
    if is_link_local_ip(address) {
        return write!(
            f,
            "Outbound URL targets a private or reserved address {target}; link-local addresses are always blocked (SSRF protection)"
        );
    }
    write!(
        f,
        "Outbound URL targets a private or reserved address {target}, which is not allowed (SSRF protection)"
    )
}

impl SsrfError {
    /// The concrete address the guard rejected, if it rejected one.
    pub fn rejected_address(&self) -> Option<IpAddr> {
        match self {
            SsrfError::PrivateOrReservedHost { address, .. } => *address,
            _ => None,
        }
    }

    /// For a configured provider endpoint: the setting that would allow the
    /// rejected address. `None` when no setting can (metadata, link-local,
    /// blocked names).
    pub fn provider_setting_hint(&self) -> Option<String> {
        self.provider_setting_hint_for(true)
    }

    /// Like [`Self::provider_setting_hint`], for a caller that knows whether the
    /// provider type honours `endpoint_allow_cidrs`.
    pub fn provider_setting_hint_for(&self, allow_cidrs_supported: bool) -> Option<String> {
        let address = self
            .rejected_address()
            .filter(|ip| !is_never_allow_listed_ip(ip))?;
        let mut hint = if allow_cidrs_supported {
            format!(
                "to allow it for this provider, add a CIDR covering {address} to the provider's `endpoint_allow_cidrs`"
            )
        } else {
            format!(
                "this provider type does not support `endpoint_allow_cidrs`; to allow {address}, configure the endpoint as `provider_type: openai_compatible` with an `endpoint_allow_cidrs` entry covering it"
            )
        };
        if is_private_network_ip(&address) {
            hint.push_str(" or set `endpoint_access: private_network`");
        }
        Some(hint)
    }

    /// `Display` plus [`Self::provider_setting_hint`] when there is one.
    pub fn with_provider_setting_hint(&self) -> String {
        self.with_provider_setting_hint_for(true)
    }

    /// `Display` plus [`Self::provider_setting_hint_for`] when there is one.
    pub fn with_provider_setting_hint_for(&self, allow_cidrs_supported: bool) -> String {
        match self.provider_setting_hint_for(allow_cidrs_supported) {
            Some(hint) => format!("{self}; {hint}"),
            None => self.to_string(),
        }
    }
}

impl std::error::Error for SsrfError {}

/// Address rules for one provider endpoint: its access level plus any
/// operator-configured `endpoint_allow_cidrs`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct EndpointAddressRules {
    access: ProviderEndpointAccess,
    allow_cidrs: EndpointAllowCidrs,
}

impl From<ProviderEndpointAccess> for EndpointAddressRules {
    fn from(access: ProviderEndpointAccess) -> Self {
        Self::new(access, EndpointAllowCidrs::default())
    }
}

impl EndpointAddressRules {
    pub fn new(access: ProviderEndpointAccess, allow_cidrs: EndpointAllowCidrs) -> Self {
        Self {
            access,
            allow_cidrs,
        }
    }

    pub fn access(&self) -> ProviderEndpointAccess {
        self.access
    }

    pub fn allow_cidrs(&self) -> &EndpointAllowCidrs {
        &self.allow_cidrs
    }

    /// Whether `ip` may be dialed. Cloud metadata and link-local addresses are
    /// rejected even when an allow-listed CIDR covers them.
    pub fn allows_ip(&self, ip: &IpAddr) -> bool {
        is_provider_endpoint_ip_allowed(self.access, ip)
            || (!is_never_allow_listed_ip(ip) && self.allow_cidrs.contains(ip))
    }

    /// Reject the whole DNS answer if any address is disallowed.
    pub fn validate_resolved_addresses(
        &self,
        host: &str,
        addresses: &[IpAddr],
    ) -> Result<(), SsrfError> {
        if addresses.is_empty() {
            return Err(SsrfError::HostResolutionFailed {
                host: host.to_string(),
                message: "no addresses returned".to_string(),
            });
        }
        if let Some(address) = addresses.iter().find(|address| !self.allows_ip(address)) {
            return Err(SsrfError::PrivateOrReservedHost {
                host: host.to_string(),
                address: Some(*address),
            });
        }
        Ok(())
    }

    /// Validate an endpoint and resolve its hostname using the system resolver.
    pub fn validate_url(&self, url: &Url) -> Result<(), SsrfError> {
        self.validate_url_with_resolver(url, resolve_host_addresses)
    }

    /// Validate an endpoint without resolving its hostname.
    pub fn validate_url_without_resolution(&self, url: &Url) -> Result<(), SsrfError> {
        self.resolution_target(url).map(drop)
    }

    pub(crate) fn validate_url_with_resolver<F>(
        &self,
        url: &Url,
        resolver: F,
    ) -> Result<(), SsrfError>
    where
        F: Fn(&str, u16) -> Result<Vec<IpAddr>, SsrfError>,
    {
        if let Some((host, port)) = self.resolution_target(url)? {
            let addresses = resolver(&host, port)?;
            self.validate_resolved_addresses(&host, &addresses)?;
        }
        Ok(())
    }

    fn resolution_target(&self, url: &Url) -> Result<Option<(String, u16)>, SsrfError> {
        match url.scheme() {
            "http" | "https" | "ws" | "wss" => {}
            scheme => {
                return Err(SsrfError::UnsupportedScheme {
                    scheme: scheme.to_string(),
                });
            }
        }

        let host = extract_url_host(url.as_str()).ok_or_else(|| SsrfError::MissingHost {
            url: url.to_string(),
        })?;

        if is_permanently_blocked_hostname(&host) {
            return Err(SsrfError::PrivateOrReservedHost {
                host,
                address: None,
            });
        }

        if let Ok(ip) = host.parse::<IpAddr>() {
            if !self.allows_ip(&ip) {
                return Err(SsrfError::PrivateOrReservedHost {
                    host,
                    address: Some(ip),
                });
            }
            return Ok(None);
        }

        if self.access == ProviderEndpointAccess::PublicOnly
            && (matches!(host.as_str(), "localhost" | "internal" | "local")
                || [".localhost", ".internal", ".local"]
                    .iter()
                    .any(|suffix| host.ends_with(suffix)))
        {
            return Err(SsrfError::PrivateOrReservedHost {
                host,
                address: None,
            });
        }

        Ok(Some((host, url.port_or_known_default().unwrap_or(0))))
    }
}

/// Immutable runtime policy for one provider endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderEndpointPolicy {
    rules: EndpointAddressRules,
    private_authority: Option<String>,
}

impl ProviderEndpointPolicy {
    pub fn public_only() -> Self {
        Self {
            rules: EndpointAddressRules::default(),
            private_authority: None,
        }
    }

    /// Public-only policy for a provider without a configured base URL that
    /// still honours its `endpoint_allow_cidrs`.
    pub fn public_only_with_allow_cidrs(allow_cidrs: EndpointAllowCidrs) -> Self {
        Self {
            rules: EndpointAddressRules::new(ProviderEndpointAccess::PublicOnly, allow_cidrs),
            private_authority: None,
        }
    }

    pub fn for_base_url(
        access: ProviderEndpointAccess,
        raw_base_url: &str,
    ) -> Result<Self, SsrfError> {
        Self::for_base_url_with_rules(access.into(), raw_base_url)
    }

    /// Bind access level and `endpoint_allow_cidrs` to one configured base URL.
    pub fn for_base_url_with_rules(
        rules: EndpointAddressRules,
        raw_base_url: &str,
    ) -> Result<Self, SsrfError> {
        let url = Url::parse(raw_base_url).map_err(|error| SsrfError::InvalidUrl {
            url: raw_base_url.to_string(),
            message: error.to_string(),
        })?;
        rules.validate_url_without_resolution(&url)?;

        let private_authority = (rules.access == ProviderEndpointAccess::PrivateNetwork)
            .then(|| endpoint_authority(&url))
            .transpose()?;
        Ok(Self {
            rules,
            private_authority,
        })
    }

    pub fn access(&self) -> ProviderEndpointAccess {
        self.rules.access
    }

    pub fn rules(&self) -> &EndpointAddressRules {
        &self.rules
    }

    pub fn validate_url_without_resolution(&self, url: &Url) -> Result<(), SsrfError> {
        self.rules.validate_url_without_resolution(url)?;
        self.validate_private_authority(url)
    }

    fn validate_private_authority(&self, url: &Url) -> Result<(), SsrfError> {
        let Some(expected) = &self.private_authority else {
            return Ok(());
        };
        let actual = endpoint_authority(url)?;
        if actual != *expected {
            return Err(SsrfError::AuthorityMismatch {
                expected: expected.clone(),
                actual,
            });
        }
        Ok(())
    }
}

fn endpoint_authority(url: &Url) -> Result<String, SsrfError> {
    let host = url.host_str().ok_or_else(|| SsrfError::MissingHost {
        url: url.to_string(),
    })?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| SsrfError::MissingHost {
            url: url.to_string(),
        })?;
    let host = normalize_hostname(host);
    Ok(format!("{}://{host}:{port}", url.scheme()))
}

fn normalize_hostname(host: &str) -> String {
    host.trim()
        .trim_matches(['[', ']'])
        .trim_end_matches('.')
        .to_ascii_lowercase()
}

/// Parse and validate an outbound URL string.
pub fn validate_outbound_url_str(raw_url: &str) -> Result<Url, SsrfError> {
    validate_provider_endpoint_url_str(raw_url, ProviderEndpointAccess::PublicOnly)
}

/// Parse and validate an outbound URL string without doing DNS resolution.
pub fn validate_outbound_url_str_without_resolution(raw_url: &str) -> Result<Url, SsrfError> {
    let url = Url::parse(raw_url).map_err(|error| SsrfError::InvalidUrl {
        url: raw_url.to_string(),
        message: error.to_string(),
    })?;

    validate_outbound_url_without_resolution(&url)?;
    Ok(url)
}

/// Validate an already parsed outbound URL.
pub fn validate_outbound_url(url: &Url) -> Result<(), SsrfError> {
    validate_provider_endpoint_url(url, ProviderEndpointAccess::PublicOnly)
}

/// Validate an already parsed outbound URL without doing DNS resolution.
pub fn validate_outbound_url_without_resolution(url: &Url) -> Result<(), SsrfError> {
    validate_provider_endpoint_url_without_resolution(url, ProviderEndpointAccess::PublicOnly)
}

/// Parse and validate a configured provider endpoint using its declared access policy.
pub fn validate_provider_endpoint_url_str(
    raw_url: &str,
    access: ProviderEndpointAccess,
) -> Result<Url, SsrfError> {
    let url = Url::parse(raw_url).map_err(|error| SsrfError::InvalidUrl {
        url: raw_url.to_string(),
        message: error.to_string(),
    })?;
    validate_provider_endpoint_url(&url, access)?;
    Ok(url)
}

/// Validate an endpoint and resolve its hostname using the system resolver.
pub fn validate_provider_endpoint_url(
    url: &Url,
    access: ProviderEndpointAccess,
) -> Result<(), SsrfError> {
    EndpointAddressRules::from(access).validate_url(url)
}

/// Validate an endpoint without resolving its hostname.
pub fn validate_provider_endpoint_url_without_resolution(
    url: &Url,
    access: ProviderEndpointAccess,
) -> Result<(), SsrfError> {
    EndpointAddressRules::from(access).validate_url_without_resolution(url)
}

#[cfg(test)]
pub(crate) fn validate_provider_endpoint_url_with_resolver<F>(
    url: &Url,
    access: ProviderEndpointAccess,
    resolver: F,
) -> Result<(), SsrfError>
where
    F: Fn(&str, u16) -> Result<Vec<IpAddr>, SsrfError>,
{
    EndpointAddressRules::from(access).validate_url_with_resolver(url, resolver)
}

fn is_permanently_blocked_hostname(host: &str) -> bool {
    let normalized = normalize_hostname(host);
    normalized == "metadata"
        || normalized == "metadata.google.internal"
        || normalized.ends_with(".metadata.google.internal")
        || normalized == "metadata.goog"
        || normalized.ends_with(".metadata.goog")
}

fn resolve_host_addresses(host: &str, port: u16) -> Result<Vec<IpAddr>, SsrfError> {
    (host, port)
        .to_socket_addrs()
        .map(|addresses| addresses.map(|address| address.ip()).collect())
        .map_err(|error| SsrfError::HostResolutionFailed {
            host: host.to_string(),
            message: error.to_string(),
        })
}

/// Extract the lowercase host portion from a URL string.
pub fn extract_url_host(raw_url: &str) -> Option<String> {
    let url = Url::parse(raw_url).ok()?;
    if !matches!(url.scheme(), "http" | "https" | "ws" | "wss") {
        return None;
    }

    url.host_str().map(normalize_hostname)
}

/// Returns true for private, loopback, link-local, or reserved hosts.
pub fn is_private_or_reserved_host(host: &str) -> bool {
    let normalized = normalize_hostname(host);

    if normalized == "localhost"
        || normalized.ends_with(".localhost")
        || is_permanently_blocked_hostname(&normalized)
    {
        return true;
    }

    if let Ok(ip) = normalized.parse::<IpAddr>() {
        return !is_provider_endpoint_ip_allowed(ProviderEndpointAccess::PublicOnly, &ip);
    }

    false
}

/// Returns true when an address is allowed by a provider endpoint access policy.
pub fn is_provider_endpoint_ip_allowed(access: ProviderEndpointAccess, ip: &IpAddr) -> bool {
    !is_metadata_ip(ip)
        && (!is_private_or_reserved_ip(ip)
            || (access == ProviderEndpointAccess::PrivateNetwork && is_private_network_ip(ip)))
}

/// Cloud metadata and credential endpoints, blocked under every setting:
/// the common IMDS address, Azure WireServer, Alibaba Cloud ECS metadata,
/// AWS ECS task credentials, EKS Pod Identity (v4 + v6), AWS IMDS v6 and
/// OCI IMDS v6, plus their IPv4-mapped and NAT64 forms.
fn is_metadata_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => matches!(
            v4.octets(),
            [169, 254, 169, 254]
                | [168, 63, 129, 16]
                | [100, 100, 100, 200]
                | [169, 254, 170, 2]
                | [169, 254, 170, 23]
        ),
        IpAddr::V6(v6) => {
            matches!(
                v6.segments(),
                [0xfd00, 0x0ec2, 0, 0, 0, 0, 0, 0x0254 | 0x0023]
                    | [0xfd00, 0x00c1, 0, 0, 0, 0, 0xa9fe, 0xa9fe]
            ) || embedded_ipv4(v6).is_some_and(|v4| is_metadata_ip(&IpAddr::V4(v4)))
        }
    }
}

/// Link-local ranges (`169.254.0.0/16`, `fe80::/10`, and the IPv4-mapped and
/// NAT64 forms of the former) host per-cloud metadata and credential agents at
/// addresses beyond [`is_metadata_ip`]'s list, and no fake-IP or CGNAT setup
/// answers from them, so `endpoint_allow_cidrs` never re-admits them.
pub(super) fn is_link_local_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_link_local(),
        IpAddr::V6(v6) => {
            (v6.segments()[0] & 0xffc0) == 0xfe80
                || embedded_ipv4(v6).is_some_and(|v4| v4.is_link_local())
        }
    }
}

/// Addresses that no `endpoint_allow_cidrs` entry can re-admit.
fn is_never_allow_listed_ip(ip: &IpAddr) -> bool {
    is_metadata_ip(ip) || is_link_local_ip(ip)
}

pub(super) fn embedded_ipv4(ip: &Ipv6Addr) -> Option<Ipv4Addr> {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return Some(v4);
    }
    let segments = ip.segments();
    (segments[..6] == [0x0064, 0xff9b, 0, 0, 0, 0])
        .then(|| Ipv4Addr::from((u32::from(segments[6]) << 16) | u32::from(segments[7])))
}

fn is_private_network_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            v4.is_loopback()
                || octets[0] == 10
                || (octets[0] == 172 && (16..=31).contains(&octets[1]))
                || (octets[0] == 192 && octets[1] == 168)
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || v6
                    .to_ipv4_mapped()
                    .is_some_and(|v4| is_private_network_ip(&IpAddr::V4(v4)))
        }
    }
}

fn is_non_global_ietf_protocol_assignment(ip: &Ipv6Addr) -> bool {
    let segments = ip.segments();
    if segments[0] != 0x2001 || segments[1] >= 0x0200 {
        return false;
    }

    let raw = u128::from_be_bytes(ip.octets());
    let anycast = raw.wrapping_sub(0x2001_0001_0000_0000_0000_0000_0000_0001) <= 2;
    let globally_reachable = anycast
        || segments[1] == 0x0003
        || (segments[1] == 0x0004 && segments[2] == 0x0112)
        || (0x0020..=0x003f).contains(&segments[1]);
    !globally_reachable
}

/// Check whether a parsed IP address falls within private or reserved ranges.
pub fn is_private_or_reserved_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();

            octets[0] == 0
                || octets[0] == 10
                || octets[0] == 127
                || (octets[0] == 100 && (64..=127).contains(&octets[1]))
                || (octets[0] == 169 && octets[1] == 254)
                || (octets[0] == 172 && (16..=31).contains(&octets[1]))
                || (octets[0] == 192 && octets[1] == 168)
                || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
                || (octets[0] == 192 && octets[1] == 0 && octets[2] == 2)
                || (octets[0] == 198 && (18..=19).contains(&octets[1]))
                || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
                || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113)
                || (octets[0] == 192 && octets[1] == 88 && octets[2] == 99)
                || octets[0] >= 240
                || v4.is_broadcast()
                || v4.is_multicast()
        }
        IpAddr::V6(v6) => {
            if v6.is_loopback() || v6.is_unspecified() || v6.is_multicast() {
                return true;
            }

            let segments = v6.segments();
            if let Some(v4) = embedded_ipv4(v6) {
                return is_private_or_reserved_ip(&IpAddr::V4(v4));
            }

            segments[..6] == [0, 0, 0, 0, 0, 0]
                || (segments[0] == 0x0064 && segments[1] == 0xff9b && segments[2] == 1)
                || segments[..4] == [0x0100, 0, 0, 0]
                || segments[..4] == [0x0100, 0, 0, 1]
                || is_non_global_ietf_protocol_assignment(v6)
                || (segments[0] == 0x2001 && segments[1] == 0x0db8)
                || segments[0] == 0x2002
                || (segments[0] == 0x3fff && segments[1] & 0xf000 == 0)
                || segments[0] == 0x5f00
                || (segments[0] & 0xfe00) == 0xfc00
                || (segments[0] & 0xffc0) == 0xfe80
                || (segments[0] & 0xffc0) == 0xfec0
                || (segments[0] & 0xe000) != 0x2000
        }
    }
}

#[cfg(test)]
#[path = "ssrf_guard_tests.rs"]
mod tests;
