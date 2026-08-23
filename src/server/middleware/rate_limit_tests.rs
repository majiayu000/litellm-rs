use super::*;
use actix_web::test::TestRequest;
use uuid::Uuid;

fn require_recorded_at(value: Option<Instant>) -> Instant {
    match value {
        Some(value) => value,
        None => panic!("allowed reservation should record a timestamp"),
    }
}

#[test]
fn test_parse_peer_ip_ipv4_with_port() {
    assert_eq!(parse_peer_ip("127.0.0.1:1234"), "127.0.0.1");
}

#[test]
fn test_parse_peer_ip_ipv4_no_port() {
    assert_eq!(parse_peer_ip("10.0.0.1"), "10.0.0.1");
}

#[test]
fn test_parse_peer_ip_ipv6_with_port() {
    assert_eq!(parse_peer_ip("[::1]:8080"), "::1");
}

#[test]
fn test_parse_peer_ip_unknown_falls_back() {
    assert_eq!(parse_peer_ip("unknown"), "unknown");
}

#[test]
fn test_trusted_proxy_match() {
    let proxies = ["10.0.0.1".to_string()];
    assert!(proxies.iter().any(|p| p == "10.0.0.1"));
}

#[test]
fn test_trusted_proxy_no_match() {
    let proxies = ["10.0.0.1".to_string()];
    assert!(!proxies.iter().any(|p| p == "10.0.0.2"));
}

#[test]
fn test_trusted_proxy_empty_list() {
    let proxies: Vec<String> = vec![];
    assert!(!proxies.iter().any(|p| p == "127.0.0.1"));
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
    let got = last_untrusted_xff_ip("1.2.3.4, 203.0.113.50", &trusted);
    assert_eq!(got.as_deref(), Some("203.0.113.50"));
}

#[test]
fn test_last_untrusted_xff_ip_walks_past_trusted_chain() {
    let trusted = vec!["10.0.0.1".to_string(), "10.0.0.2".to_string()];
    let got = last_untrusted_xff_ip("9.9.9.9, 1.2.3.4, 10.0.0.2, 10.0.0.1", &trusted);
    assert_eq!(got.as_deref(), Some("1.2.3.4"));
}

#[test]
fn test_last_untrusted_xff_ip_all_trusted_falls_back_to_rightmost() {
    let trusted = vec!["10.0.0.1".to_string(), "10.0.0.2".to_string()];
    let got = last_untrusted_xff_ip("10.0.0.2, 10.0.0.1", &trusted);
    assert_eq!(got.as_deref(), Some("10.0.0.1"));
}

#[test]
fn test_last_untrusted_xff_ip_single_entry() {
    let trusted = vec![];
    let got = last_untrusted_xff_ip("203.0.113.7", &trusted);
    assert_eq!(got.as_deref(), Some("203.0.113.7"));
}

#[test]
fn test_last_untrusted_xff_ip_ignores_empty_entries() {
    let trusted = vec!["10.0.0.1".to_string()];
    let got = last_untrusted_xff_ip("  ,  , 10.0.0.1", &trusted);
    assert_eq!(got.as_deref(), Some("10.0.0.1"));
}
