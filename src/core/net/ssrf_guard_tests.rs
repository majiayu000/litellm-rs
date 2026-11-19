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
        "1::1".parse().unwrap(),
        "4000::1".parse().unwrap(),
        "5f00::1".parse().unwrap(),
        "fe00::1".parse().unwrap(),
        "::ffff:127.0.0.1".parse().unwrap(),
    ] {
        assert!(is_private_or_reserved_ip(&IpAddr::V6(ip)), "{ip}");
    }
}

#[test]
fn public_ipv6_and_nat64_require_a_globally_routable_destination() {
    for ip in ["2606:4700:4700::1111", "64:ff9b::808:808"] {
        assert!(!is_private_or_reserved_ip(&ip.parse().unwrap()), "{ip}");
    }
    assert!(is_private_or_reserved_ip(
        &"64:ff9b::a00:1".parse().unwrap()
    ));
}

#[test]
fn private_hostnames_are_rejected() {
    assert!(is_private_or_reserved_host("localhost"));
    assert!(is_private_or_reserved_host("my.localhost"));
    assert!(is_private_or_reserved_host("metadata.google.internal"));
    assert!(is_private_or_reserved_host("metadata.google.internal."));
    assert!(is_private_or_reserved_host("metadata.goog"));
    assert!(is_private_or_reserved_host("metadata.goog."));
    assert!(is_private_or_reserved_host("api.metadata.goog"));
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
        IpAddr::V4(Ipv4Addr::new(168, 63, 129, 16)),
        IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254)),
        IpAddr::V4(Ipv4Addr::new(198, 18, 0, 1)),
        IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1)),
        IpAddr::V6("fd00:ec2::254".parse().unwrap()),
        IpAddr::V6("::ffff:168.63.129.16".parse().unwrap()),
        IpAddr::V6("64:ff9b::a83f:8110".parse().unwrap()),
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
    assert!(
        policy
            .validate_url_without_resolution(&parse_test_url("http://localhost.:11434/api/chat"))
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

// ---- #1503: operator allow list for fake-IP DNS and CGNAT ----

fn rules_with(access: ProviderEndpointAccess, cidrs: &[&str]) -> EndpointAddressRules {
    let cidrs = EndpointAllowCidrs::parse(cidrs).expect("test CIDRs should parse");
    EndpointAddressRules::new(access, cidrs)
}

fn resolve_to(rules: &EndpointAddressRules, url: &str, answer: IpAddr) -> Result<(), SsrfError> {
    rules.validate_url_with_resolver(&parse_test_url(url), |_host, _port| Ok(vec![answer]))
}

const FAKE_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(198, 18, 0, 7));
const CGNAT: IpAddr = IpAddr::V4(Ipv4Addr::new(100, 64, 3, 9));

#[test]
fn fake_ip_and_cgnat_answers_stay_rejected_by_default() {
    for access in [
        ProviderEndpointAccess::PublicOnly,
        ProviderEndpointAccess::PrivateNetwork,
    ] {
        let rules = EndpointAddressRules::from(access);
        for answer in [FAKE_IP, CGNAT, IpAddr::V4(Ipv4Addr::new(198, 19, 255, 254))] {
            let error = resolve_to(&rules, "https://api.provider.test/v1", answer)
                .expect_err("reserved answer must be rejected without an opt-in");
            assert_eq!(error.rejected_address(), Some(answer), "{access:?}");
        }
    }
}

#[test]
fn allow_list_admits_exactly_the_listed_ranges() {
    let fake_ip = rules_with(ProviderEndpointAccess::PublicOnly, &["198.18.0.0/15"]);
    assert!(resolve_to(&fake_ip, "https://api.provider.test/v1", FAKE_IP).is_ok());
    assert!(
        resolve_to(
            &fake_ip,
            "https://api.provider.test/v1",
            IpAddr::V4(Ipv4Addr::new(198, 19, 255, 254))
        )
        .is_ok()
    );
    // Narrow: CGNAT, RFC 1918 and other reserved ranges (TEST-NET-2) stay rejected.
    for answer in [
        CGNAT,
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)),
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
    ] {
        assert!(
            resolve_to(&fake_ip, "https://api.provider.test/v1", answer).is_err(),
            "{answer}"
        );
    }

    let cgnat = rules_with(ProviderEndpointAccess::PublicOnly, &["100.64.0.0/10"]);
    assert!(resolve_to(&cgnat, "https://api.provider.test/v1", CGNAT).is_ok());
    assert!(resolve_to(&cgnat, "https://api.provider.test/v1", FAKE_IP).is_err());

    // A mixed answer is still rejected as a whole.
    let mixed = fake_ip.validate_url_with_resolver(
        &parse_test_url("https://api.provider.test/v1"),
        |_host, _port| Ok(vec![FAKE_IP, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))]),
    );
    assert!(mixed.is_err());

    // Literal endpoints follow the same rules, without DNS.
    let literal = parse_test_url("https://198.18.0.7/v1");
    assert!(fake_ip.validate_url_without_resolution(&literal).is_ok());
    assert!(
        EndpointAddressRules::default()
            .validate_url_without_resolution(&literal)
            .is_err()
    );
}

