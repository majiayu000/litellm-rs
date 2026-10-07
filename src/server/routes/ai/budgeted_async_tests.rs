//! Real Redis regressions through the gateway budget executor. The private TCP
//! proxy can hold a reply after Redis has applied it, without pausing Redis or
//! relying on the HTTP runtime to forward that reply.

use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{Mutex, Notify};

use super::BudgetedCall;
use crate::config::models::storage::RedisConfig;
use crate::core::budget::{
    BudgetReservationError, ModelLimitConfig, ProviderLimitConfig, ResetPeriod, UnifiedBudgetLimits,
};
use crate::storage::redis::RedisPool;

#[path = "budgeted_routing_completion_tests.rs"]
mod routing_completion_tests;
#[path = "budgeted_stream_completion_tests.rs"]
mod stream_completion_tests;

// Serialize this module's ordinary fixtures. Capacity and worker saturation
// run in separate processes so unrelated Redis tests never share those limits.
static TESTS: Mutex<()> = Mutex::const_new(());

async fn run_in_isolated_process(test_name: &str) -> bool {
    const CHILD: &str = "LITELLM_ASYNC_BUDGET_TEST_CHILD";
    let module = module_path!().split_once("::").unwrap().1;
    let name = format!("{module}::{test_name}");
    if std::env::var(CHILD).as_deref() == Ok(name.as_str()) {
        return false;
    }
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &name, "--nocapture", "--test-threads=1"])
            .env(CHILD, &name)
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("1 passed; 0 failed"),
        "isolated budget test must execute exactly once and pass:\n{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    true
}

#[derive(Default)]
struct ReplyGate {
    armed: AtomicBool,
    held: Notify,
    release: Notify,
    commands: AtomicUsize,
    watchdog_fired: AtomicBool,
}

