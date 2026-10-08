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
        Self::bind_with_reply(mode, b"*3\r\n:1\r\n:0\r\n:3\r\n".to_vec()).await
    }

    async fn bind_with_reply(mode: usize, healthy_reply: Vec<u8>) -> Self {
        let healthy_reply = Arc::new(healthy_reply);
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
                        let healthy_reply = Arc::clone(&healthy_reply);
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
                                                _ => healthy_reply.as_ref().clone(),
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

async fn routing_operation(pool: &RedisPool, circuit: bool) -> Result<()> {
    if circuit {
        pool.circuit_invoke(
            "{deadline}:circuit",
            crate::storage::redis::circuit::CircuitArgs {
                op: "failure",
                now_secs: 1_000,
                window_epoch: 1,
                token: "",
                allowed_fails: 3,
                min_requests: 10,
                cooldown_secs: 5,
                success_threshold: 1,
                reason: 0,
            },
        )
        .await
        .map(|_| ())
    } else {
        // Drop cleanup invokes the same storage boundary as normal completion.
        pool.admission_cancel("{deadline}:admission", "uncertain-cleanup")
            .await
            .map(|_| ())
    }
}

#[tokio::test(flavor = "current_thread")]
async fn routing_live_silent_replies_release_capacity_and_recover_without_replay() {
    for cluster in [false, true] {
        for circuit in [false, true] {
            let reply = if circuit {
                b"*6\r\n:1\r\n:0\r\n:1\r\n:0\r\n:1\r\n:0\r\n".to_vec()
            } else {
                b"*4\r\n:1\r\n:0\r\n:0\r\n:0\r\n".to_vec()
            };
            let server = FaultCluster::bind_with_reply(1, reply).await;
            let pool = Arc::new(
                RedisPool::new(&RedisConfig {
                    url: server.url.clone(),
                    enabled: true,
                    cluster,
                    allow_degraded: false,
                    max_connections: 64,
                    ..RedisConfig::default()
                })
                .await
                .unwrap(),
            );
            routing_operation(&pool, circuit).await.unwrap();
            assert_eq!(server.writes.load(Ordering::SeqCst), 1);
            server.mode.store(0, Ordering::SeqCst);
            let start = tokio::time::Instant::now();
            let mut operations = tokio::task::JoinSet::new();
            for _ in 0..64 {
                let pool = Arc::clone(&pool);
                operations.spawn(async move { routing_operation(&pool, circuit).await });
            }
            tokio::time::timeout(BUDGET_PHASE_TIMEOUT * 2, async {
                while let Some(result) = operations.join_next().await {
                    assert!(
                        result.unwrap().is_err(),
                        "silence must not claim a Redis receipt"
                    );
                }
            })
            .await
            .expect("64 silent routing operations must all release capacity");
            assert!(start.elapsed() >= BUDGET_PHASE_TIMEOUT / 2);
            assert_eq!(pool.semaphore.available_permits(), 64);
            assert_eq!(
                server.writes.load(Ordering::SeqCst),
                65,
                "an uncertain routing command must not be replayed"
            );
            server.mode.store(1, Ordering::SeqCst);
            routing_operation(&pool, circuit).await.unwrap();
            assert_eq!(
                server.writes.load(Ordering::SeqCst),
                66,
                "a new call must recover with one new command"
            );
            assert_eq!(pool.semaphore.available_permits(), 64);
        }
    }
}