#[test]
fn allow_list_covers_ipv4_mapped_and_nat64_forms() {
    let rules = rules_with(ProviderEndpointAccess::PublicOnly, &["198.18.0.0/15"]);
    for answer in ["::ffff:198.18.0.7", "64:ff9b::c612:7"] {
        let answer: IpAddr = answer.parse().unwrap();
        assert!(rules.allows_ip(&answer), "{answer}");
        assert!(
            !EndpointAddressRules::default().allows_ip(&answer),
            "{answer}"
        );
    }
}

#[test]
fn metadata_addresses_stay_blocked_even_when_an_allow_list_covers_them() {
    // Broad ranges that cover every metadata address without being link-local only.
    let rules = rules_with(
        ProviderEndpointAccess::PrivateNetwork,
        &[
            "168.0.0.0/7",
            "100.64.0.0/10",
            "fd00::/8",
            "fe00::/7",
            "64:ff9b::/96",
            "::ffff:0:0/96",
        ],
    );
    for metadata in [
        "169.254.169.254",
        "168.63.129.16",
        "100.100.100.200",
        "169.254.170.2",
        "169.254.170.23",
        "fd00:ec2::254",
        "fd00:ec2::23",
        "fd00:c1::a9fe:a9fe",
        "::ffff:169.254.169.254",
        "::ffff:168.63.129.16",
        "::ffff:100.100.100.200",
        "64:ff9b::a9fe:a9fe",
        "64:ff9b::a83f:8110",
        "64:ff9b::6464:64c8",
        "64:ff9b::a9fe:aa02",
    ] {
        let ip: IpAddr = metadata.parse().unwrap();
        assert!(!rules.allows_ip(&ip), "{metadata} must stay blocked");
        assert!(
            !is_provider_endpoint_ip_allowed(ProviderEndpointAccess::PrivateNetwork, &ip),
            "{metadata}"
        );
        let error = resolve_to(&rules, "https://api.provider.test/v1", ip)
            .expect_err("metadata answer must be rejected");
        assert!(error.to_string().contains("always blocked"), "{error}");
        assert_eq!(error.provider_setting_hint(), None, "{metadata}");
    }
    // The non-metadata, non-link-local parts of the listed ranges are still allowed.
    for allowed in [
        "168.63.1.1",
        "100.100.100.201",
        "100.64.3.9",
        "fd00:ec2::24",
    ] {
        assert!(rules.allows_ip(&allowed.parse().unwrap()), "{allowed}");
    }
}

#[test]
fn cgnat_opt_in_does_not_reach_alibaba_metadata_through_redirects() {
    // The README's recommended fake-IP + CGNAT list.
    let rules = rules_with(
        ProviderEndpointAccess::PublicOnly,
        &["198.18.0.0/15", "100.64.0.0/10"],
    );
    for target in [
        "http://100.100.100.200/latest/meta-data/ram/security-credentials/",
        "http://[::ffff:100.100.100.200]/latest/meta-data/",
        "http://[64:ff9b::6464:64c8]/latest/meta-data/",
    ] {
        let error = rules
            .validate_url_without_resolution(&parse_test_url(target))
            .expect_err("Alibaba metadata must stay blocked with a CGNAT opt-in");
        assert!(
            error.to_string().contains("always blocked"),
            "{target}: {error}"
        );
    }
    assert!(
        resolve_to(
            &rules,
            "https://api.provider.test/v1",
            "100.100.100.200".parse().unwrap()
        )
        .is_err()
    );
    assert!(rules.allows_ip(&CGNAT));
}

