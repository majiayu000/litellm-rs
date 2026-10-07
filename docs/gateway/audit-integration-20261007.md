# Runtime audit candidate integration — 2026-10-07

The October audit requested validation of the fixes together. This draft
integration preserves the original implementation commits and combines them in
an explicit candidate so the complete CI can exercise their interactions.
Implementation discussion and issue ownership stay with the original PRs.

## Input revisions

| PR | Concern | Fixed source revision |
| --- | --- | --- |
| #1458 | Runtime-bound SDK embeddings | `d35b75cd407b61a7b1dd70be4b629edc7a662bd0` |
| #1459 | Independent async Redis budgets, bounded waits, settlement ownership | `3848322dcf6f9d3e788017b700b469ea690f0638` |
| #1460 | SSE terminal-state validation | `7fc7f25422279c0a2158bbcbba1e0605a08e2128` |
| #1461 | SDK tool-result identity and streamed tool calls | `8c516942e8a2a552371ab8a507d2e2524b9bb122` |
| #1462 | Credential grants for key management | `21097d5305759744bcecc45e86d5839e595b9a28` |
| #1463 | Missing/inactive team key lifecycle | `77cf4c2d179cd5d3cf961f78caa391a82a27afc2` |
| #1464 | Quota continuity and measured reload construction | `719f6dd10a9226c3ede839eae7d726abd45b2de8` |
| #1465 | Async Redis routing admission and circuit operations | `b804d4cff3c30cc3ee30cfebde6c69d176babd4f` |
| #1466 | Owned SDK/gateway completion, cancellation and budget settlement | `7ab5f1eb0e103b104116b802a58fa030d73b9ee3` |
| #1467 | Discarded configuration candidate cleanup ownership | `ca6e1848f188ffd5d9744b491bd96cb2ca44bcb7` |
| #1469 | Bounded live-key eviction metadata | `d9eff858c2d9e23494f2bd8729debfbb214736e1` |

#1459 and #1465 are already ancestors of the selected #1466 revision. Each
other source in the original ten-candidate combination is retained as a merge parent. The final candidate tree and
its CI checkout identity are recorded in the integration PR.

#1469 is added after the original ten-candidate combination by three linear,
DCO-signed cherry-picks with source trailers: `c25722778f3b3a9c2de9bed0b13ba894866974ab`,
`95a044baf6f7aa01e278ab53c78182af1d979865`, and
`d9eff858c2d9e23494f2bd8729debfbb214736e1`. The resulting implementation revision
is `3789703602b3ed87b03891be0beadfa92df9bc74`, descending from the original
combination `39fe86e2f5cb497107becee8123a64c3c430e4a8`.

## Cross-branch resolution

The explicit content conflict is in `chat_stream_with_runtime`. The merged
method takes #1461's complete `SdkChatRequest`, resolves the supplied model,
preserves its options/tools/tool choice, and then maps the selected deployment
to the provider model. It also retains #1466's `include_usage=true`, observed
usage tracking, and owned completion guard through failure, EOF and Drop.

An independent source review confirmed these two contracts remain present.
`tests/sdk_runtime_tools.rs` checks complete tool arguments, interleaved deltas,
the usage-only tail and request release. The budget and stream-completion
regressions separately check actual accounting and cancellation ownership.
Running both groups on this complete candidate is required before treating the
integration as verified.

The SDK stream-completion fixture from #1466 also supplies the optional
`tool_call_id` field added by #1461. Its user-message role and all accounting,
release, cancellation and terminal-outcome assertions remain unchanged.

The other merged files use Git's combined content without a manual behavioral
rewrite. Source review alone does not establish that their composition passes.

The cache test conflict only joins #1467's three cleanup-lifecycle tests with
#1469's moved hot-key regression. All original cache tests and all seven added
metadata regressions remain. Relative to #1469, `memory.rs` retains exactly
#1467's single-start, weak-ownership and persistent watch-shutdown changes;
the live-key candidate indexes, atomic hit bookkeeping, conditional expiry
cleanup and replacement checks are preserved. The other ten-candidate files
are unchanged by these cherry-picks.

## Verification scope

Local `cargo +1.96.1 fmt --all -- --check` passed after the initial combination.
The existing full main and shipped-gateway CI jobs run the complete candidate,
including their applicable integration tests. Strict Clippy, feature profiles,
default-library checks, disabled-module compilation and two-gateway convergence
must be read at that candidate's final head. Parent CI results remain evidence
for the parent revisions until those combined checks finish.

