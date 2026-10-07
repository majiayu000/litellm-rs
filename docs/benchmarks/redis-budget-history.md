# Redis budget history growth

## What was measured

The production `BUDGET_LEASE_SCRIPT` from commit
`114f5cae230408320d68f7e299d58569fde8c4b5` was executed against an owned,
loopback-only Redis 7.2.7 instance. The deadline changes do not change this Lua.
The driver varies live/pending identities (half each) and durable Responses
receipts independently. Each cell uses ten warmup reserve/settle pairs followed
by 200 measured pairs. A separate connection sends PING with a 1 ms pause to
expose delays experienced by another Redis client.

Before measurement, the driver checks late actual settlement, duplicate
settlement, durable receipt replay, rejection at 65,536 unfinished identities,
and admission after terminal release. Every measured pair checks the returned
committed/outstanding counters; the final hash field count must match its seed.
The raw record includes source, Lua, runner and Redis-binary SHA-256 values,
timestamps, platform, individual samples and memory sizes.

## Recorded run: 2026-10-07

Shared Linux cloud executor, 9 reported logical CPUs, Redis built with libc.
Other review jobs were active on this host. These are raw Lua loopback timings,
not gateway HTTP latency, production QPS, or a controlled before/after speedup.
In particular, host scheduling affects the high percentiles. Raw samples are in
[`redis-budget-history-20261007.json`](redis-budget-history-20261007.json).

| Unfinished | Receipts | Reserve p50 ms | Reserve p95 ms | Reserve p99 ms | Settle p95 ms | Concurrent PING p99 ms | Hash MiB |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1,000 | 0 | 0.913 | 2.455 | 5.501 | 2.611 | 5.728 | 0.077 |
| 1,000 | 10,000 | 4.953 | 10.110 | 14.086 | 8.072 | 11.229 | 0.881 |
| 1,000 | 65,000 | 41.771 | 282.893 | 1068.500 | 388.130 | 1066.480 | 6.032 |
| 10,000 | 0 | 10.060 | 19.081 | 24.616 | 19.433 | 22.613 | 0.812 |
| 10,000 | 10,000 | 16.458 | 37.124 | 65.800 | 33.110 | 63.624 | 1.749 |
| 10,000 | 65,000 | 46.260 | 85.590 | 123.387 | 76.332 | 127.031 | 6.650 |
| 65,000 | 0 | 76.003 | 136.082 | 147.719 | 128.817 | 228.460 | 4.964 |
| 65,000 | 10,000 | 90.920 | 147.188 | 204.639 | 130.207 | 203.545 | 6.650 |
| 65,000 | 65,000 | 146.958 | 272.251 | 406.890 | 228.459 | 498.269 | 9.927 |

## Engineering decision

The scan cost grows with both unfinished identities and completed durable
receipts. The unfinished-identity capacity limit therefore does not cap hot-path
work. A separate receipt retention/index design is justified; adding command
deadlines only makes waits finite and does not make the scan cheaper.

This change records the baseline and preserves the accounting format. Splitting
hot counters/expiry into an index and moving receipt storage needs a versioned
migration and replay protocol: all Lua keys must share one Cluster slot; old
writers must not silently bypass the new index; old pending identities must
continue accepting late costs; and receipt deletion must wait for the SQL
recovery horizon. An uncoordinated TTL or hash-field cleanup would invalidate
those guarantees. A subsequent implementation must rerun these correctness
cases and the same growth matrix before claiming improvement.

## Reproduction

Build the desired Redis version and run this from a clean checkout:

```bash
python3 scripts/bench/redis_budget_history.py \
  --redis-server /absolute/path/to/redis-server \
  --output /absolute/path/to/redis-budget-history.json
```

The runner starts and terminates only its own Redis child. It verifies the
server's reported process ID before seeding any data. It accepts no arbitrary
shared Redis URL and does not touch an existing service. It requires a free
loopback TCP port and no external Python dependencies. Source cleanliness is
checked for the production Lua file, and the exact checked-out commit is
recorded. Reopening or clearing a client connection is not an OS-cold-cache run;
this benchmark makes no cold-storage claim.
