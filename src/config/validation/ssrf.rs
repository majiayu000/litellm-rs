//! SSRF (Server-Side Request Forgery) protection utilities
//!
//! This module provides validation functions to protect against SSRF attacks
//! by checking URLs for private/internal IP addresses and blocked hosts.

use std::net::{IpAddr, Ipv4Addr, ToSocketAddrs};
use url::Url;

/// Validate a URL against SSRF attacks
///
/// This function checks that:
/// - The URL is well-formed
/// - The host is not a private/internal IP address
/// - The host is not localhost or a loopback address
/// - The host is not a cloud metadata endpoint
pub fn validate_url_against_ssrf(url_str: &str, context: &str) -> Result<(), String> {
    let url =
        Url::parse(url_str).map_err(|e| format!("{} has invalid URL format: {}", context, e))?;

    // Ensure scheme is http or https
    match url.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(format!(
                "{} must use http:// or https:// scheme, got: {}",
                context, scheme
            ));
        }
    }

    // Get the host
    let host = url
        .host_str()
        .ok_or_else(|| format!("{} URL must have a valid host", context))?;

    // Check for localhost and other local aliases
    let host_lower = host.to_lowercase();
    let blocked_hosts = [
        "localhost",
        "127.0.0.1",
        "::1",
        "[::1]",
        "0.0.0.0",
        "0",
        // AWS metadata endpoint
        "169.254.169.254",
        // Azure metadata endpoint
        "169.254.169.254",
        // GCP metadata endpoint
        "metadata.google.internal",
        "metadata",
        // Common internal hostnames
        "internal",
        "local",
    ];

    for blocked in blocked_hosts {
        if host_lower == blocked || host_lower.ends_with(&format!(".{}", blocked)) {
            return Err(format!(
                "{} URL host '{}' is blocked for security reasons (SSRF protection)",
                context, host
            ));
        }
    }

    // Try to parse as IP address and check for private/internal ranges
    if let Ok(ip) = host.parse::<IpAddr>()
        && is_private_or_internal_ip(&ip)
    {
        return Err(format!(
            "{} URL host '{}' is a private/internal IP address (SSRF protection)",
            context, host
        ));
    }

    // Check for IP addresses in brackets (IPv6)
    if host.starts_with('[') && host.ends_with(']') {
        let ip_str = &host[1..host.len() - 1];
        if let Ok(ip) = ip_str.parse::<IpAddr>()
            && is_private_or_internal_ip(&ip)
        {
            return Err(format!(
                "{} URL host '{}' is a private/internal IP address (SSRF protection)",
                context, host
            ));
        }
    }

    // Check for decimal/octal/hex encoded IP addresses that bypass filters
    // e.g., 2130706433 = 127.0.0.1, 0x7f000001 = 127.0.0.1
    if host.chars().all(|c| c.is_ascii_digit()) {
        // Decimal encoded IP
        if let Ok(num) = host.parse::<u32>() {
            let ip = Ipv4Addr::from(num);
            if is_private_or_internal_ip(&IpAddr::V4(ip)) {
                return Err(format!(
                    "{} URL host '{}' is a decimal-encoded private IP address (SSRF protection)",
                    context, host
                ));
            }
        }
    }

    // Check for hex-encoded IP (0x prefix)
    if (host.starts_with("0x") || host.starts_with("0X"))
        && let Ok(num) = u32::from_str_radix(&host[2..], 16)
    {
        let ip = Ipv4Addr::from(num);
        if is_private_or_internal_ip(&IpAddr::V4(ip)) {
            return Err(format!(
                "{} URL host '{}' is a hex-encoded private IP address (SSRF protection)",
                context, host
            ));
        }
    }

    // Resolve DNS to ensure host does not map to private/internal IPs
    let host_is_literal = host.parse::<IpAddr>().is_ok()
        || (host.starts_with('[') && host.ends_with(']'))
        || host.chars().all(|c| c.is_ascii_digit())
        || host.starts_with("0x")
        || host.starts_with("0X");

    if !host_is_literal {
        let port = url.port_or_known_default().unwrap_or(80);
        match (host, port).to_socket_addrs() {
            Ok(addrs) => {
                for addr in addrs {
                    if is_private_or_internal_ip(&addr.ip()) {
                        return Err(format!(
                            "{} URL host '{}' resolves to a private/internal IP address (SSRF protection)",
                            context, host
                        ));
                    }
                }
            }
            Err(_) => {
                return Err(format!(
                    "{} URL host '{}' could not be resolved — unresolvable hosts are rejected (SSRF protection)",
                    context, host
                ));
            }
        }
    }

    Ok(())
}

