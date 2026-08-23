use super::*;
use actix_web::http::header::{HeaderName, HeaderValue};
use actix_web::test::TestRequest;
use uuid::Uuid;

fn require_recorded_at(value: Option<Instant>) -> Instant {
    match value {
        Some(value) => value,
        None => panic!("allowed reservation should record a timestamp"),
    }
}

fn parse_single_xff(value: &str, trusted_proxies: &[String]) -> Option<IpAddr> {
    let req = TestRequest::default()
        .insert_header(("X-Forwarded-For", value))
        .to_srv_request();
    last_untrusted_xff_ip(req.headers(), trusted_proxies)
}

#[test]
fn test_parse_ip_ipv4_with_port() {
    assert_eq!(parse_ip("127.0.0.1:1234"), "127.0.0.1".parse().ok());
}

#[test]
fn test_parse_ip_ipv4_no_port() {
    assert_eq!(parse_ip("10.0.0.1"), "10.0.0.1".parse().ok());
}

#[test]
fn test_parse_ip_ipv6_with_port() {
    assert_eq!(parse_ip("[::1]:8080"), "::1".parse().ok());
}

#[test]
fn test_parse_ip_rejects_unknown() {
    assert_eq!(parse_ip("unknown"), None);
}

#[test]
fn test_parse_ip_canonicalizes_ipv4_mapped_ipv6() {
    assert_eq!(
        parse_ip("::ffff:192.0.2.1"),
        Some("192.0.2.1".parse().unwrap())
    );
}

#[test]
fn test_trusted_proxy_match() {
    let proxies = ["10.0.0.1".to_string()];
    assert!(is_trusted_proxy("10.0.0.1".parse().unwrap(), &proxies));
}

#[test]
fn test_trusted_proxy_no_match() {
    let proxies = ["10.0.0.1".to_string()];
    assert!(!is_trusted_proxy("10.0.0.2".parse().unwrap(), &proxies));
}

#[test]
fn test_trusted_proxy_empty_list() {
    let proxies: Vec<String> = vec![];
    assert!(!is_trusted_proxy("127.0.0.1".parse().unwrap(), &proxies));
}

#[test]
fn test_extract_client_key_ignores_rotating_authorization_headers() {
    let req_a = TestRequest::default()
        .peer_addr("203.0.113.10:1000".parse().unwrap())
        .insert_header(("Authorization", "Bearer bogus-a"))
        .to_srv_request();
    let req_b = TestRequest::default()
        .peer_addr("203.0.113.10:1000".parse().unwrap())
        .insert_header(("Authorization", "Bearer bogus-b"))
        .to_srv_request();

    let key_a = extract_client_key(&req_a, &[]);
    let key_b = extract_client_key(&req_b, &[]);

    assert_eq!(key_a, "ip:203.0.113.10");
    assert_eq!(key_a, key_b);
}

#[test]
fn test_extract_client_key_ignores_rotating_api_key_headers() {
    let req_a = TestRequest::default()
        .peer_addr("203.0.113.20:1000".parse().unwrap())
        .insert_header(("x-api-key", "bogus-a"))
        .to_srv_request();
    let req_b = TestRequest::default()
        .peer_addr("203.0.113.20:1000".parse().unwrap())
        .insert_header(("x-api-key", "bogus-b"))
        .to_srv_request();

    let key_a = extract_client_key(&req_a, &[]);
    let key_b = extract_client_key(&req_b, &[]);

    assert_eq!(key_a, "ip:203.0.113.20");
    assert_eq!(key_a, key_b);
}

#[test]
fn test_extract_client_key_uses_rightmost_untrusted_forwarded_ip() {
    // Only enumerated proxies are skipped when walking X-Forwarded-For from
    // the right. 10.0.0.2 is not listed, so its own address is used instead
    // of the attacker-controllable leftmost entry.
    let req = TestRequest::default()
        .peer_addr("10.0.0.1:1000".parse().unwrap())
        .insert_header(("X-Forwarded-For", "198.51.100.7, 10.0.0.2"))
        .to_srv_request();

    let key = extract_client_key(&req, &["10.0.0.1".to_string()]);

    assert_eq!(key, "ip:10.0.0.2");
}

