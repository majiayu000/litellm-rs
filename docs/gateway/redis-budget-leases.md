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

Routing outcomes and dollar-budget ownership are separate. Once a unary provider
has returned a successful response, cancellation during its owned dollar-budget
settlement retains the selected deployment's local success, RPM/TPM and actual
routing-admission usage. The dollar worker keeps its existing settlement identity.
A later conversion error on a normally completed operation remains an error;
observed usage alone does not imply provider success. Allowed unpriced settlement
remains a successful accounting policy rather than being relabeled a provider error.

Gateway stream completion scopes begin only after EOF or a known terminal result.
Cancelling that accounting wait records the known success, typed provider failure,
or neutral interruption once and finalizes the original lease, even when its
caller retains it. Completing the wait normally leaves outcome recording to the
existing terminal method. Failure counters and known tokens are recorded before
shared admission or circuit I/O. These scopes retain the selected deployment
object and hold; replacing a deployment under the same ID cannot transfer its
outcome to the replacement. Intermediate usage does not terminate a stream.
A terminal provider failure with zero observed tokens cancels ordinary stream
admission and refunds its reserved RPM. Realtime interrupted failures retain their
existing request-usage contract, including shared RPM when actual tokens are zero.
Both normal completion and cancelled accounting waits use the same decision.

This change bounds queued work and keeps the HTTP executor responsive. It does
not add or change Redis deadlines. With the currently locked `redis` 1.2.0,
standalone connections inherit `AsyncConnectionConfig`'s 500 ms response timeout;
connection setup uses the configured connection timeout. The current Cluster
builder sets a connection timeout but leaves response and overall response
timeouts unset. In that mode a connected server which never replies can retain
worker and reservation slots until I/O fails or resumes. The bridge adds no
end-to-end recovery deadline or guarantee that terminal cleanup finishes within
a fixed duration. A lost Redis reply still leaves the write result uncertain;
the bridge does not replay admissions and preserves the existing client settings,
remote capacity expiry and pending-identity rules.

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