struct Fixture {
    pool: Arc<RedisPool>,
    limits: Arc<UnifiedBudgetLimits>,
    provider: String,
    model: String,
    control: redis::aio::MultiplexedConnection,
    gate: Arc<ReplyGate>,
    stop: Arc<Notify>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Fixture {
    async fn new() -> Option<Self> {
        let Ok(redis_url) = std::env::var("REDIS_URL") else {
            assert!(std::env::var("CI").is_err(), "REDIS_URL is required in CI");
            return None;
        };
        let mut proxy_url = url::Url::parse(&redis_url).unwrap();
        assert_eq!(proxy_url.scheme(), "redis", "use the local Redis fixture");
        let upstream = (
            proxy_url.host_str().unwrap().to_owned(),
            proxy_url.port().unwrap_or(6379),
        );
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        proxy_url.set_host(Some("127.0.0.1")).unwrap();
        proxy_url
            .set_port(Some(listener.local_addr().unwrap().port()))
            .unwrap();
        let gate = Arc::new(ReplyGate::default());
        let stop = Arc::new(Notify::new());
        let thread_gate = Arc::clone(&gate);
        let thread_stop = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                    loop {
                        tokio::select! {
                            _ = thread_stop.notified() => break,
                            accepted = listener.accept() => {
                                let (client, _) = accepted.unwrap();
                                let upstream = upstream.clone();
                                let gate = Arc::clone(&thread_gate);
                                tokio::spawn(async move {
                                    let _ = proxy_connection(client, upstream, gate).await;
                                });
                            }
                        }
                    }
                });
        });
        let pool = Arc::new(
            RedisPool::new(&RedisConfig {
                url: proxy_url.to_string(),
                enabled: true,
                allow_degraded: false,
                // Worker saturation must reach all 32 SDK workers before any
                // reply is released, rather than saturating the default
                // 20-operation Redis semaphore first.
                max_connections: 64,
                ..RedisConfig::default()
            })
            .await
            .unwrap(),
        );
        let control = redis::Client::open(redis_url)
            .unwrap()
            .get_multiplexed_async_connection()
            .await
            .unwrap();
        let provider = format!("async-provider-{}", uuid::Uuid::new_v4());
        let model = format!("async-model-{}", uuid::Uuid::new_v4());
        let limits = Arc::new(UnifiedBudgetLimits::new().with_redis(pool.clone()));
        limits.providers.set_provider_limit(
            &provider,
            ProviderLimitConfig::new(1_000_000.0, ResetPeriod::Never),
        );
        limits.models.set_model_limit(
            &model,
            ModelLimitConfig::new(1_000_000.0, ResetPeriod::Never),
        );
        let fixture = Self {
            pool,
            limits,
            provider,
            model,
            control,
            gate,
            stop,
            thread: Some(thread),
        };
        // Open the dedicated budget connection before arming a reply barrier,
        // so that the barrier observes a Lua result rather than a handshake.
        fixture
            .limits
            .reserve_spend_async(&fixture.provider, &fixture.model, 0.1)
            .await
            .unwrap()
            .cancel_async()
            .await
            .unwrap();
        Some(fixture)
    }

    fn call(&self) -> BudgetedCall {
        BudgetedCall::new(
            Arc::clone(&self.limits),
            self.provider.clone(),
            self.model.clone(),
        )
    }

    fn pause_reply(&self) {
        assert!(!self.gate.armed.swap(true, Ordering::SeqCst));
    }

    async fn reply_held(&self) {
        tokio::time::timeout(Duration::from_secs(3), self.gate.held.notified())
            .await
            .expect("Redis reply must reach the proxy");
    }

    async fn state(&self, scope: &str) -> (i64, i64) {
        let name = if scope == "provider" {
            &self.provider
        } else {
            &self.model
        };
        let mut control = self.control.clone();
        redis::cmd("HMGET")
            .arg(RedisPool::budget_lease_key(scope, name))
            .arg(&["c", "o"])
            .query_async(&mut control)
            .await
            .unwrap()
    }

    async fn wait_state(&self, committed: i64, outstanding: i64) {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if self.state("provider").await == (committed, outstanding)
                    && self.state("model").await == (committed, outstanding)
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("both real Redis ledgers must reach the terminal state");
    }

    async fn finish(&self) {
        // A round trip after the terminal operations also drains earlier replies
        // before shutting down this fixture's private proxy.
        self.limits
            .reserve_spend_async(&self.provider, &self.model, 0.1)
            .await
            .unwrap()
            .cancel_async()
            .await
            .unwrap();
        assert!(!self.gate.watchdog_fired.load(Ordering::SeqCst));
        let mut control = self.control.clone();
        for (scope, name) in [("provider", &self.provider), ("model", &self.model)] {
            let fields: Vec<String> = redis::cmd("HKEYS")
                .arg(RedisPool::budget_lease_key(scope, name))
                .query_async(&mut control)
                .await
                .unwrap();
            assert!(
                fields
                    .iter()
                    .all(|field| !field.starts_with("l:") && !field.starts_with("p:")),
                "terminal cleanup must remove unfinished reservation identities"
            );
        }
        let _: i64 = redis::cmd("DEL")
            .arg(RedisPool::budget_lease_key("provider", &self.provider))
            .arg(RedisPool::budget_lease_key("model", &self.model))
            .query_async(&mut control)
            .await
            .unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.gate.release.notify_one();
        self.stop.notify_one();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

async fn proxy_connection(
    client: tokio::net::TcpStream,
    upstream: (String, u16),
    gate: Arc<ReplyGate>,
) -> std::io::Result<()> {
    let server = tokio::net::TcpStream::connect(upstream).await?;
    client.set_nodelay(true)?;
    server.set_nodelay(true)?;
    let (mut client_read, mut client_write) = client.into_split();
    let (mut server_read, mut server_write) = server.into_split();
    let commands = Arc::clone(&gate);
    let upload = async move {
        let mut buffer = [0; 8192];
        let mut tail = Vec::new();
        loop {
            let length = client_read.read(&mut buffer).await?;
            if length == 0 {
                return server_write.shutdown().await;
            }
            server_write.write_all(&buffer[..length]).await?;
            let previous = tail.len();
            tail.extend_from_slice(&buffer[..length]);
            let marker = b"\r\nEVALSHA\r\n";
            let count = tail
                .windows(marker.len())
                .enumerate()
                .filter(|(index, bytes)| *bytes == marker && index + marker.len() > previous)
                .count();
            commands.commands.fetch_add(count, Ordering::SeqCst);
            let keep = tail.len().saturating_sub(marker.len());
            tail.drain(..keep);
        }
    };
    let download = async move {
        let mut buffer = [0; 8192];
        loop {
            let length = server_read.read(&mut buffer).await?;
            if length == 0 {
                return client_write.shutdown().await;
            }
            if gate.armed.swap(false, Ordering::SeqCst) {
                gate.held.notify_one();
                // A regression which blocks the current-thread executor must
                // fail an assertion instead of hanging the test process forever.
                if tokio::time::timeout(Duration::from_secs(5), gate.release.notified())
                    .await
                    .is_err()
                {
                    gate.watchdog_fired.store(true, Ordering::SeqCst);
                }
            }
            client_write.write_all(&buffer[..length]).await?;
        }
    };
    tokio::try_join!(upload, download)?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn async_budget_gateway_waits_without_blocking_current_thread() {
    let _test = TESTS.lock().await;
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    fixture.pause_reply();
    let mut request = Box::pin(fixture.call().reserve_call(
        async |context| context.reserve_spend(1.0).await.map(Some),
        || async { Ok(()) },
    ));
    tokio::select! {
        _ = fixture.reply_held() => {},
        _ = &mut request => panic!("reservation must wait for its Redis reply"),
    }
    let started = std::time::Instant::now();
    for _ in 0..4 {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(10)) => {},
            _ = &mut request => panic!("Redis reply is still held"),
        }
    }
    assert!(started.elapsed() < Duration::from_secs(1));
    fixture.gate.release.notify_one();
    let (_, mut reservations) = request.await.unwrap();
    reservations.cancel().await;
    fixture.wait_state(0, 0).await;
    fixture.finish().await;
}