#[test]
fn test_extract_client_key_sees_through_enumerated_proxy_chain() {
    let req = TestRequest::default()
        .peer_addr("10.0.0.1:1000".parse().unwrap())
        .insert_header(("X-Forwarded-For", "198.51.100.7, 10.0.0.2"))
        .to_srv_request();

    let key = extract_client_key(&req, &["10.0.0.1".to_string(), "10.0.0.2".to_string()]);

    assert_eq!(key, "ip:198.51.100.7");
}

#[test]
fn test_extract_client_key_normalizes_ipv4_ports_and_whitespace() {
    let req = TestRequest::default()
        .peer_addr("10.0.0.1:1000".parse().unwrap())
        .insert_header(("X-Forwarded-For", "198.51.100.7:4242 , 10.0.0.2:8080"))
        .to_srv_request();

    let key = extract_client_key(&req, &["10.0.0.1:9000".to_string(), "10.0.0.2".to_string()]);

    assert_eq!(key, "ip:198.51.100.7");
}

#[test]
fn test_extract_client_key_normalizes_bracketed_ipv6_ports() {
    let req = TestRequest::default()
        .peer_addr("[2001:db8::1]:1000".parse().unwrap())
        .insert_header(("X-Forwarded-For", "[2001:db8::7]:4242, [2001:db8::2]:8080"))
        .to_srv_request();

    let key = extract_client_key(
        &req,
        &[
            "[2001:0DB8:0:0:0:0:0:1]:9000".to_string(),
            "2001:0DB8:0:0:0:0:0:2".to_string(),
        ],
    );

    assert_eq!(key, "ip:2001:db8::7");
}

#[test]
fn test_extract_client_key_rejects_malformed_forwarded_chain() {
    let req = TestRequest::default()
        .peer_addr("10.0.0.1:1000".parse().unwrap())
        .insert_header(("X-Forwarded-For", "198.51.100.7, malformed, 10.0.0.2"))
        .to_srv_request();

    let key = extract_client_key(&req, &["10.0.0.1".to_string(), "10.0.0.2".to_string()]);

    assert_eq!(key, "ip:10.0.0.1");
}

#[test]
fn test_extract_client_key_ignores_forwarded_header_from_untrusted_peer() {
    let req = TestRequest::default()
        .peer_addr("203.0.113.10:1000".parse().unwrap())
        .insert_header(("X-Forwarded-For", "198.51.100.7"))
        .to_srv_request();

    let key = extract_client_key(&req, &["10.0.0.1".to_string()]);

    assert_eq!(key, "ip:203.0.113.10");
}

#[test]
fn test_extract_client_key_ignores_forwarded_header_for_malformed_proxy_config() {
    let req = TestRequest::default()
        .peer_addr("203.0.113.10:1000".parse().unwrap())
        .insert_header(("X-Forwarded-For", "198.51.100.7"))
        .to_srv_request();

    let key = extract_client_key(&req, &["203.0.113.10/24".to_string()]);

    assert_eq!(key, "ip:203.0.113.10");
}

#[test]
fn test_extract_client_key_without_peer_ignores_forwarded_header() {
    let req = TestRequest::default()
        .insert_header(("X-Forwarded-For", "198.51.100.7"))
        .to_srv_request();

    let key = extract_client_key(&req, &["198.51.100.1".to_string()]);

    assert_eq!(key, "ip:unknown");
}

#[test]
fn test_extract_client_key_walks_repeated_forwarded_fields_in_global_reverse_order() {
    let req = TestRequest::default()
        .peer_addr("10.0.0.1:1000".parse().unwrap())
        .insert_header(("X-Forwarded-For", "192.0.2.66"))
        .append_header(("X-Forwarded-For", "198.51.100.7, 10.0.0.2"))
        .to_srv_request();

    let key = extract_client_key(&req, &["10.0.0.1".to_string(), "10.0.0.2".to_string()]);

    assert_eq!(key, "ip:198.51.100.7");
}

#[test]
fn test_extract_client_key_rejects_non_utf8_forwarded_field() {
    let non_utf8 = HeaderValue::from_bytes(&[0xff]).unwrap();
    let req = TestRequest::default()
        .peer_addr("10.0.0.1:1000".parse().unwrap())
        .insert_header(("X-Forwarded-For", "192.0.2.66"))
        .append_header((HeaderName::from_static("x-forwarded-for"), non_utf8))
        .to_srv_request();

    let key = extract_client_key(&req, &["10.0.0.1".to_string()]);

    assert_eq!(key, "ip:10.0.0.1");
}

