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
