# Runtime reload construction profile

This is an opt-in measurement of `AppState::apply_runtime`, including candidate
validation, router/provider construction, runtime-state inheritance, and
publication. It records logical provider-instance changes separately from
elapsed time. The profiling change does not implement provider or HTTP-client
reuse.

## Reproduce

Use the repository toolchain and run from the repository root:

```bash
CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --locked --features sqlite \
  --lib profile_runtime_reload_construction \
  --config profile.test.package.litellm-rs.codegen-units=256 \
  --config profile.test.package.litellm-rs.debug=0 \
  -- --ignored --nocapture --test-threads=1
```

The profile is ignored during normal tests because it is a measurement, not a
latency threshold. Each `RUNTIME_RELOAD_PROFILE` line contains one scenario's
raw samples, nearest-rank p50/p95, maximum, and instance counts. A successful
run emits twelve records: 1, 10, and 100 providers, each with routing-policy,
credential, endpoint, and timeout changes. Every record discards three warmup
applies and retains thirty measured applies.

The [profile source](../../src/server/runtime_reload_profile.rs) constructs
real `HttpServer` state with one explicit OpenAI model per provider, synthetic
credentials, and no upstream request. Database, Redis, pricing downloads,
response-cache tasks, and provider probes are disabled. An assertion confirms
that configuration synchronization and the response cache are absent. A
single-thread Tokio runtime runs each scenario sequentially.

Each iteration clones and changes the current configuration before starting
the timer. The timer surrounds only `state.apply_runtime(candidate).await`.
The preceding runtime revision remains pinned until after timing and identity
inspection, as an in-flight request may pin an old generation. Initial server
construction, candidate cloning, identity inspection, and dropping the old
revision are outside the measured interval. There is no competing reload or
request in this workload.

Routing-policy changes adjust weight and RPM for every provider, and toggle a
single alias; the alias map does not grow across samples. The other scenarios
change every provider's synthetic credential, endpoint path, or timeout while
keeping routing policy fixed. Every apply must publish exactly one new
generation. Credential, endpoint, and timeout changes must produce distinct
logical provider instances.

The count comes from `ProviderInstanceIdentity`, allocated once after each
successful canonical provider-factory call and retained independently of
quota-state inheritance. One model per provider makes the count unambiguous.
It does not count socket creation, live TCP connections, DNS queries, TLS
handshakes, connection-pool hits, or request latency. The OpenAI constructor
builds policy-bound ordinary and streaming clients, but this workload never
sends a request through them.

## Recorded measurement

The recorded run used production code from
`3cb5e0499aa437b15784c78fc4cd34010468bb35` plus this test-only profile and
module registration. The [raw JSON](runtime-reload-construction-20261007.json)
contains all 360 samples, source and lockfile SHA-256 digests, exact command,
compiler metadata, and host limits. The cargo invocation ran on 2026-10-07 from
09:13:44 to 09:20:15 UTC, including compilation; the profile test itself took
104.97 seconds and passed. All 360 measured applies rebuilt every logical
provider, totaling **13,320 new logical instances**.

The host was a shared Linux 6.18.44 x86-64 container on Intel Xeon Platinum
8573C, with 9 visible/affinity CPUs, an 8-CPU cgroup time quota and 8 GiB memory
limit. Rust/Cargo 1.96.1 compiled `default + sqlite` in the unoptimized test
profile. Only this package's debug information was disabled and its codegen
units set to 256; debug assertions remained enabled. Incremental compilation
was disabled, with one Cargo job and no RUSTFLAGS override. Initial and final
host load observations in the JSON surround the whole cargo invocation, not
individual samples.

| Providers | Change | p50 (ms) | p95 (ms) | Max (ms) | New logical instances per apply |
| ---: | --- | ---: | ---: | ---: | ---: |
| 1 | routing policy | 208.296 | 293.241 | 298.486 | 1 / 1 |
| 1 | credential | 193.607 | 219.629 | 226.146 | 1 / 1 |
| 1 | endpoint | 269.489 | 469.653 | 541.645 | 1 / 1 |
| 1 | timeout | 200.805 | 397.199 | 419.559 | 1 / 1 |
| 10 | routing policy | 210.511 | 423.926 | 549.085 | 10 / 10 |
| 10 | credential | 200.837 | 297.983 | 329.700 | 10 / 10 |
| 10 | endpoint | 343.058 | 728.097 | 1019.630 | 10 / 10 |
| 10 | timeout | 201.520 | 217.129 | 221.047 | 10 / 10 |
| 100 | routing policy | 223.177 | 410.452 | 585.598 | 100 / 100 |
| 100 | credential | 218.724 | 257.373 | 277.874 | 100 / 100 |
| 100 | endpoint | 216.150 | 358.492 | 398.706 | 100 / 100 |
| 100 | timeout | 291.531 | 665.848 | 695.442 | 100 / 100 |

These are one process's sequential samples, in provider-count and scenario
order, on a shared host. The wide tails and non-monotonic endpoint medians
limit comparisons. The profile does not identify which apply substep consumes
time, isolate constructor cost, provide confidence intervals, or estimate a
release-build latency. It excludes persistence, peer notification, active
traffic and network handshakes. It is a reproducible baseline with explicit
scope, not a production reload SLO or a speedup measurement.


## Construction-reuse decision

Keep this change scoped to measurement. Routing-policy p50 was 208.296,
210.511 and 223.177 ms at 1, 10 and 100 providers, respectively, despite every
logical provider being reconstructed. That result establishes the repeated
construction mechanism but does not establish substantial latency savings
from client reuse. Before prioritizing a transport cache, measure the
validation, router/provider, guardrail and publication substeps separately
with a release build and a relevant reload frequency/latency requirement.

The existing runtime-state continuity implementation shares matching live
quota counters while leaving fresh provider clients owned by the candidate
revision. The canonical factory still runs before
`UnifiedRouter::inherit_runtime_state`; quota continuity alone cannot skip
that construction.

A future reuse experiment should be scoped to the preceding successful
revision and use a distinct key for normalized constructor inputs. The
existing `GatewayRuntimeIdentity` represents quota ownership and deliberately
excludes timeout, retry, routing, and model-list inputs. Using it as a transport
reuse key could retain an old timeout or other constructor policy. The
provider factory consumes selector, resolved credentials, endpoint/access
policy, API version, organization/project, timeout, retries, settings, and in
some branches model lists; an initial conservative key must account for all
of those inputs.

Such a change also needs lifecycle tests proving that old in-flight requests
keep their original provider, failed candidates do not retire live resources,
and credential/endpoint/timeout changes select the new policy. The measured
profile can then compare the same before/after workload and logical instance
counts. Real connection reuse and streaming behavior require a separate local
HTTP/SSE workload. This profile supplies no before/after speedup or transport
reuse acceptance result.