#[test]
fn test_extract_client_key_ignores_non_utf8_prefix_after_selecting_client() {
    let mut forwarded = vec![0xff, b',', b' '];
    forwarded.extend_from_slice(b"198.51.100.7");
    let req = TestRequest::default()
        .peer_addr("10.0.0.1:1000".parse().unwrap())
        .insert_header((
            HeaderName::from_static("x-forwarded-for"),
            HeaderValue::from_bytes(&forwarded).unwrap(),
        ))
        .to_srv_request();

    let key = extract_client_key(&req, &["10.0.0.1".to_string()]);

    assert_eq!(key, "ip:198.51.100.7");
}

#[test]
fn test_extract_client_key_rejects_non_utf8_suffix_before_client_boundary() {
    let mut forwarded = b"198.51.100.7, ".to_vec();
    forwarded.push(0xff);
    let req = TestRequest::default()
        .peer_addr("10.0.0.1:1000".parse().unwrap())
        .insert_header((
            HeaderName::from_static("x-forwarded-for"),
            HeaderValue::from_bytes(&forwarded).unwrap(),
        ))
        .to_srv_request();

    let key = extract_client_key(&req, &["10.0.0.1".to_string()]);

    assert_eq!(key, "ip:10.0.0.1");
}

#[test]
fn test_extract_client_key_canonicalizes_ipv4_mapped_ipv6_everywhere() {
    let req = TestRequest::default()
        .peer_addr("[::ffff:10.0.0.1]:1000".parse().unwrap())
        .insert_header((
            "X-Forwarded-For",
            "[::ffff:198.51.100.7]:4242, ::ffff:10.0.0.2",
        ))
        .to_srv_request();

    let key = extract_client_key(
        &req,
        &["10.0.0.1".to_string(), "[::ffff:10.0.0.2]:9000".to_string()],
    );

    assert_eq!(key, "ip:198.51.100.7");
}

#[test]
fn test_extract_client_key_prefers_authenticated_api_key_id() {
    let api_key_id = Uuid::new_v4();
    let req = TestRequest::default()
        .peer_addr("203.0.113.30:1000".parse().unwrap())
        .to_srv_request();
    req.extensions_mut()
        .insert(RequestContext::new().with_api_key(api_key_id));

    let key = extract_client_key(&req, &[]);

    assert_eq!(key, format!("api_key:{}", api_key_id));
}

#[test]
fn test_extract_client_key_uses_authenticated_user_id_without_api_key() {
    let req = TestRequest::default()
        .peer_addr("203.0.113.40:1000".parse().unwrap())
        .to_srv_request();
    req.extensions_mut()
        .insert(RequestContext::new().with_user_id("user-123"));

    let key = extract_client_key(&req, &[]);

    assert_eq!(key, "user:user-123");
}

#[test]
fn test_key_tracker_release_removes_recorded_slot() {
    let mut tracker = KeyTracker::new();
    let window = Duration::from_secs(60);

    let (allowed, _, recorded_at) = tracker.check_and_record(1, window);
    assert!(allowed);
    let recorded_at = require_recorded_at(recorded_at);
    assert_eq!(tracker.timestamps.len(), 1);

    tracker.release(recorded_at);

    assert!(tracker.timestamps.is_empty());
}

#[test]
fn test_key_tracker_release_allows_new_reservation() {
    let mut tracker = KeyTracker::new();
    let window = Duration::from_secs(60);
    let (allowed, _, recorded_at) = tracker.check_and_record(1, window);
    assert!(allowed);
    let recorded_at = require_recorded_at(recorded_at);
    let (allowed, retry_after, _) = tracker.check_and_record(1, window);
    assert!(!allowed);
    assert!(retry_after > 0);

    tracker.release(recorded_at);
    let (allowed, retry_after, _) = tracker.check_and_record(1, window);

    assert!(allowed);
    assert_eq!(retry_after, 0);
}

#[test]
fn test_key_tracker_release_keeps_newer_rejected_auth_slot() {
    let mut tracker = KeyTracker::new();
    let window = Duration::from_secs(60);

    let (first_allowed, _, first_recorded_at) = tracker.check_and_record(2, window);
    assert!(first_allowed);
    let first_recorded_at = require_recorded_at(first_recorded_at);

    std::thread::sleep(Duration::from_millis(1));

    let (second_allowed, _, second_recorded_at) = tracker.check_and_record(2, window);
    assert!(second_allowed);
    let second_recorded_at = require_recorded_at(second_recorded_at);

    tracker.release(first_recorded_at);

    assert_eq!(tracker.timestamps, vec![second_recorded_at]);
}