/// Check if an IP address is private, internal, or reserved
pub(crate) fn is_private_or_internal_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            // Loopback (127.0.0.0/8)
            ipv4.is_loopback()
            // Private networks (RFC 1918)
            || ipv4.is_private()
            // Link-local (169.254.0.0/16) - includes AWS metadata endpoint
            || ipv4.is_link_local()
            // Broadcast
            || ipv4.is_broadcast()
            // Documentation (TEST-NET)
            || ipv4.is_documentation()
            // Unspecified (0.0.0.0)
            || ipv4.is_unspecified()
            // Shared address space (100.64.0.0/10) - RFC 6598
            || (ipv4.octets()[0] == 100 && (ipv4.octets()[1] & 0xC0) == 64)
            // Reserved (240.0.0.0/4)
            || ipv4.octets()[0] >= 240
        }
        IpAddr::V6(ipv6) => {
            // Loopback (::1)
            ipv6.is_loopback()
            // Unspecified (::)
            || ipv6.is_unspecified()
            // Unique local (fc00::/7)
            || ((ipv6.segments()[0] & 0xfe00) == 0xfc00)
            // Link-local (fe80::/10)
            || ((ipv6.segments()[0] & 0xffc0) == 0xfe80)
            // IPv4-mapped addresses - check the embedded IPv4
            || ipv6.to_ipv4_mapped().is_some_and(|ipv4| {
                ipv4.is_loopback() || ipv4.is_private() || ipv4.is_link_local()
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Valid Public URLs ====================

    #[test]
    fn test_valid_public_https_url() {
        let result = validate_url_against_ssrf("https://example.com/api", "API endpoint");
        assert!(result.is_ok());
    }

    #[test]
    fn test_valid_public_http_url() {
        let result = validate_url_against_ssrf("http://api.openai.com/v1", "OpenAI API");
        assert!(result.is_ok());
    }

    #[test]
    fn test_valid_url_with_port() {
        // Use a literal public IP to avoid DNS dependency in tests
        let result = validate_url_against_ssrf("https://8.8.8.8:8443/v1", "API endpoint");
        assert!(result.is_ok());
    }

    #[test]
    fn test_valid_url_with_path() {
        let result = validate_url_against_ssrf(
            "https://example.com/api/v1/chat/completions",
            "Chat endpoint",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_valid_url_with_query() {
        let result =
            validate_url_against_ssrf("https://example.com/api?key=value", "API with query");
        assert!(result.is_ok());
    }

    #[test]
    fn test_valid_subdomain() {
        // Use a literal public IP — subdomain DNS is not reliably available in all test envs
        let result = validate_url_against_ssrf("https://1.1.1.1", "Subdomain API");
        assert!(result.is_ok());
    }

    // ==================== Invalid Scheme Tests ====================

    #[test]
    fn test_invalid_ftp_scheme() {
        let result = validate_url_against_ssrf("ftp://example.com/file", "FTP endpoint");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("http:// or https://"));
    }

    #[test]
    fn test_invalid_file_scheme() {
        let result = validate_url_against_ssrf("file:///etc/passwd", "File path");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("http:// or https://"));
    }

    #[test]
    fn test_invalid_javascript_scheme() {
        let result = validate_url_against_ssrf("javascript:alert(1)", "JS");
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_data_scheme() {
        let result = validate_url_against_ssrf("data:text/html,<h1>Hi</h1>", "Data URI");
        assert!(result.is_err());
    }

    // ==================== Localhost/Loopback Tests ====================

    #[test]
    fn test_blocked_localhost() {
        let result = validate_url_against_ssrf("http://localhost/api", "Local API");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("SSRF protection"));
    }

    #[test]
    fn test_blocked_127_0_0_1() {
        let result = validate_url_against_ssrf("http://127.0.0.1/api", "Loopback API");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("SSRF protection"));
    }

    #[test]
    fn test_blocked_127_0_0_1_with_port() {
        let result = validate_url_against_ssrf("http://127.0.0.1:8080/api", "Loopback with port");
        assert!(result.is_err());
    }

    #[test]
    fn test_blocked_ipv6_loopback() {
        let result = validate_url_against_ssrf("http://[::1]/api", "IPv6 loopback");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("SSRF protection"));
    }

    #[test]
    fn test_blocked_0_0_0_0() {
        let result = validate_url_against_ssrf("http://0.0.0.0/api", "Unspecified");
        assert!(result.is_err());
    }

    // ==================== Private IP Address Tests ====================

    #[test]
    fn test_blocked_private_10_network() {
        let result = validate_url_against_ssrf("http://10.0.0.1/api", "Private 10.x");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("private/internal IP"));
    }

    #[test]
    fn test_blocked_private_172_16_network() {
        let result = validate_url_against_ssrf("http://172.16.0.1/api", "Private 172.16.x");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("private/internal IP"));
    }

    #[test]
    fn test_blocked_private_192_168_network() {
        let result = validate_url_against_ssrf("http://192.168.1.1/api", "Private 192.168.x");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("private/internal IP"));
    }

    #[test]
    fn test_blocked_private_172_31_network() {
        let result = validate_url_against_ssrf("http://172.31.255.255/api", "Private 172.31.x");
        assert!(result.is_err());
    }

    // ==================== Cloud Metadata Endpoint Tests ====================

    #[test]
    fn test_blocked_aws_metadata_endpoint() {
        let result =
            validate_url_against_ssrf("http://169.254.169.254/latest/meta-data/", "AWS metadata");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("SSRF protection"));
    }

    #[test]
    fn test_blocked_link_local_ip() {
        let result = validate_url_against_ssrf("http://169.254.1.1/api", "Link local");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("private/internal IP"));
    }

    #[test]
    fn test_blocked_gcp_metadata_hostname() {
        let result =
            validate_url_against_ssrf("http://metadata.google.internal/v1/", "GCP metadata");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("SSRF protection"));
    }

    #[test]
    fn test_blocked_metadata_hostname() {
        let result = validate_url_against_ssrf("http://metadata/v1/", "Metadata shortname");
        assert!(result.is_err());
    }

    // ==================== Decimal/Octal/Hex Encoded IP Tests ====================

    #[test]
    fn test_blocked_decimal_encoded_loopback() {
        // 2130706433 = 127.0.0.1
        // Note: URL parser resolves this to 127.0.0.1 before our check runs
        let result = validate_url_against_ssrf("http://2130706433/api", "Decimal encoded");
        assert!(result.is_err());
        let err = result.unwrap_err();
        // The URL parser resolves to 127.0.0.1, so it gets blocked by the host check
        assert!(
            err.contains("SSRF protection") || err.contains("private/internal IP"),
            "Expected SSRF error, got: {}",
            err
        );
    }

    #[test]
    fn test_blocked_hex_encoded_loopback() {
        // 0x7f000001 = 127.0.0.1
        // Note: URL parser resolves this to 127.0.0.1 before our check runs
        let result = validate_url_against_ssrf("http://0x7f000001/api", "Hex encoded");
        assert!(result.is_err());
        let err = result.unwrap_err();
        // The URL parser resolves to 127.0.0.1, so it gets blocked by the host check
        assert!(
            err.contains("SSRF protection") || err.contains("private/internal IP"),
            "Expected SSRF error, got: {}",
            err
        );
    }

    #[test]
    fn test_blocked_hex_encoded_private() {
        // 0x0a000001 = 10.0.0.1
        let result = validate_url_against_ssrf("http://0x0a000001/api", "Hex private");
        assert!(result.is_err());
    }

    // ==================== IPv6 Private Address Tests ====================

    #[test]
    fn test_blocked_ipv6_unique_local() {
        // fc00::/7 - unique local addresses
        let result = validate_url_against_ssrf("http://[fc00::1]/api", "IPv6 unique local");
        assert!(result.is_err());
    }

    #[test]
    fn test_blocked_ipv6_link_local() {
        // fe80::/10 - link local addresses
        let result = validate_url_against_ssrf("http://[fe80::1]/api", "IPv6 link local");
        assert!(result.is_err());
    }

    // ==================== Reserved IP Range Tests ====================

    #[test]
    fn test_blocked_reserved_240_range() {
        let result = validate_url_against_ssrf("http://240.0.0.1/api", "Reserved 240.x");
        assert!(result.is_err());
    }

    #[test]
    fn test_blocked_reserved_255_range() {
        let result = validate_url_against_ssrf("http://255.255.255.255/api", "Broadcast");
        assert!(result.is_err());
    }

    #[test]
    fn test_blocked_shared_address_space() {
        // 100.64.0.0/10 - carrier-grade NAT (RFC 6598)
        let result = validate_url_against_ssrf("http://100.64.0.1/api", "CGN address");
        assert!(result.is_err());
    }

    // ==================== Malformed URL Tests ====================

    #[test]
    fn test_invalid_url_format() {
        let result = validate_url_against_ssrf("not-a-valid-url", "Invalid URL");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid URL format"));
    }

    #[test]
    fn test_empty_url() {
        let result = validate_url_against_ssrf("", "Empty URL");
        assert!(result.is_err());
    }

    #[test]
    fn test_url_without_host() {
        // Some URL parsers allow http:/// with empty host
        // Test with a clearly invalid URL that has no host
        let result = validate_url_against_ssrf("http:///path", "No host");
        // This may either fail parsing or be treated as empty host
        // Either way, if it passes, it should still block "" as a host
        if result.is_ok() {
            // If URL parser accepted it, the function should still work
            // on whatever host it extracted (likely empty or /)
        }
        // This test is just checking the function doesn't panic
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_localhost_with_subdomain_blocked() {
        // Subdomains of blocked hosts should also be blocked
        let result =
            validate_url_against_ssrf("http://sub.localhost/api", "Subdomain of localhost");
        assert!(result.is_err());
    }

    #[test]
    fn test_internal_hostname_blocked() {
        let result = validate_url_against_ssrf("http://internal/api", "Internal hostname");
        assert!(result.is_err());
    }

    #[test]
    fn test_local_hostname_blocked() {
        let result = validate_url_against_ssrf("http://local/api", "Local hostname");
        assert!(result.is_err());
    }

    #[test]
    fn test_subdomain_of_internal_blocked() {
        let result = validate_url_against_ssrf("http://api.internal/v1", "Subdomain of internal");
        assert!(result.is_err());
    }

    #[test]
    fn test_valid_external_ip() {
        // 8.8.8.8 is Google's public DNS - should be allowed
        let result = validate_url_against_ssrf("http://8.8.8.8/api", "Public IP");
        assert!(result.is_ok());
    }

    #[test]
    fn test_valid_external_ip_2() {
        // 1.1.1.1 is Cloudflare's public DNS - should be allowed
        let result = validate_url_against_ssrf("http://1.1.1.1/api", "Cloudflare DNS");
        assert!(result.is_ok());
    }

    // ==================== Context Message Tests ====================

    #[test]
    fn test_context_in_error_message() {
        let result = validate_url_against_ssrf("http://localhost/api", "Webhook URL");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Webhook URL"));
    }

    #[test]
    fn test_context_in_scheme_error() {
        let result = validate_url_against_ssrf("ftp://example.com/file", "Callback endpoint");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Callback endpoint"));
    }

    // ==================== is_private_or_internal_ip Tests ====================

    #[test]
    fn test_is_private_loopback_v4() {
        let ip = "127.0.0.1".parse().unwrap();
        assert!(is_private_or_internal_ip(&ip));
    }

    #[test]
    fn test_is_private_loopback_v6() {
        let ip = "::1".parse().unwrap();
        assert!(is_private_or_internal_ip(&ip));
    }

    #[test]
    fn test_is_private_10_network() {
        let ip = "10.255.255.255".parse().unwrap();
        assert!(is_private_or_internal_ip(&ip));
    }

    #[test]
    fn test_is_private_172_16_network() {
        let ip = "172.16.0.0".parse().unwrap();
        assert!(is_private_or_internal_ip(&ip));
    }

    #[test]
    fn test_is_private_172_31_network() {
        let ip = "172.31.255.255".parse().unwrap();
        assert!(is_private_or_internal_ip(&ip));
    }

    #[test]
    fn test_is_private_192_168_network() {
        let ip = "192.168.0.0".parse().unwrap();
        assert!(is_private_or_internal_ip(&ip));
    }

    #[test]
    fn test_is_private_link_local() {
        let ip = "169.254.169.254".parse().unwrap();
        assert!(is_private_or_internal_ip(&ip));
    }

    #[test]
    fn test_is_public_ip() {
        let ip = "8.8.8.8".parse().unwrap();
        assert!(!is_private_or_internal_ip(&ip));
    }

    #[test]
    fn test_is_public_ip_2() {
        let ip = "93.184.216.34".parse().unwrap(); // example.com
        assert!(!is_private_or_internal_ip(&ip));
    }

    #[test]
    fn test_is_private_broadcast() {
        let ip = "255.255.255.255".parse().unwrap();
        assert!(is_private_or_internal_ip(&ip));
    }

    #[test]
    fn test_is_private_unspecified_v4() {
        let ip = "0.0.0.0".parse().unwrap();
        assert!(is_private_or_internal_ip(&ip));
    }

    #[test]
    fn test_is_private_unspecified_v6() {
        let ip = "::".parse().unwrap();
        assert!(is_private_or_internal_ip(&ip));
    }

    #[test]
    fn test_is_private_ipv6_unique_local() {
        let ip = "fc00::1".parse().unwrap();
        assert!(is_private_or_internal_ip(&ip));
    }

    #[test]
    fn test_is_private_ipv6_link_local() {
        let ip = "fe80::1".parse().unwrap();
        assert!(is_private_or_internal_ip(&ip));
    }

    #[test]
    fn test_is_public_ipv6() {
        let ip = "2607:f8b0:4004:800::200e".parse().unwrap(); // Google's IPv6
        assert!(!is_private_or_internal_ip(&ip));
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_real_world_api_endpoints() {
        // Test common real-world API endpoints that should be allowed
        let valid_endpoints = vec![
            "https://api.openai.com/v1/chat/completions",
            "https://api.anthropic.com/v1/messages",
            "https://generativelanguage.googleapis.com/v1/models",
            "https://api.cohere.ai/v1/generate",
        ];

        for endpoint in valid_endpoints {
            let result = validate_url_against_ssrf(endpoint, "API endpoint");
            assert!(result.is_ok(), "Expected {} to be valid", endpoint);
        }
    }

    #[test]
    fn test_ssrf_attack_vectors() {
        // Test common SSRF attack vectors that should be blocked
        let attack_vectors = vec![
            "http://localhost/admin",
            "http://127.0.0.1/admin",
            "http://[::1]/admin",
            "http://169.254.169.254/latest/meta-data/",
            "http://10.0.0.1/internal",
            "http://192.168.1.1/router",
            "http://2130706433/decimal-bypass",
            "http://0x7f000001/hex-bypass",
        ];

        for vector in attack_vectors {
            let result = validate_url_against_ssrf(vector, "Attack vector");
            assert!(result.is_err(), "Expected {} to be blocked", vector);
        }
    }
}