The Redis history-growth samples and runtime-reload construction profile are
diagnostic measurements. They do not establish production throughput, transport
reuse or a storage-format optimization. API-key budget definitions/counters
retain the documented process-local capability boundary.

At cache implementation revision `3789703602b3ed87b03891be0beadfa92df9bc74`,
local focused validation with `sqlite,websockets` passed:

- `cargo test --locked --features sqlite,websockets --lib core::cache::`:
  179 passed, 0 failed, 0 ignored, including all 34 in-memory-cache tests and
  the memory/dual-cache cleanup ownership and early-shutdown regressions.
- `cargo test --locked --features sqlite,websockets --test sdk_runtime_tools --test sdk_runtime_stream_completion --test sdk_runtime_embeddings`:
  tools 4, stream completion 8 and embeddings 4 passed, with no failures or ignores.
- `cargo check --locked --features sqlite,websockets --lib --bench in_memory_cache_benchmarks`
  and `cargo clippy --locked --features sqlite,websockets --lib --tests --bins --benches -- -D warnings --force-warn clippy::collapsible-if` passed.
- `cargo fmt --all -- --check` and `git diff --check` passed.

These focused checks use jobs=4, dev/test debug=0 and incremental=0, with no
repository profile changes. The existing `cancel_admission` helper emits a
dead-code warning in both sqlite-only benchmark builds because its callers
require `websockets`. The strict local check includes those callers, as the
gateway CI profiles do; the warning and original source are preserved.
Default and shipped-feature full suites, feature matrix and convergence must
run on the newly submitted combination in CI. Their previous successes apply
to the original combination, and are not new-combination verification.

## Cache throughput comparison for #1468

The unchanged `benches/in_memory_cache_benchmarks.rs` production-cache case
`in_memory_cache_hot_path/current_atomic_sampled_get_hits/4` was compiled at
the original combination `39fe86e2f5cb497107becee8123a64c3c430e4a8` and the cache
combination `3789703602b3ed87b03891be0beadfa92df9bc74` on the same Apple M3 Max
(16 CPUs, 128 GiB), macOS 26.5.1, Rust/Cargo 1.96.1 aarch64 toolchain:

```sh
cargo bench --locked --features sqlite --bench in_memory_cache_benchmarks -- current_atomic_sampled_get_hits
```

Both builds use the repository's unchanged optimized bench profile, inheriting
release opt-level=3, LTO and one codegen unit; no profile overrides were used. The exact
executables were retained and hashed, then run sequentially in three
before/after pairs after compilation finished. Each run uses the original
10 Criterion samples, 500 ms warmup and 2 s measurement target. The fixture
preloads 4,096 keys into capacity 8,192 with the original `cached-value` string,
LRU policy, eight Tokio workers and four concurrent tasks doing 512 gets each
(2,048 operations per iteration). Its original index pattern touches 803 keys.
Measured runs contain 14,981,120–22,302,720 timed get operations each.

| Pair | Before (Melem/s) | After (Melem/s) | Throughput change |
| --- | ---: | ---: | ---: |
| 1 | 8.039 | 9.403 | +16.97% |
| 2 | 7.961 | 8.647 | +8.62% |
| 3 | 8.912 | 10.904 | +22.35% |

These are throughput estimates derived from Criterion's latency slope.
All six measured processes exited successfully. This fixed hit fixture showed
no throughput regression in the observed runs. It includes task scheduling and
value cloning, and does not measure insertion, eviction, Redis, gateway traffic,
memory usage or production throughput. The shared workstation still had
background activity; only the coordinated agents' heavy builds were paused.
The short 10-sample runs and within-run confidence intervals do not establish
a general performance guarantee. Raw sample counts, estimates, 95% intervals,
binary hashes, commands and per-run process/load snapshots are retained under
`.codex/threads/20261007-pr-triage/round2/artifacts/litellm-rs/` in the triage
workspace, especially `alternating-summary.json` and `alternating/`.

The first unfiltered baseline run completed the production-cache case, then
the benchmark-local `legacy_global_mutex_lru_get_hits` reference stalled for
over 120 seconds during sample collection at 0% CPU. Its native stack sample
and incomplete log are retained; the owned benchmark child was terminated and
Cargo exited 101. Stripped symbols do not establish its cause. The acceptance
comparison above filters the same production-cache case in both original
executables; benchmark source and workload inputs were unchanged.