#[actix_web::test]
async fn test_auth_attempt_reservation_blocks_next_attempt_before_auth_result() {
    let first = TestRequest::default()
        .peer_addr(SocketAddr::from(([203, 0, 113, 210], 1000)))
        .to_srv_request();
    let second = TestRequest::default()
        .peer_addr(SocketAddr::from(([203, 0, 113, 210], 1001)))
        .to_srv_request();

    let reservation = match reserve_rate_limit_for_auth_attempt(&first, 1, &[]).await {
        Ok(reservation) => reservation,
        Err(err) => panic!("first auth attempt should reserve capacity: {err}"),
    };
    let second_result = reserve_rate_limit_for_auth_attempt(&second, 1, &[]).await;
    reservation.release().await;

    let rejected = match second_result {
        Ok(_) => panic!("second auth attempt should see the existing reservation"),
        Err(err) => err,
    };
    assert_eq!(rejected.status_code(), StatusCode::TOO_MANY_REQUESTS);
}

#[test]
fn test_enforce_fallback_capacity_evicts_stale_first() {
    let store: DashMap<String, KeyTracker> = DashMap::new();
    let now = Instant::now();
    let window = Duration::from_secs(60);
    for i in 0..3 {
        let mut t = KeyTracker::new();
        t.timestamps.push(now - Duration::from_secs(120));
        store.insert(format!("stale-{i}"), t);
    }
    for i in 0..2 {
        let mut t = KeyTracker::new();
        t.timestamps.push(now);
        store.insert(format!("fresh-{i}"), t);
    }
    assert_eq!(store.len(), 5);
    enforce_fallback_capacity(&store, window);
    assert_eq!(store.len(), 2);
    assert!(store.contains_key("fresh-0"));
    assert!(store.contains_key("fresh-1"));
}

#[test]
fn test_enforce_fallback_capacity_evicts_oldest_when_all_fresh() {
    let store: DashMap<String, KeyTracker> = DashMap::new();
    let base = Instant::now();
    for i in 0..MAX_FALLBACK_ENTRIES + 5 {
        let mut t = KeyTracker::new();
        t.timestamps.push(base + Duration::from_millis(i as u64));
        store.insert(format!("k-{i}"), t);
    }
    enforce_fallback_capacity(&store, Duration::from_secs(60));
    assert!(store.len() <= MAX_FALLBACK_ENTRIES);
    assert!(!store.contains_key("k-0"));
    assert!(store.contains_key(&format!("k-{}", MAX_FALLBACK_ENTRIES + 4)));
}

#[test]
fn test_last_untrusted_xff_ip_ignores_attacker_seeded_prefix() {
    // Client seeds "1.2.3.4"; the trusted proxy appends the client's real
    // address. The seeded entry must be ignored (old code picked it).
    let trusted = vec!["10.0.0.1".to_string()];
    let got = parse_single_xff("1.2.3.4, 203.0.113.50", &trusted);
    assert_eq!(got, Some("203.0.113.50".parse().unwrap()));
}

#[test]
fn test_last_untrusted_xff_ip_walks_past_trusted_chain() {
    let trusted = vec!["10.0.0.1".to_string(), "10.0.0.2".to_string()];
    let got = parse_single_xff("9.9.9.9, 1.2.3.4, 10.0.0.2, 10.0.0.1", &trusted);
    assert_eq!(got, Some("1.2.3.4".parse().unwrap()));
}

#[test]
fn test_last_untrusted_xff_ip_all_trusted_falls_back_to_rightmost() {
    let trusted = vec!["10.0.0.1".to_string(), "10.0.0.2".to_string()];
    let got = parse_single_xff("10.0.0.2, 10.0.0.1", &trusted);
    assert_eq!(got, Some("10.0.0.1".parse().unwrap()));
}

#[test]
fn test_last_untrusted_xff_ip_single_entry() {
    let trusted = vec![];
    let got = parse_single_xff("203.0.113.7", &trusted);
    assert_eq!(got, Some("203.0.113.7".parse().unwrap()));
}

#[test]
fn test_last_untrusted_xff_ip_rejects_empty_entries_in_trusted_suffix() {
    let trusted = vec!["10.0.0.1".to_string()];
    let got = parse_single_xff("  ,  , 10.0.0.1", &trusted);
    assert_eq!(got, None);
}
