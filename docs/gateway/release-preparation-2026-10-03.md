# Parity release preparation (F18)

Issue #1403. This is preparation, not a published release or a final release candidate.
The tested source is main `ceff7f39bd2902ce7a1d1ff816d84c7fadabfb14` plus this
preparation patch. Other parity PRs still require acceptance and integration.
Package version remains 0.7.0; no tag or registry upload was made.

## Changes

- Release archives and both Dockerfiles use Rust 1.96.1 and the existing shipped
  feature profile, with A2A included and the expired analytics feature removed.
  The feature is not evidence that the pending A2A route is in this baseline.
- The Docker dependency-dummy layer failed Cargo manifest validation because
  declared examples were absent. Both files now copy the actual declared targets
  and required embedded config/OpenAPI files, then build the gateway directly.
- GHCR publication uses the existing GITHUB_TOKEN independently of optional
  Docker Hub credentials. Published manifests are inspected by immutable digest.
- Binary archives include SHA-256 sidecars. Empty staging/production echo jobs
  were removed; this workflow distributes artifacts and does not deploy servers.
- Registry installation documentation pins binary and example config to the same
  actually published version and explicitly enables SQLite.

## Finished local checks

`cargo fmt --check`, `cargo check`, `cargo test` (7,267 library tests passed,
1 ignored, plus integration/doc tests), and all-target clippy passed.
Both changed workflow files parse as YAML. The shipped-profile `cargo package`
verification compiled the unpacked crate successfully, with embedded prices,
catalog decisions/authority and both OpenAPI documents included.

Commands run in the independent release-preparation worktree:

```sh
cargo package --allow-dirty --locked --no-default-features --features postgres,sqlite,redis,s3,metrics,tracing,websockets,providers-extra,providers-extended,mcp-validation,a2a
cargo install --path target/package/litellm-rs-0.7.0 --debug --locked --root /tmp/litellm-preflight-install-20261003 --features sqlite --bin gateway
/tmp/litellm-preflight-install-20261003/bin/gateway --version
/tmp/litellm-preflight-install-20261003/bin/gateway --config target/package/litellm-rs-0.7.0/config/gateway.dev.yaml.example validate-config
docker build --progress=plain -f deployment/docker/Dockerfile -t litellm-parity-preflight:20261003 .
docker run --rm litellm-parity-preflight:20261003 gateway --version
docker run -d --name litellm-parity-preflight-20261003 -p 127.0.0.1:18089:8000 litellm-parity-preflight:20261003 gateway --config config/gateway.dev.yaml.example --host 0.0.0.0 --port 8000
curl --max-time 10 -i http://127.0.0.1:18089/health
docker stop litellm-parity-preflight-20261003
docker rm litellm-parity-preflight-20261003
```

The clean local install reported `gateway 0.7.0` and accepted its packaged config.
The Linux ARM64 image ran as `appuser` and returned HTTP 200 from `/health`.
Local image ID: `sha256:511857cec29c6de0ccf5996ff8dea1b5f0a1a438cf8e53013c3706ec177363c6`.
Preverification crate SHA-256:
`b214178427ed9b5fe1dbd2b4ca6d1f9a130e90d6ed5225bec7e7cab068b11af2`.
These identify local dirty-tree preflight artifacts, not the existing published
0.7.0 artifact or a future candidate. Repackaging the committed candidate will
produce its own evidence. The ARM-specific alternative Dockerfile has not been
built separately. No paid provider calls were run.

## Final acceptance still required

- Integrate accepted protocol/catalog/removal PRs and choose an immutable version
  commit; the repository's pre-1.0 breaking-change release policy applies.
- Re-run shipped-profile CI and platform builds on that candidate, and verify the
  packaged crate and fresh install against its matching examples.
- Smoke-test the accepted native Responses/Messages/Gemini, MCP, A2A and Realtime
  routes from actual container/archive contents with local mock upstreams.
- Record GitHub archive checksums, crates.io version/checksum and GHCR manifest
  digest/platforms after actual publication. Docker Hub remains optional and must
  be reported skipped when credentials are absent, not reported published.
- Verify Homebrew's actual formula/binary update separately from a successful job
  that may skip. Only then record F18 completion in the parity tracker.
