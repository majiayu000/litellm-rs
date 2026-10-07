# Gateway deployment-profile benchmark

This extends the [bare gateway baseline](gateway-overhead.md) with two real
`gateway` processes, JWT authentication and canonical session/user lookup,
provider **and** model budget leases in one owned Redis service, and a shared
file-backed SQLite terminal request ledger. It then measures long SSE streams
with consumers reading more slowly than the synthetic upstream produces data,
and closes live client sockets to measure cancellation.

The local OpenAI-compatible upstream is deterministic synthetic traffic.
These are real gateway, Redis and SQL measurements; they are not real model
inference, provider billing, Internet latency or user acceptance evidence.
SQLite is a supported deployment backend. Results do not establish PostgreSQL,
Redis Cluster or remote multi-host performance. API-key budget accounting is
not exercised; provider/model accounting is shared through Redis.

## Run

Prerequisites: macOS/Linux, Python 3 with PyYAML (`python3 -m pip install PyYAML`,
this session uses PyYAML 6.0.3), Docker with a local Redis image,
and a gateway built from the revision under review. No external credentials,
shared Redis endpoint or existing database is accepted by the runner. It starts
an isolated disposable Redis container on a loopback port and creates a temporary
SQLite database. Only its own processes/container and temporary files are removed.
Existing services and host settings are untouched.

Build once, separately from the run (avoid concurrent Rust builds):

```bash
cargo build --locked --release --bin gateway --no-default-features \
  --features sqlite,redis,metrics,tracing

scripts/bench/deployment_profile.py \
  --binary target/release/gateway \
  --build-description 'cargo build --locked --release --bin gateway --no-default-features --features sqlite,redis,metrics,tracing' \
  --output artifacts/benchmarks/deployment-profile-20261007
```

The runner records the actual binary SHA-256, source HEAD/dirty status, runner
hash, caller's build description, configuration (with ephemeral secrets removed),
Python/PyYAML/Docker versions, Redis version and immutable image ID. A dirty
source tree is explicit; for release claims supply the binary built from the
exact reviewed revision. Use an otherwise idle host, retain every run, and use
identical build flags/workloads for comparisons. The driver, synthetic upstream
and gateways compete for host CPU; Docker adds its local VM/network overhead.

Two gateways each use two HTTP workers, warning-level logs, share SQLite and
Redis, and authenticate
separately using temporary registered administrator sessions. The runner checks
that unauthenticated inference is rejected, installs provider/model budgets on
both nodes, performs ten warmup requests, and checks the authenticated ledger
query. It uses the ledger `fail` write policy so unary SQL failures surface as
request errors. Streaming persistence errors may occur after response commitment;
missing terminal rows remain explicitly reported.

Default measured workloads:

| Workload | Client behavior |
| --- | --- |
| Auth + shared budget + ledger | 60 seconds, eight clients divided across two nodes, closed loop |
| Long SSE + slow consumer | Four concurrent streams, upstream sends 4 KiB content every 50 ms for 60 seconds; clients read at most 4 KiB then wait 100 ms |
| Cancellation | Four concurrent slow consumers close their actual socket after two seconds; observe upstream disconnect and terminal ledger row for at most 30 seconds |

The streaming `max_tokens=8192` is an admission input, not a claim about generated
model tokens. The synthetic upstream emits fixed token usage at completion.
The route uses `gpt-4` with the embedded price catalog and a visibly synthetic
local upstream API key, avoiding fallback-pricing logs in the measured path.
Completed SSE must contain `[DONE]`; EOF alone is a recorded error. Intentional
cancellation is separate from transport/status failures.

## Metrics and limits

`result.json` contains every client sample, HTTP status and transport exception
class, nearest-rank latency p50/p95/p99, SSE time to first body byte, throughput,
error rate, sampled RSS per gateway and combined peak. Four streaming samples
are a small-sample observation; their percentiles are not population tail-latency
estimates. Clients use a fresh HTTP/1.1 connection per request, so request latency
includes local connection establishment. RSS comes from `ps` at
roughly 100 ms intervals and excludes Redis, driver and the synthetic upstream;
short-lived peaks may be missed. Each sample also retains host load averages and
the sum of OS-reported process CPU percentages from `ps -A -o pcpu=`. This recent
CPU estimate may exceed 100 across cores; it does not expose command arguments or
process identities and is not an interval CPU accounting measurement. Post-cancellation RSS is a snapshot rather than
a proof that the allocator returned memory to the OS.

Cancellation reports two distinct observations: the mock sees upstream socket
disconnection, and SQL contains the terminal request row. Upstream detection is
limited by the 50 ms send interval and kernel buffering. Ledger latency is an
upper bound from 25 ms polling after clients close. Neither is a measurement of
provider-side compute cancellation. A null recovery value means it was not
observed before the configured deadline; the upstream outcome is retained.
Unknown final costs must remain unknown, with the corresponding budget liability
visible in the Redis state. The runner does not estimate actual cost for cancelled
streams or call a budget hold release proof of correct financial reconciliation.

The cancellation fixture intentionally closes before the synthetic upstream's final
usage frame. Outside the performance window, the runner decodes SQL billing JSON
and queries the existing admin ledger endpoint for each cancelled request. It
records checks that cost remains null, reason/waiting duration are visible,
provider and model each acknowledge their own reserved estimate as budget
responsibility, and SQL/admin billing agree. These scopes are never summed.
No supplier verification is fabricated or submitted. These checks cover this chat
SSE fixture; the added Realtime session subtotal/count fields are retained when
present but this workload does not exercise Realtime.

Raw Redis budget hashes and measured ledger rows are retained, allowing reviewers
to inspect actual settlement and unresolved liabilities. Temporary JWT secrets,
passwords, tokens and the authentication database are not published. A new output
directory is required; failed runs also preserve available measurements and logs.
The runner exits nonzero for request errors, missing ledger rows or failed
cancellation billing observations. A cancellation
recovery deadline miss is preserved as a measured result rather than hidden by
an aggregate throughput number.

Do not infer a speedup from a single run. For regression comparison, repeat on the
same host/profile before and after with both raw artifacts; no performance gain is
claimed by implementing this benchmark.

This session runs on a shared active host with other repository builds. Any
collected result is exploratory deployment-configuration evidence. Host load/CPU
samples make contention visible; an otherwise idle repeat is needed before
publishing an absolute-performance or regression claim.
