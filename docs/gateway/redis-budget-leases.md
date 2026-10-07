# Redis provider/model budget leases

Issue #1452 separates the lifetime of reserved capacity from the right to record
the actual cost of an ordinary request. It does not change API-key accounting or
the native Responses SQL recovery contract.

Each reservation has a UUID and a live `l:<UUID>` field in the existing budget
hash. Its ten-minute capacity lease contributes to outstanding spend. Expiry,
period rollover, or an explicit budget reset releases that capacity and moves the
field to `p:<UUID>`. The pending field holds no outstanding capacity; it proves
that the backend accepted a reservation which has not yet finished.

An ordinary settlement atomically consumes either the live or pending field and
adds the supplied actual cost. Duplicate settlements and settlements of unknown
IDs do nothing. Late actual cost is attributed to the currently stored budget
period, consistent with the in-process backend. It never releases capacity owned
by a newer reservation, and a late operation cannot rewind the period.

Explicit cancellation consumes both field forms without adding spend. If cancel
and settle race, the first Lua operation wins; cancellation after settlement never
refunds committed spend. Callers must only cancel when their existing execution
contract allows it. Expiry alone does not assert that the provider used zero
tokens and does not turn an estimated reservation into an actual charge.

The budget hash has no lease-derived key TTL. Pending fields survive subsequent
period resets until an explicit settlement or cancellation consumes them. Ordinary
success does not leave a permanent per-request receipt, while durable Responses
still uses its existing `r:<UUID>` receipt for retry after SQL recovery and clears
any pending marker when it settles.

Pending metadata for a permanently abandoned request remains until reconciliation
or explicit cancellation. This preserves late-settlement correctness; operators
must not delete pending fields on a timer while still accepting late costs.

Each budget permits at most 65,536 unfinished reservation fields (`l:` plus `p:`).
The existing reclamation scan counts these fields without another hash scan.
At this limit, a new reservation fails closed with the infrastructure error
`BudgetReservationError::BackendUnavailable`, carrying the metadata-capacity
reason in the backend log. It follows existing backend-unavailable handling,
rather than reporting that the monetary budget has been exhausted. The rejected
reservation adds no outstanding spend or reservation field.

Settlement and cancellation remain available at or above the limit and release
identity capacity. Expiry, period rollover and manual budget resets do not free
identity slots; an actual charge can still arrive after 24 hours or after a reset.
An existing hash above the limit accepts no new reservations until terminal
operations reduce its unfinished field count below the limit. Permanently
abandoned reservations therefore require reconciliation before admission can
resume. Ordinary completed requests leave no receipt and do not consume this
capacity.

The limit applies only to unfinished reservations. Durable Responses `r:` receipts
are excluded and retain their existing SQL recovery and replay semantics, even
when the unfinished reservation capacity is full. This change does not bound the
entire budget hash or its scan cost when durable receipts accumulate.

The dedicated budget connection is replaced after an unrecoverable Redis
connection error. Concurrent reconnects share one new connection, and a delayed
error from an older connection cannot evict its replacement. Script errors keep
the healthy connection. The failed operation is returned to its caller without
automatic replay: a lost response can follow a successfully applied write, so
reconnecting does not prove that a reservation or settlement was never applied.
Subsequent operations reconnect while preserving the existing receipt and
reservation-ID rules.

## Async gateway execution

The synchronous provider/model budget SDK remains available. Gateway request
handlers use `reserve_spend_async`, `settle_async` and `cancel_async`; the pricing
helpers await the reservation before invoking a provider. Admin provider/model
resets use the same bounded worker bridge. Availability checks,
API-key in-process bookkeeping and local spend snapshots remain synchronous
memory operations.

The async bridge runs the existing accounting operations on a separate budget
SDK worker runtime, with at most 32 blocking workers. Redis I/O runs on its own
runtime so a full SDK worker pool cannot starve connection setup or DNS work.
A process-wide limit of 1,024 slots covers the async bridge's queued admissions,
active reservations, and terminal cleanup across all managers and Redis
endpoints. New admissions
fail immediately with `BackendUnavailable` when those slots are occupied; there
is no unbounded semaphore waiter queue. Each reservation can dispatch at most two
Drop cancellations, one for its provider and one for its model, and both retain
its original slot until their Redis calls finish. Synchronous SDK reservations
retain their existing blocking calls and Drop cleanup behavior; they do not
consume this new async capacity or lose cleanup when it is full. This process
capacity is separate from each budget hash's unfinished-identity limit.

Cancelling an HTTP future before its queued worker starts prevents that worker
from reserving. If Redis is already processing an admission, the worker retains
ownership through its reply. A successfully accepted reservation whose result
cannot be delivered is explicitly cancelled, including provider rollback when
the model refuses admission. Once delivered, its guards retain the slot for a
later settlement, explicit cancellation or Drop cleanup. Dropping a settlement
or cancellation waiter does not abort the already dispatched terminal operation.
In-process API-key reservations are settled before waiting on Redis, and the
existing usage write is polled alongside Redis settlement so its start is not
postponed by the new wait. This preserves the usage writer's existing cancellation
contract without adding an unbounded background accounting queue.
Realtime transport Drop dispatches its existing conservative cost settlement;
it does not turn potentially billable output into a cancelled reservation.
Durable Responses retain their SQL receipt ownership and idempotent replay.

