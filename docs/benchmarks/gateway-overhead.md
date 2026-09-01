# Reproducible gateway-overhead benchmark

This benchmark measures the HTTP gateway path around a deterministic local
OpenAI-compatible upstream. It does not measure model inference, public network
latency, provider quality, or a production configuration.

The workload exercises request parsing, routing, provider HTTP transport,
response parsing, and response serialization. Authentication, caching, rate
limits, metrics, tracing, storage, and guardrails are disabled in the dedicated
configuration so the result has one stated boundary.

## Prerequisites and exact command

- macOS or Linux
- the repository's Rust toolchain
- `python3`, `curl`, and `jq` (`python3 -VV` is recorded because the mock is
  part of the measured path)
- [`oha` 1.16.0](https://github.com/hatoo/oha/releases/tag/v1.16.0)

Install the pinned load-generator release and run from a clean checkout:

```bash
cargo install oha --version 1.16.0 --locked
scripts/bench/run_gateway_overhead.sh \
  artifacts/benchmarks/gateway-overhead-$(date -u +%Y-%m-%dT%H%M%SZ).json
```

The script builds `gateway` with the recorded `build_flags` of `--release --bin
gateway`, validates the benchmark config, starts the deterministic mock and
gateway, runs a 10-second warmup, then measures for 60 seconds at concurrency
64. It rejects unrecorded Cargo/Rust environment overrides and Cargo
configuration outside the checked-out repository. It fails if the Git tree or
HEAD changes, the exact tool version differs, either benchmark port is already
owned, a spawned service exits or misses its bounded readiness deadline, the
output artifact exists, or any request fails or returns a status other than
HTTP 200.

Keep the load generator, gateway, and mock on an otherwise idle machine. Record
every run rather than selecting the best result. For comparisons, use the same
host, toolchain, workload, and power settings. The load generator and gateway
share CPU in this baseline, so the result is reproducible local-system evidence,
not an absolute gateway ceiling.

## Pull-request regression reports

The `Gateway overhead benchmark` workflow measures the pull request's current
base SHA and GitHub's exact synthetic merge SHA sequentially on one
`ubuntu-24.04` runner. This keeps intervening base-branch changes on both sides
of the comparison. Both runs force `RUSTUP_TOOLCHAIN=1.96.1`, use oha 1.16.0,
and use the deterministic mock above. Before measuring, the workflow requires
the runner, mock, and benchmark config to be byte-identical in both checkouts;
a pull request that changes the harness must validate that change separately
instead of producing an incomparable regression report. The two raw artifacts
and `comparison.json` are uploaded together, so a report remains tied to the
commits and environment that produced it. The current base checkout's
comparator owns validation, thresholds, and the verdict; only the pull request
that first introduces the comparator uses the candidate copy when the base
file does not yet exist.

The initial policy is deliberately report-only. A measured regression exits the
comparator with status 10, which the workflow renders as a warning and a job
summary without blocking the pull request. Invalid JSON, mismatched workload or
environment metadata, missing tools, build failures, and benchmark failures use
a different nonzero status and fail the workflow. Missing capture timestamps or
raw oha evidence, any non-200 response, any transport error, and any nonzero
summary error rate also make the comparison invalid rather than passing as a
performance result.

The provisional noise allowance reports a regression when throughput drops by
more than 10% or any recorded latency percentile (`p50`, `p95`, or `p99`)
increases by more than 15%. These are alert thresholds, not published
performance claims. Keep the base and candidate artifacts when investigating a
warning; do not compare results from different environments.

Promote the workflow to a blocking required check only after at least 20
successful report-only comparisons have been collected, every warning in that
sample has been immediately rerun against the same two SHAs, and the latest 10
runs contain no benchmark/tool infrastructure failure. Before promotion,
maintainers must review the reruns and adjust the provisional thresholds so a
non-reproducing warning is not made blocking. Promotion is the small workflow
change that stops accepting comparator status 10; evidence or tool failures are
already blocking.

## Deterministic upstream boundary

[`mock_openai.py`](../../scripts/bench/mock_openai.py) accepts only local HTTP
requests and returns the same compact chat-completion bytes for every valid
`POST /v1/chat/completions`. Its ID, timestamp, token usage, and content are
fixed, and it injects no delay. The gateway is configured by
[`gateway-overhead.yaml`](../../scripts/bench/gateway-overhead.yaml) to route
only `benchmark-model` to that mock.

This isolates gateway overhead from model and Internet latency. It does not
subtract direct-to-mock latency: the published value is the complete observed
gateway request latency under this stated topology.

## Raw artifact format

The output is JSON with stable top-level metadata, a concise result summary,
and the unmodified `oha_raw` JSON. A result is publishable only with all fields
below. `latency_ms` is end-to-end client-observed latency.

```json
{
  "schema_version": 1,
  "captured_at": "2026-08-31T00:00:00Z",
  "source": {
    "git_sha": "full Git commit SHA",
    "git_dirty": false,
    "build_flags": ["--release", "--bin", "gateway"]
  },
  "environment": {
    "hardware": {
      "cpu_model": "recorded CPU model",
      "logical_cpus": 8,
      "memory_bytes": 17179869184
    },
    "os": {"name": "Darwin", "release": "...", "architecture": "arm64"},
    "rust": "complete rustc -Vv output",
    "cargo": "cargo version",
    "python": "complete python3 -VV output",
    "oha": "oha 1.16.0"
  },
  "workload": {
    "concurrency": 64,
    "warmup_seconds": 10,
    "duration_seconds": 60,
    "request_bytes": 88,
    "response_bytes": 389,
    "protocol": "HTTP/1.1 keep-alive",
    "route": "POST /v1/chat/completions",
    "upstream": "deterministic local mock, fixed response, zero injected delay"
  },
  "results": {
    "requests_per_second": 0.0,
    "latency_ms": {"p50": 0.0, "p95": 0.0, "p99": 0.0},
    "error_rate": 0.0
  },
  "oha_raw": {}
}
```

Throughput is `requests_per_second`; latency reporting must include `p50`,
`p95`, and `p99`; `error_rate` is `1 - oha_raw.summary.successRate`. The raw
payload retains the status-code and transport-error distributions needed to
audit that summary.

## Publishing claims

A README throughput or latency number must link to a dated artifact produced by
this command and identify its Git SHA. Criterion microbenchmarks in `benches/`
are valuable for regression investigation, but they are not evidence for
end-to-end gateway RPS or HTTP latency claims.
