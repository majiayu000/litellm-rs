//! Redis module tests

use super::pool::RedisPool;
use crate::config::models::storage::{RedisConfig, StorageConfig};
use crate::storage::StorageLayer;
use crate::storage::dependency_status::DependencyStatus;
use crate::utils::error::gateway_error::GatewayError;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

#[test]
fn test_sanitize_url() {
    let url = "redis://user:password@localhost:6379/0";
    let sanitized = RedisPool::sanitize_url(url);
    assert!(sanitized.contains("user:***@localhost"));
    assert!(!sanitized.contains("password"));
}

#[tokio::test]
async fn test_redis_set_get_roundtrip_with_live_pool() {
    let Some(pool) = live_redis_pool().await else {
        return;
    };

    let key = unique_test_key("roundtrip");
    let value = "value-from-integration-test";

    pool.set(&key, value, Some(30))
        .await
        .expect("set should write to redis");

    let cached = pool.get(&key).await.expect("get should read from redis");
    assert_eq!(cached.as_deref(), Some(value));

    let exists = pool
        .exists(&key)
        .await
        .expect("exists should succeed for written key");
    assert!(exists);

    pool.delete(&key).await.expect("delete should remove key");
    let exists_after_delete = pool
        .exists(&key)
        .await
        .expect("exists should succeed after delete");
    assert!(!exists_after_delete);
}

#[tokio::test]
async fn test_redis_delete_by_prefix_with_live_pool() {
    let Some(pool) = live_redis_pool().await else {
        return;
    };

    let prefix = unique_test_key("prefix");
    let first = format!("{prefix}:1");
    let second = format!("{prefix}:2");

    pool.set(&first, "one", Some(30))
        .await
        .expect("first set should write to redis");
    pool.set(&second, "two", Some(30))
        .await
        .expect("second set should write to redis");

    let deleted = pool
        .delete_by_prefix(&prefix)
        .await
        .expect("prefix delete should remove matching redis keys");
    assert_eq!(deleted, 2);
    assert!(!pool.exists(&first).await.expect("exists should succeed"));
    assert!(!pool.exists(&second).await.expect("exists should succeed"));
}

#[tokio::test]
async fn test_redis_pool_creation_returns_error_for_unreachable_endpoint() {
    let config = RedisConfig {
        url: "redis://127.0.0.1:1".to_string(),
        enabled: true,
        max_connections: 10,
        connection_timeout: 1,
        cluster: false,
        allow_degraded: false,
    };

    let result = RedisPool::new(&config).await;
    assert!(matches!(result, Err(GatewayError::Storage(_))));
}

#[tokio::test]
async fn cluster_mode_fails_to_connect_to_unreachable_seed() {
    let config = unreachable_cluster_config(true);

    let error = RedisPool::new(&config)
        .await
        .expect_err("unreachable cluster seed must fail instead of using a standalone client");
    assert!(matches!(error, GatewayError::Storage(_)));
}

#[tokio::test]
async fn storage_unreachable_cluster_fails_startup_unless_degraded() {
    let mut config = StorageConfig {
        redis: unreachable_cluster_config(false),
        ..StorageConfig::default()
    };

    let error = StorageLayer::new(&config)
        .await
        .expect_err("strict cluster mode must fail startup when the seed is unreachable");
    assert!(matches!(error, GatewayError::Storage(_)));
    assert!(
        !error.to_string().contains("not implemented"),
        "got: {error}"
    );

    config.redis.allow_degraded = true;
    let storage = StorageLayer::new(&config)
        .await
        .expect("allow_degraded must continue with a no-op pool");
    assert!(storage.redis.is_noop());
    assert_eq!(storage.redis_status, DependencyStatus::Degraded);
}

#[tokio::test]
async fn test_redis_pool_disabled_is_noop() {
    let config = RedisConfig {
        url: "redis://127.0.0.1:1".to_string(),
        enabled: false,
        max_connections: 10,
        connection_timeout: 1,
        cluster: false,
        allow_degraded: false,
    };

    let pool = RedisPool::new(&config)
        .await
        .expect("Disabled redis config should create no-op pool");
    assert!(pool.is_noop());
}

#[tokio::test]
async fn test_redis_delete_by_prefix_disabled_is_noop() {
    let pool = RedisPool::create_noop();

    let deleted = pool
        .delete_by_prefix("litellm-rs:test:")
        .await
        .expect("disabled redis prefix delete should not fail");
    assert_eq!(deleted, 0);
}

