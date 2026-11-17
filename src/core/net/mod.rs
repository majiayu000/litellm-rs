//! Network validation and safety utilities.

pub mod endpoint_allow;
pub mod ssrf_guard;

pub use endpoint_allow::{EndpointAllowCidrs, IpCidr};
pub use ssrf_guard::{
    EndpointAddressRules, ProviderEndpointAccess, ProviderEndpointPolicy, SsrfError,
    extract_url_host, is_private_or_reserved_host, is_private_or_reserved_ip,
    is_provider_endpoint_ip_allowed, validate_outbound_url, validate_outbound_url_str,
    validate_outbound_url_str_without_resolution, validate_outbound_url_without_resolution,
    validate_provider_endpoint_url, validate_provider_endpoint_url_str,
    validate_provider_endpoint_url_without_resolution,
};