Each HTTP-facing async reply has a 30-second deadline, including time queued
behind the SDK workers. Expiration closes the reply receiver. An admission still
queued then skips Redis; an admission already executing keeps its original
identity and cancels a successfully returned but undeliverable reservation.
Terminal work retains its slot and accounting ownership after the caller times
out. The HTTP deadline does not abort settlement or turn billable work into a
refund.

The dedicated budget connection has separate five-second bounds for acquiring
a Redis concurrency permit, waiting for connection creation, command execution,
and failed-generation invalidation. Whole connection setup uses the smaller of
the configured connection timeout and five seconds. The underlying standalone
and Cluster clients also receive an explicit five-second response timeout;
Cluster requests have a five-second overall timeout and zero automatic retries.
Consequently an uncertain write is not replayed by the driver. A Cluster topology
redirect fails the current operation closed and evicts that connection; a later
operation discovers a fresh mapping. Other Redis consumers retain their existing
client settings.

These are phase and caller-wait bounds, not a promise that every combined
provider/model operation succeeds within five seconds. A lost reply remains
uncertain even after timeout: remote capacity can expire while its pending
identity remains for reconciliation. Backend diagnostics retain the operation
and original lease ID. A failed actual settlement preserves that identity for
reconciliation and cannot fall through Drop to cancellation. Provider and model
settlement are both attempted, even if the first scope fails; the combined call
then reports the failure. Settlement-failure diagnostics include the original
lease ID, scope, name and actual amount, including failures before command I/O. The bridge does not invent an acknowledgement, retry a
reserve, or delete an uncertain remote identity. Durable Responses continue to
use the SQL-owned, idempotent receipt recovery path.

The new async SDK methods should be used as a pair: an async reservation carries
its terminal capacity. If a synchronous SDK reservation is passed to async
settlement while admission capacity is full, settlement reports unavailable and
conservatively leaves its remote identity for reconciliation instead of refunding
billable work.

Regression tests use an isolated TCP proxy on a separate thread to hold real
Redis replies after Lua has run. They check current-thread gateway progress,
caller cancellation before and after dispatch, 32 occupied workers, 1,024 live
reservation slots, retained Drop-cleanup capacity, terminal operations whose
waiters disappear, legacy SDK Drop while async admission is full, detached
billable settlement and durable replay. The proxy
has a test-only watchdog so an executor-blocking regression fails rather than
hanging the suite. The tests which saturate the process-wide reservation capacity
or worker pool execute in isolated libtest subprocesses, so they cannot consume
capacity needed by unrelated tests running in parallel.

Redis eviction, administrative key deletion and reservations already reclaimed by an
older binary cannot be recovered by this change. A rolling upgrade is complete
only once all replicas use the new expiry/reset behavior and reservation-capacity
check; older replicas do not enforce the new capacity limit.

Regression coverage uses the production Lua script against real Redis, with
synthetic timestamps for exact expiry, duplicate finish, cancellation, never-reset
budgets, rollover and manual reset. Capacity coverage seeds real Redis at and
above the production limit, preserves late settlement beyond 24 hours, and checks
that terminal operations restore admission while durable replay remains
idempotent. A manager-level capacity failure also verifies infrastructure error
classification and rollback of the provider reservation when the model is full.
A manager-level test keeps a request alive past its short test lease, creates a
new reservation through another manager, and
verifies that late settlement records actual spend without releasing the new hold.

## API-key budget capability boundary

| Scope | Definition and spend state | Restart / multiple gateways |
| --- | --- | --- |
| Provider/model with the distributed Redis backend | Configured limits plus Redis reservation/counter state | Shared reservation accounting under the documented lease and upgrade contract |
| API-key `BudgetManager` | Process-local definitions, counters and reservations | Not restored from SQL and not distributed through the provider/model Redis backend |
| Persisted API-key record | SQL record including its `budget_id` binding | The binding alone does not recreate a budget or its previous balance |

`POST /keys` and key updates reject `max_budget`. An embedded caller may create
a process-local budget and bind an API key to that existing `budget_id`; this
is a single-process capability. Missing budget bindings fail closed. Creating a
fresh budget after restart does not recover its previous spend. Applications
must not advertise a shared or restart-persistent API-key balance on this basis.

Extending this capability requires persisted budget definitions and scopes,
restoration and distribution, and a shared reservation ledger. Its acceptance
must use production authentication with real SQL and Redis: gateway A spends,
gateway B observes the same remaining balance, restarting A preserves accounting,
and concurrent/stream-interrupted/late-settled requests respect one total. That
capability is not introduced by the provider/model budget fixes.

## History growth measurement

The reproducible driver and recorded samples are described in
[the history growth benchmark](../benchmarks/redis-budget-history.md). It varies
unfinished identities independently from durable receipts and checks late
settlement, duplicate completion, durable replay and capacity rejection before
recording latency. A new timeout does not remove the Lua hash scan or bound
receipt retention.