#[tokio::test]
async fn cluster_mock_supports_get_set_mget_and_prefix_delete() {
    let mock = MockCluster::bind(false).await;
    let pool = cluster_pool_for(&mock.url).await;

    pool.set("alpha", "1", None)
        .await
        .expect("cluster set should succeed");
    pool.set("beta", "2", None)
        .await
        .expect("cluster set should succeed");

    assert_eq!(pool.get("alpha").await.expect("get"), Some("1".to_string()));

    let values = pool
        .mget(&[
            "alpha".to_string(),
            "missing".to_string(),
            "beta".to_string(),
        ])
        .await
        .expect("cluster mget should preserve order without CROSSSLOT");
    assert_eq!(
        values,
        vec![Some("1".to_string()), None, Some("2".to_string())]
    );

    pool.mset(
        &[
            ("gamma".to_string(), "3".to_string()),
            ("delta".to_string(), "4".to_string()),
        ],
        None,
    )
    .await
    .expect("cluster mset should issue per-key SET");
    assert_eq!(pool.get("gamma").await.expect("get"), Some("3".to_string()));

    let deleted = pool
        .delete_by_prefix("g")
        .await
        .expect("cluster prefix delete should SCAN masters");
    assert_eq!(deleted, 1);
    assert!(pool.get("gamma").await.expect("get").is_none());
    assert_eq!(pool.get("delta").await.expect("get"), Some("4".to_string()));
}

#[tokio::test]
async fn cluster_mock_follows_moved_redirection() {
    let mock = MockCluster::bind(true).await;
    let pool = cluster_pool_for(&mock.url).await;

    pool.set("moved-key", "ok", None)
        .await
        .expect("set before redirected get");
    let value = pool
        .get("moved-key")
        .await
        .expect("cluster client should retry after MOVED");
    assert_eq!(value.as_deref(), Some("ok"));
    assert!(mock.redirected.load(Ordering::SeqCst));
}

async fn cluster_pool_for(url: &str) -> RedisPool {
    let config = RedisConfig {
        url: url.to_string(),
        enabled: true,
        max_connections: 4,
        connection_timeout: 2,
        cluster: true,
        allow_degraded: false,
    };
    RedisPool::new(&config)
        .await
        .unwrap_or_else(|error| panic!("mock cluster should accept ClusterClient: {error}"))
}

fn unreachable_cluster_config(allow_degraded: bool) -> RedisConfig {
    RedisConfig {
        url: "redis://127.0.0.1:1".to_string(),
        enabled: true,
        max_connections: 10,
        connection_timeout: 1,
        cluster: true,
        allow_degraded,
    }
}

async fn live_redis_pool() -> Option<RedisPool> {
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let config = RedisConfig {
        url: redis_url.clone(),
        enabled: true,
        max_connections: 10,
        connection_timeout: 1,
        cluster: false,
        allow_degraded: false,
    };

    match RedisPool::new(&config).await {
        Ok(pool) => match pool.health_check().await {
            Ok(()) => Some(pool),
            Err(err) => {
                if std::env::var("CI").is_ok() {
                    panic!("Redis should pass health check in CI at {redis_url}: {err}");
                }

                eprintln!("Skipping live Redis integration test: {err}");
                None
            }
        },
        Err(err) => {
            if std::env::var("CI").is_ok() {
                panic!("Redis should be reachable in CI at {redis_url}: {err}");
            }

            eprintln!("Skipping live Redis integration test: {err}");
            None
        }
    }
}

fn unique_test_key(suffix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    format!("litellm-rs:test:{suffix}:{nanos}")
}

/// In-process Redis Cluster RESP mock (PING / CLUSTER SLOTS / GET / SET / SCAN / DEL).
///
/// CI does not run a real cluster. This handshake is the minimum
/// `redis::cluster_async::ClusterClient` accepts for a single-node mapping.
struct MockCluster {
    url: String,
    redirected: Arc<AtomicBool>,
}

impl MockCluster {
    async fn bind(redirect_first_get: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("mock cluster should bind");
        let addr = listener.local_addr().expect("mock cluster address");
        let store = Arc::new(Mutex::new(HashMap::<String, String>::new()));
        let redirected = Arc::new(AtomicBool::new(false));
        let pending_redirect = Arc::new(AtomicBool::new(redirect_first_get));
        let host = addr.ip().to_string();
        let port = addr.port();
        let accept_redirected = Arc::clone(&redirected);

        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    break;
                };
                let store = Arc::clone(&store);
                let pending_redirect = Arc::clone(&pending_redirect);
                let redirected = Arc::clone(&accept_redirected);
                let host = host.clone();
                tokio::spawn(async move {
                    serve_mock_cluster(stream, store, pending_redirect, redirected, host, port)
                        .await;
                });
            }
        });

        Self {
            url: format!("redis://{addr}"),
            redirected,
        }
    }
}

async fn serve_mock_cluster(
    mut stream: tokio::net::TcpStream,
    store: Arc<Mutex<HashMap<String, String>>>,
    pending_redirect: Arc<AtomicBool>,
    redirected: Arc<AtomicBool>,
    host: String,
    port: u16,
) {
    let mut buf = Vec::new();
    loop {
        let mut chunk = [0_u8; 1024];
        let n = match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(n) => n,
        };
        buf.extend_from_slice(&chunk[..n]);

        while let Some((args, consumed)) = parse_resp_array(&buf) {
            buf.drain(..consumed);
            let reply =
                handle_mock_command(&args, &store, &pending_redirect, &redirected, &host, port)
                    .await;
            if stream.write_all(&reply).await.is_err() {
                return;
            }
        }
    }
}

