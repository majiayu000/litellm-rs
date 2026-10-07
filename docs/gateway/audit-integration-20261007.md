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

#1459 and #1465 are already ancestors of the selected #1466 revision. Each
additional source is retained as a merge parent. The final candidate tree and
its CI checkout identity are recorded in the integration PR.

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