#[tokio::test(flavor = "current_thread")]
async fn slow_budget_redis_does_not_block_an_unrelated_http_request_on_the_same_thread() {
    use actix_web::{App, HttpResponse, test, web};

    let _test = TESTS.lock().await;
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    let data = web::Data::new((
        Arc::clone(&fixture.limits),
        fixture.provider.clone(),
        fixture.model.clone(),
    ));
    let app = test::init_service(
        App::new()
            .app_data(data)
            .route(
                "/budgeted",
                web::post().to(
                    |data: web::Data<(Arc<UnifiedBudgetLimits>, String, String)>| async move {
                        let (limits, provider, model) = data.get_ref();
                        let call =
                            BudgetedCall::new(Arc::clone(limits), provider.clone(), model.clone());
                        let (_, mut reservations) = call
                            .reserve_call(
                                async |context| context.reserve_spend(1.0).await.map(Some),
                                || async { Ok(()) },
                            )
                            .await
                            .unwrap();
                        reservations.cancel().await;
                        HttpResponse::Ok().finish()
                    },
                ),
            )
            .route(
                "/unrelated",
                web::get().to(|| async { HttpResponse::Ok().finish() }),
            ),
    )
    .await;
    fixture.pause_reply();
    let mut budgeted = Box::pin(test::call_service(
        &app,
        test::TestRequest::post().uri("/budgeted").to_request(),
    ));
    tokio::select! {
        _ = fixture.reply_held() => {},
        _ = &mut budgeted => panic!("budgeted HTTP request must wait for Redis"),
    }
    let unrelated = tokio::time::timeout(
        Duration::from_secs(1),
        test::call_service(
            &app,
            test::TestRequest::get().uri("/unrelated").to_request(),
        ),
    )
    .await;
    fixture.gate.release.notify_one();
    assert!(budgeted.await.status().is_success());
    fixture.wait_state(0, 0).await;
    fixture.finish().await;
    assert!(
        unrelated
            .expect("same-thread HTTP progress")
            .status()
            .is_success()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn async_budget_cancelled_admission_reclaims_an_accepted_lease() {
    let _test = TESTS.lock().await;
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    let called = AtomicBool::new(false);
    fixture.pause_reply();
    let mut request = Box::pin(fixture.call().reserve_call(
        async |context| context.reserve_spend(1.0).await.map(Some),
        || async {
            called.store(true, Ordering::SeqCst);
            Ok(())
        },
    ));
    tokio::select! {
        _ = fixture.reply_held() => {},
        _ = &mut request => panic!("request cannot finish before Redis replies"),
    }
    assert_eq!(fixture.state("provider").await, (0, 1_000_000_000));
    assert_eq!(fixture.state("model").await, (0, 0));
    drop(request);
    fixture.gate.release.notify_one();
    fixture.wait_state(0, 0).await;
    assert!(!called.load(Ordering::SeqCst));
    fixture.finish().await;
}

#[tokio::test(flavor = "current_thread")]
async fn async_budget_cancelled_queued_admission_never_reaches_redis() {
    if run_in_isolated_process("async_budget_cancelled_queued_admission_never_reaches_redis").await
    {
        return;
    }
    let _test = TESTS.lock().await;
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    let baseline = fixture.gate.commands.load(Ordering::SeqCst);
    fixture.pause_reply();
    let mut active = Vec::new();
    for _ in 0..32 {
        let limits = Arc::clone(&fixture.limits);
        let provider = fixture.provider.clone();
        let model = fixture.model.clone();
        active.push(tokio::spawn(async move {
            limits.reserve_spend_async(&provider, &model, 1.0).await
        }));
    }
    fixture.reply_held().await;
    tokio::time::timeout(Duration::from_secs(3), async {
        while fixture.gate.commands.load(Ordering::SeqCst) != baseline + 32 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("all 32 budget workers must be waiting on Redis");
    let mut queued = Box::pin(fixture.limits.reserve_spend_async(
        &fixture.provider,
        &fixture.model,
        1.0,
    ));
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_millis(20)) => {},
        _ = &mut queued => panic!("all budget workers are occupied"),
    }
    drop(queued);
    fixture.gate.release.notify_one();
    for task in active {
        task.await.unwrap().unwrap().cancel_async().await.unwrap();
    }
    fixture.wait_state(0, 0).await;
    assert_eq!(
        fixture.gate.commands.load(Ordering::SeqCst),
        baseline + 32 * 4,
        "the cancelled queued request must issue no reserve or cancel command"
    );
    fixture.finish().await;
}

#[tokio::test(flavor = "current_thread")]
async fn async_budget_capacity_covers_drop_cleanup_until_redis_replies() {
    if run_in_isolated_process("async_budget_capacity_covers_drop_cleanup_until_redis_replies")
        .await
    {
        return;
    }
    let _test = TESTS.lock().await;
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    let mut reservations = Vec::new();
    for _ in 0..1024 {
        reservations.push(
            fixture
                .limits
                .reserve_spend_async(&fixture.provider, &fixture.model, 1.0)
                .await
                .unwrap(),
        );
    }
    // The synchronous SDK remains independent of async admission. Even when
    // all async slots are occupied, its accepted lease must retain the original
    // Drop cleanup behavior instead of becoming an orphaned pending identity.
    let limits = Arc::clone(&fixture.limits);
    let provider = fixture.provider.clone();
    let model = fixture.model.clone();
    let legacy_leases = tokio::task::spawn_blocking(move || {
        let reservation = limits.reserve_spend(&provider, &model, 1.0).unwrap();
        let leases = reservation.response_leases().unwrap();
        drop(reservation);
        leases
    })
    .await
    .unwrap();
    fixture.wait_state(0, 1_024_000_000_000).await;
    let mut control = fixture.control.clone();
    for (scope, name, lease) in [
        ("provider", &fixture.provider, legacy_leases.provider),
        ("model", &fixture.model, legacy_leases.model),
    ] {
        let (id, _) = lease.unwrap();
        let identities: Vec<Option<String>> = redis::cmd("HMGET")
            .arg(RedisPool::budget_lease_key(scope, name))
            .arg(&[format!("l:{id}"), format!("p:{id}")])
            .query_async(&mut control)
            .await
            .unwrap();
        assert_eq!(
            identities,
            [None, None],
            "legacy Drop must remove its lease"
        );
    }
    // The control connection observes application before its reply necessarily
    // reaches the budget connection. Drain that connection before arming a gate
    // which must observe the following async guard's cleanup.
    let limits = Arc::clone(&fixture.limits);
    let provider = fixture.provider.clone();
    let model = fixture.model.clone();
    tokio::task::spawn_blocking(move || {
        limits
            .reserve_spend(&provider, &model, 0.1)
            .unwrap()
            .cancel();
    })
    .await
    .unwrap();
    fixture.pause_reply();
    drop(reservations.pop());
    fixture.reply_held().await;
    let called = AtomicBool::new(false);
    let overloaded = fixture.call().reserve_call(
        async |context| context.reserve_spend(1.0).await.map(Some),
        || async {
            called.store(true, Ordering::SeqCst);
            Ok(())
        },
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(100), overloaded)
            .await
            .expect("saturation must fail before queueing")
            .is_err()
    );
    assert!(!called.load(Ordering::SeqCst));
    fixture.gate.release.notify_one();
    let replacement = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match fixture
                .limits
                .reserve_spend_async(&fixture.provider, &fixture.model, 1.0)
                .await
            {
                Ok(reservation) => break reservation,
                Err(BudgetReservationError::BackendUnavailable) => {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
                Err(error) => panic!("unexpected admission error: {error:?}"),
            }
        }
    })
    .await
    .expect("acknowledged cleanup must restore one admission slot");
    replacement.cancel_async().await.unwrap();
    for reservation in reservations {
        reservation.cancel_async().await.unwrap();
    }
    fixture.wait_state(0, 0).await;
    fixture.finish().await;
}

