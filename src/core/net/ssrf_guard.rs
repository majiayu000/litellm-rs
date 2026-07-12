//! SSRF guard helpers for outbound URLs.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};
use std::str::FromStr;
use url::Url;

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
    InvalidUrl { url: String, message: String },
    UnsupportedScheme { scheme: String },
    MissingHost { url: String },
    PrivateOrReservedHost { host: String },
    HostResolutionFailed { host: String, message: String },
    AuthorityMismatch { expected: String, actual: String },
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
            SsrfError::PrivateOrReservedHost { host } => write!(
                f,
                "Outbound URL targets a private or reserved address '{host}', which is not allowed (SSRF protection)"
            ),
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

impl std::error::Error for SsrfError {}

/// Immutable runtime policy for one provider endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderEndpointPolicy {
    access: ProviderEndpointAccess,
    private_authority: Option<String>,
}

impl ProviderEndpointPolicy {
    pub fn public_only() -> Self {
        Self {
            access: ProviderEndpointAccess::PublicOnly,
            private_authority: None,
        }
    }

    pub fn for_base_url(
        access: ProviderEndpointAccess,
        raw_base_url: &str,
    ) -> Result<Self, SsrfError> {
        let url = Url::parse(raw_base_url).map_err(|error| SsrfError::InvalidUrl {
            url: raw_base_url.to_string(),
            message: error.to_string(),
        })?;
        validate_provider_endpoint_url_without_resolution(&url, access)?;

        Ok(Self {
            access,
            private_authority: (access == ProviderEndpointAccess::PrivateNetwork)
                .then(|| endpoint_authority(&url))
                .transpose()?,
        })
    }

    pub fn access(&self) -> ProviderEndpointAccess {
        self.access
    }

    pub fn validate_url_without_resolution(&self, url: &Url) -> Result<(), SsrfError> {
        validate_provider_endpoint_url_without_resolution(url, self.access)?;
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
    let host = host.to_ascii_lowercase();
    Ok(format!("{}://{host}:{port}", url.scheme()))
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
    validate_provider_endpoint_url_with_resolver(url, access, resolve_host_addresses)
}

/// Validate an endpoint without resolving its hostname.
pub fn validate_provider_endpoint_url_without_resolution(
    url: &Url,
    access: ProviderEndpointAccess,
) -> Result<(), SsrfError> {
    resolution_target(url, access)?;
    Ok(())
}

pub(crate) fn validate_provider_endpoint_url_with_resolver<F>(
    url: &Url,
    access: ProviderEndpointAccess,
    resolver: F,
) -> Result<(), SsrfError>
where
    F: Fn(&str, u16) -> Result<Vec<IpAddr>, SsrfError>,
{
    if let Some((host, port)) = resolution_target(url, access)? {
        let addresses = resolver(&host, port)?;
        validate_resolved_addresses(access, host, addresses)?;
    }

    Ok(())
}

fn resolution_target(
    url: &Url,
    access: ProviderEndpointAccess,
) -> Result<Option<(String, u16)>, SsrfError> {
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
        return Err(SsrfError::PrivateOrReservedHost { host });
    }

    if let Ok(ip) = host.parse::<IpAddr>() {
        if !is_provider_endpoint_ip_allowed(access, &ip) {
            return Err(SsrfError::PrivateOrReservedHost { host });
        }
        return Ok(None);
    }

    if access == ProviderEndpointAccess::PublicOnly
        && (matches!(host.as_str(), "localhost" | "internal" | "local")
            || [".localhost", ".internal", ".local"]
                .iter()
                .any(|suffix| host.ends_with(suffix)))
    {
        return Err(SsrfError::PrivateOrReservedHost { host });
    }

    Ok(Some((host, url.port_or_known_default().unwrap_or(0))))
}

fn validate_resolved_addresses(
    access: ProviderEndpointAccess,
    host: String,
    addresses: Vec<IpAddr>,
) -> Result<(), SsrfError> {
    if addresses.is_empty() {
        return Err(SsrfError::HostResolutionFailed {
            host,
            message: "no addresses returned".to_string(),
        });
    }

    if let Some(address) = addresses
        .iter()
        .find(|address| !is_provider_endpoint_ip_allowed(access, address))
    {
        return Err(SsrfError::PrivateOrReservedHost {
            host: format!("{host} ({address})"),
        });
    }

    Ok(())
}

fn is_permanently_blocked_hostname(host: &str) -> bool {
    let normalized = host.trim().trim_matches(['[', ']']).to_ascii_lowercase();
    normalized == "metadata"
        || normalized == "metadata.google.internal"
        || normalized.ends_with(".metadata.google.internal")
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

    url.host_str()
        .map(|host| host.trim_matches(['[', ']']).to_ascii_lowercase())
}

