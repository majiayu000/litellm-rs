//! Exercise the real Cluster client against an owned RESP fault server.

use super::*;
use crate::config::models::storage::RedisConfig;
use crate::storage::redis::tests::{cluster_slots_resp, parse_resp_array};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

struct FaultCluster {
    url: String,
    mode: Arc<AtomicUsize>,
    writes: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for FaultCluster {
    fn drop(&mut self) {
        // Dropping the accept task's JoinSet aborts every owned socket task.
        self.task.abort();
    }
}

impl FaultCluster {
    async fn bind(mode: usize) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let mode = Arc::new(AtomicUsize::new(mode));
        let writes = Arc::new(AtomicUsize::new(0));
        let server_mode = Arc::clone(&mode);
        let server_writes = Arc::clone(&writes);
        let task = tokio::spawn(async move {
            let mut clients = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let Ok((mut socket, _)) = accepted else { return; };
                        let mode = Arc::clone(&server_mode);
                        let writes = Arc::clone(&server_writes);
                        clients.spawn(async move {
                            let mut buffer = Vec::new();
                            loop {
                                let mut chunk = [0; 8192];
                                let length = match socket.read(&mut chunk).await {
                                    Ok(0) | Err(_) => return,
                                    Ok(length) => length,
                                };
                                buffer.extend_from_slice(&chunk[..length]);
                                while let Some((args, consumed)) = parse_resp_array(&buffer) {
                                    buffer.drain(..consumed);
                                    let command = args[0].as_slice();
                                    let reply = match command {
                                        b"CLUSTER" => cluster_slots_resp("127.0.0.1", address.port()),
                                        b"PING" => b"+PONG\r\n".to_vec(),
                                        b"EVALSHA" | b"EVAL" => {
                                            writes.fetch_add(1, Ordering::SeqCst);
                                            match mode.load(Ordering::SeqCst) {
                                                0 => continue, // Connected, but no command response.
                                                2 => return,   // Write received, connection lost.
                                                3 => format!("-MOVED 1 {address}\r\n").into_bytes(),
                                                _ => b"*3\r\n:1\r\n:0\r\n:3\r\n".to_vec(),
                                            }
                                        }
                                        _ => b"+OK\r\n".to_vec(),
                                    };
                                    if socket.write_all(&reply).await.is_err() { return; }
                                }
                            }
                        });
                    }
                    _ = clients.join_next(), if !clients.is_empty() => {}
                }
            }
        });
        Self {
            url: format!("redis://{address}"),
            mode,
            writes,
            task,
        }
    }
}

fn args() -> BudgetLeaseArgs<'static> {
    BudgetLeaseArgs {
        op: "reserve",
        now_ms: 1_000,
        period_epoch: 1,
        amount: 3,
        max_or_actual_or_force: 100,
        seed_committed: 0,
        lease_id: "one-write-only",
        ttl_ms: 60_000,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn cluster_silent_reply_and_disconnect_are_bounded_without_write_replay() {
    for failure in [0, 2, 3] {
        let server = FaultCluster::bind(failure).await;
        let pool = RedisPool::new(&RedisConfig {
            url: server.url.clone(),
            enabled: true,
            cluster: true,
            allow_degraded: false,
            ..RedisConfig::default()
        })
        .await
        .unwrap();
        let cached = BudgetRuntimeConnection::default();
        let original = cached.connect(&pool).await.unwrap();
        let start = tokio::time::Instant::now();
        let failed = tokio::time::timeout(
            BUDGET_PHASE_TIMEOUT * 2,
            cached.invoke(&pool, "{deadline}:budget", args()),
        )
        .await
        .expect("a connected silent Cluster must not occupy a worker forever");
        assert!(failed.is_err());
        if failure == 0 {
            assert!(start.elapsed() >= BUDGET_PHASE_TIMEOUT / 2);
        }
        assert_eq!(
            server.writes.load(Ordering::SeqCst),
            1,
            "an uncertain reserve must not be retried by the Cluster driver"
        );
        assert!(cached.live.lock().await.is_none());

        server.mode.store(1, Ordering::SeqCst);
        let accepted = cached
            .invoke(&pool, "{deadline}:budget", args())
            .await
            .unwrap();
        assert!(accepted.allowed);
        assert_eq!(accepted.outstanding, 3);
        assert_eq!(server.writes.load(Ordering::SeqCst), 2);
        let replacement = cached.connect(&pool).await.unwrap();
        assert!(!Arc::ptr_eq(&original, &replacement));
        cached.invalidate(&original).await;
        assert!(Arc::ptr_eq(
            &replacement,
            &cached.connect(&pool).await.unwrap()
        ));
    }
}

fn disconnected_pool() -> RedisPool {
    RedisPool {
        connection: None,
        config: RedisConfig {
            url: "redis://127.0.0.1:9".into(),
            enabled: true,
            ..RedisConfig::default()
        },
        noop_mode: false,
        semaphore: Arc::new(tokio::sync::Semaphore::new(1)),
    }
}

#[tokio::test(start_paused = true)]
async fn occupied_connection_permit_has_a_deadline_without_starting_io() {
    let pool = disconnected_pool();
    let held = pool.semaphore.acquire().await.unwrap();
    let start = tokio::time::Instant::now();
    let error = pool
        .invoke_budget_lease("deadline", args())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("connection permit deadline"));
    assert_eq!(start.elapsed(), BUDGET_PHASE_TIMEOUT);
    assert_eq!(pool.semaphore.available_permits(), 0);
    drop(held);
    assert_eq!(pool.semaphore.available_permits(), 1);
}

#[tokio::test(start_paused = true)]
async fn contended_connection_creation_has_a_deadline_without_starting_io() {
    let pool = disconnected_pool();
    let cached = BudgetRuntimeConnection::default();
    let _held = cached.live.lock().await;
    let start = tokio::time::Instant::now();
    let error = cached.invoke(&pool, "deadline", args()).await.unwrap_err();
    assert!(error.to_string().contains("connection lock deadline"));
    assert_eq!(start.elapsed(), BUDGET_PHASE_TIMEOUT);
}