async fn handle_mock_command(
    args: &[Vec<u8>],
    store: &Arc<Mutex<HashMap<String, String>>>,
    pending_redirect: &Arc<AtomicBool>,
    redirected: &Arc<AtomicBool>,
    host: &str,
    port: u16,
) -> Vec<u8> {
    let name = args
        .first()
        .map(|arg| String::from_utf8_lossy(arg).to_ascii_uppercase())
        .unwrap_or_default();
    match name.as_str() {
        "PING" => b"+PONG\r\n".to_vec(),
        "READONLY" | "CLIENT" | "OK" => b"+OK\r\n".to_vec(),
        "CLUSTER"
            if args
                .get(1)
                .is_some_and(|arg| eq_ignore_ascii(arg, b"SLOTS")) =>
        {
            cluster_slots_resp(host, port)
        }
        "GET" => {
            let key = arg_str(args, 1);
            if pending_redirect.swap(false, Ordering::SeqCst) {
                redirected.store(true, Ordering::SeqCst);
                return format!("-MOVED 123 {host}:{port}\r\n").into_bytes();
            }
            match store.lock().await.get(&key) {
                Some(value) => bulk_string(value),
                None => b"$-1\r\n".to_vec(),
            }
        }
        "SET" | "SETEX" => {
            let (key, value) = if name == "SETEX" {
                (arg_str(args, 1), arg_str(args, 3))
            } else {
                (arg_str(args, 1), arg_str(args, 2))
            };
            store.lock().await.insert(key, value);
            b"+OK\r\n".to_vec()
        }
        "DEL" => {
            let key = arg_str(args, 1);
            let removed = store.lock().await.remove(&key).is_some() as i64;
            format!(":{removed}\r\n").into_bytes()
        }
        "SCAN" => {
            let pattern = scan_pattern(args);
            let keys: Vec<String> = store
                .lock()
                .await
                .keys()
                .filter(|key| glob_prefix_match(key, &pattern))
                .cloned()
                .collect();
            scan_resp(&keys)
        }
        "EXISTS" => {
            let present = store.lock().await.contains_key(&arg_str(args, 1)) as i64;
            format!(":{present}\r\n").into_bytes()
        }
        _ => b"-ERR unknown command\r\n".to_vec(),
    }
}

fn arg_str(args: &[Vec<u8>], index: usize) -> String {
    args.get(index)
        .map(|arg| String::from_utf8_lossy(arg).into_owned())
        .unwrap_or_default()
}

fn eq_ignore_ascii(value: &[u8], expected: &[u8]) -> bool {
    value.eq_ignore_ascii_case(expected)
}

fn scan_pattern(args: &[Vec<u8>]) -> String {
    args.windows(2)
        .find(|pair| eq_ignore_ascii(&pair[0], b"MATCH"))
        .map(|pair| String::from_utf8_lossy(&pair[1]).into_owned())
        .unwrap_or_else(|| "*".to_string())
}

fn glob_prefix_match(key: &str, pattern: &str) -> bool {
    pattern
        .strip_suffix('*')
        .map(|prefix| key.starts_with(prefix))
        .unwrap_or_else(|| key == pattern)
}

fn bulk_string(value: &str) -> Vec<u8> {
    format!("${}\r\n{}\r\n", value.len(), value).into_bytes()
}

fn cluster_slots_resp(host: &str, port: u16) -> Vec<u8> {
    format!(
        "*1\r\n*3\r\n:0\r\n:16383\r\n*2\r\n${}\r\n{host}\r\n:{port}\r\n",
        host.len()
    )
    .into_bytes()
}

fn scan_resp(keys: &[String]) -> Vec<u8> {
    let mut out = format!("*2\r\n$1\r\n0\r\n*{}\r\n", keys.len()).into_bytes();
    for key in keys {
        out.extend(bulk_string(key));
    }
    out
}

fn parse_resp_array(buf: &[u8]) -> Option<(Vec<Vec<u8>>, usize)> {
    if buf.first()? != &b'*' {
        return None;
    }
    let (count, mut pos) = parse_crlf_int(buf, 1)?;
    if count < 0 {
        return None;
    }
    let mut args = Vec::with_capacity(count as usize);
    for _ in 0..count {
        if buf.get(pos)? != &b'$' {
            return None;
        }
        let (len, next) = parse_crlf_int(buf, pos + 1)?;
        pos = next;
        let len = usize::try_from(len).ok()?;
        if buf.len() < pos + len + 2 || &buf[pos + len..pos + len + 2] != b"\r\n" {
            return None;
        }
        args.push(buf[pos..pos + len].to_vec());
        pos += len + 2;
    }
    Some((args, pos))
}

fn parse_crlf_int(buf: &[u8], start: usize) -> Option<(i64, usize)> {
    let rest = buf.get(start..)?;
    let cr = rest.windows(2).position(|window| window == b"\r\n")?;
    let value = std::str::from_utf8(rest.get(..cr)?).ok()?.parse().ok()?;
    Some((value, start + cr + 2))
}