#[test]
fn link_local_is_never_re_admitted_by_an_allow_list() {
    let rules = rules_with(
        ProviderEndpointAccess::PublicOnly,
        &["168.0.0.0/7", "fe00::/7"],
    );
    for link_local in [
        "169.254.0.1",
        "169.254.0.23",
        "169.254.10.1",
        "::ffff:169.254.10.1",
        "64:ff9b::a9fe:a01",
        "fe80::a9fe:a9fe",
    ] {
        let ip: IpAddr = link_local.parse().unwrap();
        assert!(!rules.allows_ip(&ip), "{link_local} must stay blocked");
        let error = resolve_to(&rules, "https://api.provider.test/v1", ip)
            .expect_err("link-local answer must be rejected");
        assert!(error.to_string().contains("always blocked"), "{error}");
        assert_eq!(error.provider_setting_hint(), None, "{link_local}");
    }
    assert!(rules.allows_ip(&"168.63.1.1".parse().unwrap()));
    // An entry that is entirely link-local can never take effect, so it is a config error.
    for entry in [
        "169.254.0.0/16",
        "169.254.170.0/24",
        "169.254.170.2",
        "fe80::/10",
        "fe80::/64",
    ] {
        let error = EndpointAllowCidrs::parse([entry]).expect_err(entry);
        assert!(error.contains("link-local"), "{entry}: {error}");
    }
}

#[test]
fn rejection_names_the_resolved_address_and_the_allowing_setting() {
    let error = resolve_to(
        &EndpointAddressRules::default(),
        "https://api.provider.test/v1/chat?key=secret",
        FAKE_IP,
    )
    .expect_err("fake-IP answer must be rejected by default");
    let message = error.with_provider_setting_hint();
    assert!(
        message.contains("'api.provider.test' (resolved to 198.18.0.7)"),
        "{message}"
    );
    assert!(
        message.contains("add a CIDR covering 198.18.0.7 to the provider's `endpoint_allow_cidrs`"),
        "{message}"
    );
    // Fake-IP addresses are not private-network addresses, so that is not suggested.
    assert!(!message.contains("private_network"), "{message}");
    assert!(
        !message.contains("secret") && !message.contains("/v1"),
        "{message}"
    );

    let private = resolve_to(
        &EndpointAddressRules::default(),
        "https://api.provider.test/v1",
        IpAddr::V4(Ipv4Addr::new(10, 1, 2, 3)),
    )
    .expect_err("RFC 1918 answer must be rejected for public_only");
    assert!(
        private
            .with_provider_setting_hint()
            .contains("or set `endpoint_access: private_network`")
    );

    // Generic outbound callers (webhooks, alerts) get the address but no provider hint in Display.
    assert!(!error.to_string().contains("endpoint_allow_cidrs"));
}

#[test]
fn allow_cidr_parsing_is_strict() {
    let parsed = EndpointAllowCidrs::parse(["198.18.0.0/15", " 100.64.0.0/10 ", "fd7a::/16"])
        .expect("valid CIDRs should parse");
    assert_eq!(parsed.as_slice().len(), 3);
    assert_eq!(parsed.as_slice()[0].to_string(), "198.18.0.0/15");
    // A bare address is a single-host range.
    let single = EndpointAllowCidrs::parse(["198.18.0.7"]).unwrap();
    assert!(single.contains(&FAKE_IP));
    assert!(!single.contains(&IpAddr::V4(Ipv4Addr::new(198, 18, 0, 8))));

    for invalid in [
        "198.18.0.0/33",
        "198.18.0.1/15",
        "0.0.0.0/0",
        "::/0",
        "198.18.0.0/x",
        "198.18.0.0/+15",
        "198.18.0.0/",
        "not-an-ip/8",
        "",
    ] {
        assert!(
            EndpointAllowCidrs::parse([invalid]).is_err(),
            "{invalid:?} must be rejected"
        );
    }
    let host_bits = EndpointAllowCidrs::parse(["198.18.0.1/15"]).unwrap_err();
    assert!(
        host_bits.contains("did you mean 198.18.0.0/15"),
        "{host_bits}"
    );

    let json: EndpointAllowCidrs = serde_json::from_value(serde_json::json!(["198.18.0.0/15"]))
        .expect("allow list should deserialize");
    assert_eq!(
        serde_json::to_value(&json).unwrap(),
        serde_json::json!(["198.18.0.0/15"])
    );
    assert!(
        serde_json::from_value::<EndpointAllowCidrs>(serde_json::json!(["198.18.0.0/99"])).is_err()
    );
}
