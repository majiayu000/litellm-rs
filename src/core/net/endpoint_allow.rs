//! Operator-configured address allow list for provider endpoints.
//!
//! Some hosts resolve every public hostname into a range the SSRF guard treats
//! as reserved, most commonly fake-IP DNS proxies (Clash, mihomo, sing-box,
//! Surge) that answer from `198.18.0.0/15`, or CGNAT egress in `100.64.0.0/10`.
//! `endpoint_allow_cidrs` lets an operator re-allow exactly those ranges for one
//! configured provider endpoint. Cloud metadata addresses stay blocked even when
//! an allow-listed range covers them; that check lives in the SSRF guard.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::net::IpAddr;
use std::str::FromStr;

use super::ssrf_guard::embedded_ipv4;

/// One IPv4 or IPv6 network in CIDR notation, e.g. `198.18.0.0/15`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IpCidr {
    network: IpAddr,
    prefix_len: u8,
}

impl IpCidr {
    /// Build a CIDR, rejecting `/0`, out-of-range prefixes, and host bits.
    pub fn new(network: IpAddr, prefix_len: u8) -> Result<Self, String> {
        let max = max_prefix_len(&network);
        if prefix_len > max {
            return Err(format!(
                "prefix length /{prefix_len} exceeds /{max} for {network}"
            ));
        }
        if prefix_len == 0 {
            return Err(
                "a /0 range would disable SSRF protection; list the specific ranges to allow"
                    .to_string(),
            );
        }
        let masked = mask(network, prefix_len);
        if masked != network {
            return Err(format!(
                "{network}/{prefix_len} has host bits set; did you mean {masked}/{prefix_len}?"
            ));
        }
        Ok(Self {
            network,
            prefix_len,
        })
    }

    pub fn network(&self) -> IpAddr {
        self.network
    }

    pub fn prefix_len(&self) -> u8 {
        self.prefix_len
    }

    /// Whether `ip` (of the same address family) falls inside this network.
    pub fn contains(&self, ip: &IpAddr) -> bool {
        match (self.network, ip) {
            (IpAddr::V4(_), IpAddr::V4(_)) | (IpAddr::V6(_), IpAddr::V6(_)) => {
                mask(*ip, self.prefix_len) == self.network
            }
            _ => false,
        }
    }
}

fn max_prefix_len(ip: &IpAddr) -> u8 {
    match ip {
        IpAddr::V4(_) => 32,
        IpAddr::V6(_) => 128,
    }
}

fn mask(ip: IpAddr, prefix_len: u8) -> IpAddr {
    match ip {
        IpAddr::V4(v4) => {
            let bits = u32::from(v4);
            let mask = u32::MAX
                .checked_shl(32 - u32::from(prefix_len))
                .unwrap_or(0);
            IpAddr::V4((bits & mask).into())
        }
        IpAddr::V6(v6) => {
            let bits = u128::from(v6);
            let mask = u128::MAX
                .checked_shl(128 - u32::from(prefix_len))
                .unwrap_or(0);
            IpAddr::V6((bits & mask).into())
        }
    }
}

impl FromStr for IpCidr {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let trimmed = value.trim();
        let invalid = |message: String| format!("invalid CIDR '{trimmed}': {message}");
        let (address, prefix_len) = match trimmed.split_once('/') {
            Some((address, prefix)) => (
                address,
                Some(prefix.parse::<u8>().map_err(|error| {
                    invalid(format!("prefix length '{prefix}' is not a number: {error}"))
                })?),
            ),
            None => (trimmed, None),
        };
        let network = address
            .parse::<IpAddr>()
            .map_err(|error| invalid(format!("'{address}' is not an IP address: {error}")))?;
        let prefix_len = prefix_len.unwrap_or_else(|| max_prefix_len(&network));
        Self::new(network, prefix_len).map_err(invalid)
    }
}

impl fmt::Display for IpCidr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.network, self.prefix_len)
    }
}

impl Serialize for IpCidr {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for IpCidr {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

/// Address ranges an operator explicitly re-allows for one provider endpoint.
///
/// Empty by default, which keeps the guard's normal public/private behaviour.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EndpointAllowCidrs(Vec<IpCidr>);

impl EndpointAllowCidrs {
    pub fn new(cidrs: impl IntoIterator<Item = IpCidr>) -> Self {
        Self(cidrs.into_iter().collect())
    }

    /// Parse CIDR strings such as `["198.18.0.0/15"]`; any invalid entry is an error.
    pub fn parse<I, S>(values: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        values
            .into_iter()
            .map(|value| value.as_ref().parse::<IpCidr>())
            .collect::<Result<Vec<_>, _>>()
            .map(Self)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_slice(&self) -> &[IpCidr] {
        &self.0
    }

    /// Whether any listed range covers `ip`, including IPv4-mapped and NAT64 forms.
    pub fn contains(&self, ip: &IpAddr) -> bool {
        let embedded = match ip {
            IpAddr::V6(v6) => embedded_ipv4(v6).map(IpAddr::V4),
            IpAddr::V4(_) => None,
        };
        self.0
            .iter()
            .any(|cidr| cidr.contains(ip) || embedded.as_ref().is_some_and(|v4| cidr.contains(v4)))
    }
}

impl FromIterator<IpCidr> for EndpointAllowCidrs {
    fn from_iter<T: IntoIterator<Item = IpCidr>>(iter: T) -> Self {
        Self::new(iter)
    }
}
