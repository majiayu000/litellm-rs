# 0.8 release verification (F18)

Issue #1403. Status: 0.8.2 is publicly released and its actual distributions
are verified. [GitHub Release](https://github.com/majiayu000/litellm-rs/releases/tag/v0.8.2),
[crates.io](https://crates.io/crates/litellm-rs/0.8.2), GHCR linux/amd64 and
linux/arm64, and the Homebrew formula all identify the accepted source below.
The earlier v0.8.0/v0.8.1 tags and drafts retain their macOS fixture failure
history; those versions were not published to crates.io or as public GitHub
releases. The 0.8 line is a pre-1.0 minor
release because accepted changes remove public APIs and change default features.

## Candidate scope

The candidate includes the accepted parity work through #1441, the remaining
model audit #1442 and unused-interface reconciliation #1443. The complete release
notes are in CHANGELOG.md. The first accepted immutable commit is `1ab0b47cc5e6f78d6ab9638921f3f739a56749b0`,
tagged as v0.8.0 after #1444 final-head 15 checks passed and all reviews were
resolved. The published version is 0.8.2: it adds only version/install metadata
and explicit fixture CA configuration to accepted 0.8.1, retaining its mock
DNS/logging correction. Production Rust behavior and the shipped profile are
unchanged. The accepted 0.8.2 commit is
`3dcc0aa9d7f0c2a585fe26fdf1ea6cc9bfd99b43`; verified publication identifiers
are recorded in the 0.8.2 acceptance section below.
PR branch heads are preparation evidence and do not substitute for that commit.

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
musl is explicitly optional. Intel macOS uses the native macos-15-intel runner;
ARM64 macOS uses macos-latest, matching the [official runner architectures](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
The existing publisher executes version/config and the local protocol harness
against each built executable before uploading its archive; all five 0.8.2
native jobs passed those checks. Each archive has a SHA-256 sidecar. GHCR ships
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
cat > "$release_smoke_dir/certs/ca.cnf" <<'EOF'
[req]
distinguished_name=req_dn
x509_extensions=v3_ca
[req_dn]
[v3_ca]
basicConstraints=critical,CA:true
EOF
openssl req -x509 -newkey rsa:2048 -nodes -days 2 \
  -config "$release_smoke_dir/certs/ca.cnf" -subj /CN=ReleaseSmokeCA \
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
The default local ports are 18443 (TLS mock) and 18808 (gateway); `--base`
and `--gateway` select matching alternate ports. A fresh virtualenv/custom-port
run passed all 8 checks. A missing-binary run failed as expected and released
the mock port; the child uses the parent interpreter and startup failures clean
up both test processes.

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

## 0.8.0 accepted commit and package verification

- Accepted source/tag: [1ab0b47cc5e6f78d6ab9638921f3f739a56749b0](https://github.com/majiayu000/litellm-rs/commit/1ab0b47cc5e6f78d6ab9638921f3f739a56749b0), v0.8.0.
- [Preparation PR #1444](https://github.com/majiayu000/litellm-rs/pull/1444) final head 7fc30ff31e4a00365641fe165d2bfd2d95c20573: 15 successful checks, all review threads resolved. Source acceptance is separate from publishing.
- Clean exact-profile `cargo package --locked` verification passed on the accepted commit. Cargo.toml/Cargo.lock contain 0.8.0; .cargo_vcs_info.json identifies that same clean commit.
- Accepted local crate SHA-256: `fa5518a655f753ee8841c61d9ff4956f7866aa65bc184d9656f004cda9d6d0ff`. Package inspection confirmed native Responses/Messages/Realtime, MCP/A2A routes, embedded prices/catalog and both OpenAPI files.
- Installed the accepted unpacked crate into a new empty root with `--debug --locked --no-default-features` and the exact shipped profile. Actual installed binary version and all 8 local TLS protocol checks passed. Registry/release-mode installation remains a separate check.
- Accepted ARM64 local release image: `sha256:f0ecb5675257f6a6a607e4e5249e4db5f0716a720361c7f702451f4e6ef16cab`. Full OCI revision equals the accepted commit; appuser, 0.8.0 version and packaged config validation checked. This local image ID is not a published GHCR manifest digest.
- Tag-triggered existing [Release run 37198981030](https://github.com/majiayu000/litellm-rs/actions/runs/37198981030) targets the accepted commit. Windows, Linux GNU and optional musl artifact smoke passed. Both macOS artifact
  smoke jobs failed; GitHub release remains draft and crates.io publication was
  skipped. The superseded Docker build was canceled before completion. The tag and
  draft/failure history are retained; this is not a completed release.

The exact shipped profile unifies reqwest's native roots through locked
object_store 0.13.2. rustls-native-certs reads the test-only SSL_CERT_FILE before
platform roots; this explains the real binary/container TLS results. No system
trust store, production TLS settings or certificate verification was modified.

## 0.8.1 artifact smoke correction

The macOS jobs built and validated the executable but failed in the local
protocol fixture. GitHub runner maintainers [reproduced a roughly 35-second
Python HTTPServer reverse-DNS stall before listening](https://github.com/actions/runner-images/issues/14409#issuecomment-5034633535).
Our mock used that same constructor path. This is a supported explanation for
the fixture delay; the corrected macOS run must establish the actual result.

The mock now binds through TCPServer, sets its local server name and activates
its TLS listener without the unused hostname lookup. A regression run with
`socket.getfqdn` forced to fail still established a listening socket. The actual
accepted 0.8.0 installed gateway passed all 8 native protocol checks with the
corrected mock in a virtualenv and alternate ports. A missing-binary run
preserved FileNotFoundError, printed both child logs and released the mock port.
Logs are read after both children stop and the parent's write handles close;
an early-exiting child also preserved the startup exception and its log marker.
No timeout increase or runner/system security change was made. Cross-platform
0.8.1 artifact and registry results remain required.

The 0.8.1 preparation worktree passed fmt/check, default full tests (7,125
library tests, 1 ignored, plus integration/doc tests) and all-target clippy.
The exact shipped profile passed full tests (11,010 library tests, 1 ignored,
plus integration/doc tests) and all-target clippy. These local source checks
do not substitute for the corrected macOS artifact smoke or publication.

## 0.8.1 accepted source and publication acceptance

- Accepted source/tag: [28362c09613d075a67f9a6dae18fb06e44b19c8c](https://github.com/majiayu000/litellm-rs/commit/28362c09613d075a67f9a6dae18fb06e44b19c8c), v0.8.1.
- [Correction PR #1445](https://github.com/majiayu000/litellm-rs/pull/1445) final head `2d1e4324dd1f5d7da6e878bad430c832513d4976`: all 15 checks succeeded, all review threads resolved, then merged.
- Clean accepted-source exact-profile `cargo package --locked` verification passed. Local package SHA-256: `7171498fba27747de9b3d794fac2d4555cba8f7dcf59dbaedf8ef256ada65f7e`. Embedded VCS metadata equals the accepted clean commit; native protocol routes and both OpenAPI files are present.
- Installed the unpacked accepted crate into a new empty root using `--debug --locked --bin gateway --no-default-features` and the exact profile. Actual binary reports `gateway 0.8.1`; packaged config validation and all 8 native TLS protocol checks passed. This precedes optimized registry installation.
- [Tag-triggered Release run 37206902557](https://github.com/majiayu000/litellm-rs/actions/runs/37206902557) targets the accepted source. Exact shipped-profile full tests/clippy and all-features compile checks passed. Windows, ARM64 macOS, Linux GNU and optional musl artifact smoke passed. Intel macOS built successfully but rejected the generated fixture CA at startup; its failure log exposed the native-root-store panic. crates.io/public GitHub release were not published; the superseded Docker build was canceled.

Downloaded 0.8.1 ARM64 macOS, Windows, Linux GNU and musl archives matched their
SHA-256 sidecars and GitHub asset digests; each contains its single executable
with the expected architecture. The actual ARM64 macOS download passed version,
config and all 8 native TLS checks. The actual GNU download passed 11 checks,
including MCP tools/list and A2A caller-owned tasks, on an isolated Docker network.
These partial artifacts do not constitute a completed release.

## 0.8.2 CA fixture correction

Intel macOS rejected the 0.8.1 fixture root certificate with `zero valid
certificates found in native root store`. Locked reqwest returns this error when
native certificates load but every DER certificate is rejected by rustls.
Reproducing the generation command with LibreSSL 3.3.6 and a default config
already containing v3_ca emitted two Basic Constraints extensions and caused
the downloaded accepted gateway to panic with that exact error. A plain local
config without that default did not fail. The runner's original certificate is
unavailable; the reproduction explains the platform-config sensitivity. The
corrected 0.8.2 Intel macOS job passed its actual executable smoke.

The CA now uses an explicit fixture config with its extension declared once,
without appending -addext to the platform defaults. Production TLS and the
trusted boundary are unchanged. Candidate 0.8.2 includes the earlier 0.8.0
changes and 0.8.1 DNS/logging correction; it does not move either existing tag.

The exact corrected workflow certificate block was executed with LibreSSL 3.3.6
and OpenSSL 3.6.3, forcing a default OPENSSL_CONF already containing v3_ca in
both cases. Each emitted one Basic Constraints extension, and the actual
downloaded gateway passed all 8 native TLS protocol checks with each certificate
set. All five native 0.8.2 jobs subsequently passed version/config and 8 checks.

The 0.8.2 preparation worktree passed fmt/check, default full tests (7,125
library tests, 1 ignored, plus integration/docs) and all-target clippy, plus
the exact shipped-profile full tests (11,010 library tests, 1 ignored, plus
integration/docs) and all-target clippy.

## 0.8.2 accepted source and artifact acceptance

- Accepted source/tag: [3dcc0aa9d7f0c2a585fe26fdf1ea6cc9bfd99b43](https://github.com/majiayu000/litellm-rs/commit/3dcc0aa9d7f0c2a585fe26fdf1ea6cc9bfd99b43), v0.8.2.
- [CA correction PR #1446](https://github.com/majiayu000/litellm-rs/pull/1446) final head `d83bd30ce624c10552b4751ba62d60fb7361dbc6`: all 15 checks succeeded, both review threads resolved, then merged. An earlier superseded CI run stalled in audio_routes; the final full run passed, and the hang root cause is not declared fixed.
- Clean accepted-source exact-profile `cargo package --locked` verification passed. Local package SHA-256: `6abcdb2695d898145d8bb673775a5472e910db49657c312b9e5e7f69ce90fd05`. Embedded VCS metadata identifies the same clean accepted commit; package contents include native routes, embedded model/pricing JSON and both OpenAPI files.
- Installed the unpacked accepted package in an empty root with `--debug --locked --bin gateway --no-default-features` and the exact shipped profile. Version reports `gateway 0.8.2`; packaged config validation and all 8 native protocol checks passed using the corrected LibreSSL-generated TLS certificates.
- The [published registry crate](https://crates.io/crates/litellm-rs/0.8.2) became available at `2026-10-04T18:40:23.971761Z`. The actual downloaded crate matches the local package SHA-256 above and its clean VCS commit. The exact README registry command completed an optimized installation into a new empty root with `--locked --bin gateway --no-default-features` and the shipped profile; installed version/config and all 8 native TLS checks passed.
- [Existing Release publisher run 37218359967](https://github.com/majiayu000/litellm-rs/actions/runs/37218359967), attempt 2, targets the accepted tag. Exact-profile full tests (11,010 library tests passed, 1 ignored, plus integration/doc tests), all-target clippy and all-features compile checks passed. All five native artifact jobs passed, including Intel macOS. The first attempt hit its declared 45-minute limit in an audio-route test after library tests passed; the API reports cancellation and the annotation identifies the time limit. Retrying that job at the same source passed. The intermittent hang root cause remains unconfirmed.
- [Public GitHub Release](https://github.com/majiayu000/litellm-rs/releases/tag/v0.8.2) was published at `2026-10-04T20:22:19Z`; it is neither draft nor prerelease and records the accepted full commit. All 12 publisher jobs in attempt 2 completed successfully, including actual crates.io upload, GHCR build/push, GitHub publication and Homebrew update. Docker Hub was skipped because its optional credentials are absent.
- Downloaded all five actual archives; SHA-256 sidecars and GitHub asset digests match, with one executable of the expected architecture per archive. The ARM64 macOS download passed version/config and all 8 native TLS checks; the GNU download passed all 11 protocols, including MCP/A2A, in an isolated container network. GNU and musl downloaded binaries both report `gateway 0.8.2`. After publication, all five archives and their sidecars were downloaded again through the public release URLs; every checksum still matches the table below.

| Archive | SHA-256 |
| --- | --- |
| Linux x86_64 GNU | `7c6d95b194c8a52f74f087688c342e2d01d3958af9029575eea0ea018808ace6` |
| Linux x86_64 musl | `94bee56b5b3555ece046ec392311776643cb3fa5e0aab2d51001fb57a710da12` |
| macOS ARM64 | `22bcdfe1c8ffa207cacfba256994a029c181c9072233bc2d69d6371d8083190b` |
| macOS x86_64 | `e19fd47ec83694306a147223950d73e2b49c22b91b855ab05eb1ce2463026cf2` |
| Windows x86_64 MSVC | `1263151a49ea04302787863491ce46fa73cee040cab028a598dedc09c3cdd463` |

Published image `ghcr.io/majiayu000/litellm-rs:0.8.2` has these immutable digests:

| Manifest | Digest |
| --- | --- |
| Multi-platform index | `sha256:551f138599554d00b9959f54d07cb0c00ec797cb59e6d114f3669ec36b8c09b3` |
| linux/amd64 | `sha256:7727bde482796a63d80c890de84a78cb9f6bc7e477d85647591dafd456a68b1c` |
| linux/arm64 | `sha256:042c2f2bf89194ed26a4e241803ace818f6baad7e75af56de5801828eb463008` |

The actual images were pulled by digest and inspected independently: each has
the expected architecture, full accepted OCI revision, version 0.8.2, appuser
and gateway command. Both passed executable version, packaged config validation,
startup, the declared health probe and all 11 TLS mock protocol checks on
separate internal networks. ARM64 ran natively; amd64 ran under local Docker
emulation. Image config IDs are distinct from the manifest digests above.

The actual [Homebrew formula at ad285f87487f12f984cff02ab223d49d8a6e8afd](https://github.com/majiayu000/homebrew-tap/blob/ad285f87487f12f984cff02ab223d49d8a6e8afd/Formula/rust-litellm-gateway.rb)
selects version 0.8.2 and the public ARM64/Intel macOS archive URLs with exactly
the two SHA-256 values above. Its install method copies the verified gateway;
Ruby syntax was checked by the publisher. Formula content and both public
downloads were verified separately from job status. A host-wide Homebrew install
and `brew test` were not performed; the optimized clean registry install and
actual archive execution are the installation/runtime evidence.