#[tokio::test(flavor = "current_thread")]
async fn async_budget_terminal_operations_survive_cancelled_waiters() {
    let _test = TESTS.lock().await;
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    let reservation = fixture
        .limits
        .reserve_spend_async(&fixture.provider, &fixture.model, 1.0)
        .await
        .unwrap();
    fixture.pause_reply();
    let facts = crate::core::request_ledger::SharedRequestLedgerFacts::new(std::sync::Mutex::new(
        Default::default(),
    ));
    let mut settlement = Box::pin(crate::core::request_ledger::scope_facts(
        facts.clone(),
        reservation.settle_async(0.25),
    ));
    tokio::select! {
        _ = fixture.reply_held() => {},
        _ = &mut settlement => panic!("settlement cannot finish before Redis replies"),
    }
    assert_eq!(fixture.state("provider").await, (250_000_000, 0));
    let snapshot = crate::core::request_ledger::snapshot_facts(&facts);
    let billing = snapshot.billing.unwrap();
    assert_eq!(
        billing.provider_settlement.as_deref(),
        Some("settlement_pending")
    );
    assert_eq!(billing.provider_charge_amount, None);
    drop(settlement);
    assert_eq!(
        crate::core::request_ledger::snapshot_facts(&facts)
            .billing
            .unwrap()
            .provider_settlement
            .as_deref(),
        Some("settlement_pending")
    );
    fixture.gate.release.notify_one();
    fixture.wait_state(250_000_000, 0).await;

    let reservation = fixture
        .limits
        .reserve_spend_async(&fixture.provider, &fixture.model, 1.0)
        .await
        .unwrap();
    fixture.pause_reply();
    let mut cancellation = Box::pin(reservation.cancel_async());
    tokio::select! {
        _ = fixture.reply_held() => {},
        _ = &mut cancellation => panic!("cancellation cannot finish before Redis replies"),
    }
    drop(cancellation);
    fixture.gate.release.notify_one();
    fixture.wait_state(250_000_000, 0).await;
    fixture.finish().await;
}

