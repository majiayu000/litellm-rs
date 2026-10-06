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
must not delete pending fields on a timer while still accepting late costs. Redis
eviction, administrative key deletion and reservations already reclaimed by an
older binary cannot be recovered by this change. A rolling upgrade is complete
only once all replicas use the new expiry/reset behavior.

Regression coverage uses the production Lua script against real Redis, with
synthetic timestamps for exact expiry, duplicate finish, cancellation, never-reset
budgets, rollover and manual reset. A manager-level test also keeps a request alive
past its short test lease, creates a new reservation through another manager, and
verifies that late settlement records actual spend without releasing the new hold.
