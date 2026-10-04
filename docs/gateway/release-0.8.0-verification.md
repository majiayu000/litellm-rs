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

## Completed preflight (2026-10-04)

These are preparation artifacts, not the final tag or published artifacts.

| Check | Actual result |
| --- | --- |
| Default checks | fmt/check/full test/all-target clippy passed; library tests: 7,125 passed, 1 ignored, plus integration/doc tests |
| Exact shipped profile | Full test and all-target clippy passed; library tests: 11,010 passed, 1 ignored, plus integration/doc tests |
| Clean package | `cargo package --locked` verification passed at `78746e1670ef0603821a8098085c13188cbf61d7`; package VCS metadata matches that commit |
| Package SHA-256 | `4dfc995e58a19b2efc0fca5bda5e743ab49496e244420c69b21c2aaebd042a5e` |
| Empty-root installation | Installed the unpacked package with the exact profile and `--debug --locked`; version/config checks passed. This checks a clean package installation, not an optimized published binary |
| Installed executable | Chat, native Responses JSON/SSE, bounded file_search, Responses 429/Retry-After, Messages, Gemini and Realtime passed (8 checks) |
| ARM64 release container | Built the real Dockerfile at source `da3d025547379d6e8296d6878ef7551e999634b1`; image `sha256:fd033bf8c7c4a4314493e48120f7b91ebf1bed76e8204fb380037c64cbc3629f` runs as appuser; version/config/health passed |
| Container protocols | The preceding 8 checks plus MCP tools/list, A2A SendMessage and owned GetTask passed (11 checks) |

Both executable and container tests use a local TLS mock with a separate CA and
server leaf certificate; TLS verification remains enabled. A fresh test user is
registered through the HTTP API, then activated only in the fixture SQLite DB.
The container network is internal: its simulated public IP range lets MCP/A2A
exercise the existing public-address policy without permitting vendor calls.
Realtime completed with modality usage totaling 60 tokens. Native markers and
429 Retry-After are asserted, not inferred from HTTP status alone.

The earlier release-CI timeout and an earlier local integration hang are not
declared fixed. The complete local exact-profile run above passed; the final
accepted commit still needs its own green CI and published-artifact identifiers.

## Reproduce the runtime smoke

Use Python 3.10+ with requests/PyYAML and OpenSSL. Run without Python's `-O`
option because this test harness uses assertions. Keep all fixtures in a new
task directory; never point the harness at a production gateway or database.

```sh
release_smoke_dir=$(mktemp -d)
mkdir "$release_smoke_dir/certs"
python3 -m venv "$release_smoke_dir/venv"
"$release_smoke_dir/venv/bin/pip" install requests==2.32.5 PyYAML==6.0.3
openssl req -x509 -newkey rsa:2048 -nodes -days 2 \
  -subj /CN=ReleaseSmokeCA -addext basicConstraints=critical,CA:true \
  -keyout "$release_smoke_dir/certs/mock-ca-key.pem" \
  -out "$release_smoke_dir/certs/mock-ca.pem"
openssl req -new -newkey rsa:2048 -nodes -subj /CN=release-mock \
  -keyout "$release_smoke_dir/certs/mock-key.pem" \
  -out "$release_smoke_dir/certs/mock.csr"
cat > "$release_smoke_dir/certs/extensions" <<'EOF'
basicConstraints=critical,CA:false
subjectAltName=DNS:release-mock,IP:127.0.0.1
extendedKeyUsage=serverAuth
keyUsage=digitalSignature,keyEncipherment
EOF
openssl x509 -req -days 2 -in "$release_smoke_dir/certs/mock.csr" \
  -CA "$release_smoke_dir/certs/mock-ca.pem" \
  -CAkey "$release_smoke_dir/certs/mock-ca-key.pem" -CAcreateserial \
  -extfile "$release_smoke_dir/certs/extensions" \
  -out "$release_smoke_dir/certs/mock-cert.pem"
"$release_smoke_dir/venv/bin/python" scripts/test/release_smoke.py local \
  --binary /absolute/path/to/clean-install/bin/gateway \
  --packaged config/gateway.dev.yaml.example \
  --directory "$release_smoke_dir/local" \
  --certificates "$release_smoke_dir/certs"
```

This writes config/logs/results under the fixture directory and stops its own
gateway/mock processes. It refuses to overwrite an existing local smoke DB.
The chosen local ports are 18443 (TLS mock) and 18808 (gateway).

For a container, the same script supplies three modes: `mock`, `remote-config`
and `remote-smoke`. Use an internal Docker network with the mock at 11.73.4.2
(alias release-mock) and the gateway at 11.73.4.3. Build a temporary Python image
with the two dependencies above. Mount the script and certificates read-only;
mount a fresh fixture directory read/write at `/app/smoke` in both config/client
containers and the gateway. Generate the config **inside the container** so its
SQLite path matches the gateway's mount:

```sh
python /release_smoke.py remote-config \
  --packaged /repo/config/gateway.dev.yaml.example --directory /app/smoke \
  --base https://release-mock:18443 --gateway http://11.73.4.3:8000
# Start the mock in its own container:
python /release_smoke.py mock --directory /app/smoke \
  --certificates /certificates
# Start a fresh gateway container from the actual candidate image with:
# SSL_CERT_FILE=/certificates/mock-ca.pem, LITELLM_DATA_DIR=/app/smoke/data
gateway --config /app/smoke/config.json --host 0.0.0.0 --port 8000
# Run the client inside the same internal network:
python /release_smoke.py remote-smoke --directory /app/smoke \
  --gateway http://11.73.4.3:8000
```

Keep the gateway fixture writable by its appuser; the mock/server needs no
database access. Run one smoke per fresh gateway process: the intentional 429
can put a deployment into its normal cooldown. Inspect Docker health and image
revision/version/user separately, and remove only the task's containers/network
when finished. No test certificate or private key is committed.