#[tokio::test(flavor = "current_thread")]
async fn async_budget_detached_settlement_preserves_billable_cost() {
    let _test = TESTS.lock().await;
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    let facts = crate::core::request_ledger::SharedRequestLedgerFacts::new(std::sync::Mutex::new(
        Default::default(),
    ));
    let reservation = fixture
        .limits
        .reserve_spend_async(&fixture.provider, &fixture.model, 1.0)
        .await
        .unwrap();
    fixture.pause_reply();
    let started = std::time::Instant::now();
    reservation.settle_detached(0.6, Some(facts.clone()));
    assert!(started.elapsed() < Duration::from_millis(100));
    fixture.reply_held().await;
    let billing = crate::core::request_ledger::snapshot_facts(&facts)
        .billing
        .unwrap();
    assert_eq!(
        billing.provider_settlement.as_deref(),
        Some("settlement_pending")
    );
    assert_eq!(billing.provider_charge_amount, None);
    fixture.gate.release.notify_one();
    fixture.wait_state(600_000_000, 0).await;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let billing = crate::core::request_ledger::snapshot_facts(&facts)
                .billing
                .unwrap();
            if billing.provider_settlement.as_deref() == Some("settled") {
                assert_eq!(billing.provider_charge_amount, Some(0.6));
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    fixture.finish().await;
}

#[tokio::test(flavor = "current_thread")]
async fn async_budget_durable_settlement_survives_cancel_and_replay() {
    let _test = TESTS.lock().await;
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    let reservation = fixture
        .limits
        .reserve_spend_async(&fixture.provider, &fixture.model, 1.0)
        .await
        .unwrap();
    let leases = reservation.response_leases().unwrap();
    reservation.detach_response();
    fixture.pause_reply();
    let mut settlement = Box::pin(fixture.limits.settle_response_leases_async(
        &fixture.provider,
        &fixture.model,
        &leases,
        0.4,
    ));
    tokio::select! {
        _ = fixture.reply_held() => {},
        _ = &mut settlement => panic!("durable settlement still awaits Redis"),
    }
    drop(settlement);
    fixture.gate.release.notify_one();
    fixture.wait_state(400_000_000, 0).await;
    fixture
        .limits
        .settle_response_leases_async(&fixture.provider, &fixture.model, &leases, 0.4)
        .await
        .unwrap();
    fixture.wait_state(400_000_000, 0).await;
    fixture.finish().await;
}

#[tokio::test(flavor = "current_thread")]
async fn async_budget_admin_reset_waits_without_blocking_current_thread() {
    let _test = TESTS.lock().await;
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    fixture
        .limits
        .reserve_spend_async(&fixture.provider, &fixture.model, 1.0)
        .await
        .unwrap()
        .settle_async(0.4)
        .await
        .unwrap();
    fixture.pause_reply();
    let mut reset = Box::pin(
        fixture
            .limits
            .providers
            .reset_provider_budget_async(&fixture.provider),
    );
    tokio::select! {
        _ = fixture.reply_held() => {},
        _ = &mut reset => panic!("reset cannot finish before Redis replies"),
    }
    assert_eq!(fixture.state("provider").await, (0, 0));
    drop(reset);
    fixture.gate.release.notify_one();
    assert!(
        fixture
            .limits
            .models
            .reset_model_budget_async(&fixture.model)
            .await
            .unwrap()
    );
    fixture.wait_state(0, 0).await;
    fixture.finish().await;
}

#[tokio::test(flavor = "current_thread")]
async fn async_budget_settlement_starts_key_accounting_before_waiting_on_redis() {
    use crate::core::budget::{BudgetConfig, BudgetManager, BudgetScope};
    use crate::core::keys::{CreateKeyConfig, InMemoryKeyRepository, KeyManager};

    let _test = TESTS.lock().await;
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    let keys = KeyManager::new(InMemoryKeyRepository::new());
    let (key_id, _) = keys
        .generate_key(CreateKeyConfig {
            name: "async budget accounting".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
    let key_budgets = BudgetManager::new();
    let scope = BudgetScope::ApiKey(key_id.to_string());
    key_budgets
        .create_budget(scope.clone(), BudgetConfig::new("async key budget", 10.0))
        .await
        .unwrap();
    let key_reservation = key_budgets.tracker().reserve_spend(&scope, 0.7).unwrap();
    let provider_reservation = fixture
        .limits
        .reserve_spend_async(&fixture.provider, &fixture.model, 1.0)
        .await
        .unwrap();

    fixture.pause_reply();
    let mut settlement = Box::pin(super::spend::record_reserved_spend_without_usage(
        &keys,
        Some(key_id),
        &fixture.provider,
        &fixture.model,
        Some(provider_reservation),
        Some(key_reservation),
        "async cancellation regression",
    ));
    tokio::select! {
        _ = fixture.reply_held() => {},
        _ = &mut settlement => panic!("provider settlement must wait for Redis"),
    }
    // The original usage write starts in the same poll as Redis settlement.
    // It must not be postponed until a new I/O await has completed.
    let stats = keys.get_usage_stats(key_id).await.unwrap();
    assert_eq!(stats.total_requests, 1);
    assert!((stats.total_cost - 0.7).abs() < f64::EPSILON);
    drop(settlement);
    assert!((key_budgets.get_current_spend(&scope) - 0.7).abs() < f64::EPSILON);
    fixture.gate.release.notify_one();
    fixture.wait_state(1_000_000_000, 0).await;
    let stats = keys.get_usage_stats(key_id).await.unwrap();
    assert_eq!(stats.total_requests, 1);
    assert!((stats.total_cost - 0.7).abs() < f64::EPSILON);
    fixture.finish().await;
}
