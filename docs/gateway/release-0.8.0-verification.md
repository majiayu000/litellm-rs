# 0.8.0 release verification (F18)

Issue #1403. Status: candidate preparation; no 0.8.0 tag or artifact has been
published yet. Version 0.8.0 is a pre-1.0 minor release because accepted changes
remove public APIs and change the default Cargo feature profile.

## Candidate scope

The candidate includes the accepted parity work through #1441, the remaining
model audit #1442 and unused-interface reconciliation #1443. The complete release
notes are in CHANGELOG.md. The immutable accepted commit and published artifact
identifiers will be recorded here after validation/publication; PR branch heads
are preparation evidence and are not substitutes for that commit.

The shared shipped profile is:

```text
postgres,sqlite,redis,s3,metrics,tracing,websockets,providers-extra,providers-extended,mcp-validation,a2a
```

Release CI tests and lints that exact profile with `--no-default-features`.
GitHub archives, both Dockerfiles and the README registry gateway install select
it. Default crate features provide a library, not a gateway executable. Rust
1.96.1 is the pinned release builder; the manifest's library MSRV is separate.

## Supported scope restrictions

- Responses built-in billing supports bounded, synchronous OpenAI file_search;
  unknown charges retain the reserve. Background tools and other hosted tools
  are not enabled.
- MCP supports stateless 2026-07-28 POST/response SSE, not older protocol sessions,
  stdio, OAuth acquisition or external tool billing.
- A2A 1.0 ownership is process-local with bounded lifetime; affinity is required
  across replicas. The gateway does not execute agents or support push tasks.
- Realtime supports OpenAI GA manual text/audio/function events. Automatic VAD,
  transcription, image input and hosted tools are excluded. Content guardrails
  must explicitly be disabled for this first scope; crash accounting is not a
  durable exactly-once ledger.
- Model retirements and non-chat capabilities follow the dated audits. Native
  Nebius image generation remains open in #1440 because its current price/usage
  contract has not been established. No paid provider calls are part of release
  verification.

## Required artifact verification

The existing Release workflow remains the publisher. Required archives target
Linux x86_64 GNU, Windows x86_64 MSVC, macOS x86_64 and macOS ARM64; Linux x86_64
musl is explicitly optional. Each archive has a SHA-256 sidecar. GHCR ships
linux/amd64 and linux/arm64 manifests; Docker Hub is optional. Actual crates.io
upload, GitHub public release and Homebrew formula must be checked separately
from job success or skip status.

Run these in the session's clean worktree at the accepted candidate commit:

```sh
cargo fmt --check
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
cargo test --locked --no-default-features --features postgres,sqlite,redis,s3,metrics,tracing,websockets,providers-extra,providers-extended,mcp-validation,a2a
cargo clippy --locked --no-default-features --all-targets --features postgres,sqlite,redis,s3,metrics,tracing,websockets,providers-extra,providers-extended,mcp-validation,a2a -- -D warnings
cargo package --locked --no-default-features --features postgres,sqlite,redis,s3,metrics,tracing,websockets,providers-extra,providers-extended,mcp-validation,a2a
```

Inspect the unpacked package's Cargo version/lockfile, embedded model and OpenAPI
files and .cargo_vcs_info.json. Install it into an empty task-owned root with the
same locked feature profile, then check version/config/startup and native
protocol requests against local mocks. Inspect container user, config, startup,
health and protocol behavior from actual image contents. Download published
archives/crate, compare checksums, confirm immutable source provenance, inspect
GHCR by digest and check the actual Homebrew URLs/checksums. Completed results
will replace this pending status; preparation alone does not close #1403.