/// Returns true for private, loopback, link-local, or reserved hosts.
pub fn is_private_or_reserved_host(host: &str) -> bool {
    let normalized = host.trim().trim_matches(['[', ']']).to_ascii_lowercase();

    if normalized == "localhost"
        || normalized.ends_with(".localhost")
        || normalized == "metadata.google.internal"
        || normalized == "169.254.169.254"
    {
        return true;
    }

    if let Ok(ip) = normalized.parse::<IpAddr>() {
        return is_private_or_reserved_ip(&ip);
    }

    false
}

/// Returns true when an address is allowed by a provider endpoint access policy.
pub fn is_provider_endpoint_ip_allowed(access: ProviderEndpointAccess, ip: &IpAddr) -> bool {
    !is_metadata_ip(ip)
        && (!is_private_or_reserved_ip(ip)
            || (access == ProviderEndpointAccess::PrivateNetwork && is_private_network_ip(ip)))
}

fn is_metadata_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.octets() == [169, 254, 169, 254],
        IpAddr::V6(v6) => v6.segments() == [0xfd00, 0x0ec2, 0, 0, 0, 0, 0, 0x0254],
    }
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
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_private_or_reserved_ip(&IpAddr::V4(v4));
            }
            if segments[..6] == [0x0064, 0xff9b, 0, 0, 0, 0] {
                let v4 = Ipv4Addr::from((u32::from(segments[6]) << 16) | u32::from(segments[7]));
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    fn parse_test_url(raw_url: &str) -> Url {
        match Url::parse(raw_url) {
            Ok(url) => url,
            Err(error) => panic!("test URL should parse: {error}"),
        }
    }

    #[test]
    fn extract_url_host_parses_standard_hosts() {
        assert_eq!(
            extract_url_host("https://example.com/path"),
            Some("example.com".to_string())
        );
        assert_eq!(
            extract_url_host("http://10.0.0.1:8080/api"),
            Some("10.0.0.1".to_string())
        );
        assert_eq!(
            extract_url_host("http://[::1]:9000/api"),
            Some("::1".to_string())
        );
        assert_eq!(extract_url_host("not a url"), None);
    }

    #[test]
    fn public_ipv4_addresses_are_allowed() {
        assert!(!is_private_or_reserved_ip(&IpAddr::V4(Ipv4Addr::new(
            8, 8, 8, 8
        ))));
        assert!(!is_private_or_reserved_ip(&IpAddr::V4(Ipv4Addr::new(
            1, 1, 1, 1
        ))));
    }

    #[test]
    fn private_and_reserved_ipv4_addresses_are_rejected() {
        for ip in [
            Ipv4Addr::new(0, 0, 0, 0),
            Ipv4Addr::new(0, 0, 0, 1),
            Ipv4Addr::new(10, 1, 2, 3),
            Ipv4Addr::new(100, 64, 0, 1),
            Ipv4Addr::new(127, 0, 0, 1),
            Ipv4Addr::new(169, 254, 169, 254),
            Ipv4Addr::new(172, 20, 0, 1),
            Ipv4Addr::new(192, 168, 0, 1),
            Ipv4Addr::new(198, 18, 0, 1),
            Ipv4Addr::new(240, 0, 0, 1),
        ] {
            assert!(is_private_or_reserved_ip(&IpAddr::V4(ip)), "{ip}");
        }
    }

    #[test]
    fn private_and_reserved_ipv6_addresses_are_rejected() {
        for ip in [
            Ipv6Addr::LOCALHOST,
            Ipv6Addr::UNSPECIFIED,
            "fc00::1".parse().unwrap(),
            "fd00::1".parse().unwrap(),
            "fe80::1".parse().unwrap(),
            "fec0::1".parse().unwrap(),
            "2001:db8::1".parse().unwrap(),
            "2001:2::1".parse().unwrap(),
            "100::1".parse().unwrap(),
            "64:ff9b:1::1".parse().unwrap(),
            "3fff::1".parse().unwrap(),
            "5f00::1".parse().unwrap(),
            "::ffff:127.0.0.1".parse().unwrap(),
        ] {
            assert!(is_private_or_reserved_ip(&IpAddr::V6(ip)), "{ip}");
        }
    }

    #[test]
    fn private_hostnames_are_rejected() {
        assert!(is_private_or_reserved_host("localhost"));
        assert!(is_private_or_reserved_host("my.localhost"));
        assert!(is_private_or_reserved_host("metadata.google.internal"));
    }

    #[test]
    fn validate_outbound_url_rejects_private_targets() {
        let url = parse_test_url("http://169.254.169.254/latest/meta-data/");
        assert!(matches!(
            validate_outbound_url(&url),
            Err(SsrfError::PrivateOrReservedHost { .. })
        ));
    }

    #[test]
    fn validate_outbound_url_allows_public_targets() {
        let url = parse_test_url("https://8.8.8.8/v1");
        assert!(validate_outbound_url(&url).is_ok());
    }

    #[test]
    fn validate_outbound_url_rejects_hostname_resolving_to_private_address() {
        let url = parse_test_url("https://api.example.com/v1");

        let result = validate_provider_endpoint_url_with_resolver(
            &url,
            ProviderEndpointAccess::PublicOnly,
            |_host, _port| Ok(vec![IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))]),
        );

        assert!(matches!(
            result,
            Err(SsrfError::PrivateOrReservedHost { .. })
        ));
    }

    #[test]
    fn validate_outbound_url_allows_hostname_resolving_to_public_address() {
        let url = parse_test_url("https://api.example.com/v1");

        let result = validate_provider_endpoint_url_with_resolver(
            &url,
            ProviderEndpointAccess::PublicOnly,
            |_host, _port| Ok(vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))]),
        );

        assert!(result.is_ok());
    }

    #[test]
    fn validate_outbound_url_rejects_unresolvable_hostname() {
        let url = parse_test_url("https://api.example.com/v1");

        let result = validate_provider_endpoint_url_with_resolver(
            &url,
            ProviderEndpointAccess::PublicOnly,
            |host, _port| {
                Err(SsrfError::HostResolutionFailed {
                    host: host.to_string(),
                    message: "lookup failed".to_string(),
                })
            },
        );

        assert!(matches!(
            result,
            Err(SsrfError::HostResolutionFailed { .. })
        ));
    }

    #[test]
    fn validate_outbound_url_rejects_empty_dns_answers() {
        let url = parse_test_url("https://api.example.com/v1");

        let result = validate_provider_endpoint_url_with_resolver(
            &url,
            ProviderEndpointAccess::PublicOnly,
            |_host, _port| Ok(vec![]),
        );

        assert!(matches!(
            result,
            Err(SsrfError::HostResolutionFailed { .. })
        ));
    }

    #[test]
    fn validate_outbound_url_rejects_unsupported_scheme() {
        let url = parse_test_url("file:///tmp/socket");
        assert!(matches!(
            validate_outbound_url(&url),
            Err(SsrfError::UnsupportedScheme { .. })
        ));
    }

    #[test]
    fn endpoint_access_parsing_is_closed_and_defaults_public_only() {
        assert_eq!(
            ProviderEndpointAccess::default(),
            ProviderEndpointAccess::PublicOnly
        );
        assert_eq!(
            "private_network".parse(),
            Ok(ProviderEndpointAccess::PrivateNetwork)
        );
        assert!("".parse::<ProviderEndpointAccess>().is_err());
        assert!("private".parse::<ProviderEndpointAccess>().is_err());
    }

    #[test]
    fn private_network_allows_only_explicit_private_ranges() {
        let private = ProviderEndpointAccess::PrivateNetwork;
        for ip in [
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1)),
            IpAddr::V6("fd00::1".parse().unwrap()),
        ] {
            assert!(is_provider_endpoint_ip_allowed(private, &ip), "{ip}");
            assert!(!is_provider_endpoint_ip_allowed(
                ProviderEndpointAccess::PublicOnly,
                &ip
            ));
        }

        for ip in [
            IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254)),
            IpAddr::V4(Ipv4Addr::new(198, 18, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1)),
            IpAddr::V6("fd00:ec2::254".parse().unwrap()),
            IpAddr::V6("fe80::1".parse().unwrap()),
        ] {
            assert!(!is_provider_endpoint_ip_allowed(private, &ip), "{ip}");
        }
    }

    #[test]
    fn mixed_dns_answer_is_rejected_instead_of_partially_filtered() {
        let url = parse_test_url("https://api.example.com/v1");
        let result = validate_provider_endpoint_url_with_resolver(
            &url,
            ProviderEndpointAccess::PublicOnly,
            |_host, _port| {
                Ok(vec![
                    IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)),
                    IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                ])
            },
        );
        assert!(matches!(
            result,
            Err(SsrfError::PrivateOrReservedHost { .. })
        ));
    }

    #[test]
    fn private_policy_is_bound_to_exact_authority() {
        let policy = ProviderEndpointPolicy::for_base_url(
            ProviderEndpointAccess::PrivateNetwork,
            "http://localhost:11434/v1",
        )
        .unwrap();
        assert!(
            policy
                .validate_url_without_resolution(&parse_test_url("http://localhost:11434/api/chat"))
                .is_ok()
        );
        for mismatched in [
            "http://localhost:11435/api/chat",
            "https://localhost:11434/api/chat",
        ] {
            assert!(matches!(
                policy.validate_url_without_resolution(&parse_test_url(mismatched)),
                Err(SsrfError::AuthorityMismatch { .. })
            ));
        }
    }
}
