# Two-gateway convergence suite

This isolated suite starts two real gateway binaries against a disposable PostgreSQL database
and Redis service, with one local mock upstream. It tests authoritative configuration convergence,
node-specific decrypt failure, Redis subscription reconnection, revision-only notification payloads,
budget reserve/settle, shared deployment concurrency, cooldown recovery, and killing a lease owner.
The owner test waits for the production 600-second lease TTL; it does not shorten or edit Redis leases.

```bash
cargo build --locked --bin gateway --features postgres
python3 -m pip install PyYAML==6.0.2 redis==6.4.0
HA_DATABASE_URL=postgresql://postgres@127.0.0.1:5432/litellm_ha \
REDIS_URL=redis://127.0.0.1:6379 \
python3 scripts/ha/two_gateways.py --binary target/debug/gateway --output /tmp/ha-evidence
```

Use a **fresh disposable database** and isolated Redis service for each run. The suite kills
Pub/Sub connections to exercise reconnect and writes global runtime configuration. It disables
gateway auth only on loopback listeners for the test. It generates a dedicated configuration
key per run and removes generated gateway config files from evidence, leaving node logs,
active/observed revision diagnostics, relevant Redis circuit/admission/budget hashes, and assertions.
A failed assertion exits nonzero and still captures diagnostics. A completed run takes about
11 minutes after compilation, mostly waiting for the actual owner lease expiration.

The dedicated `Two-gateway convergence` workflow runs separately from the normal Rust suite.
It requires the runtime revision implementation from #1282. PostgreSQL/Redis containers and the
in-memory mock are disposable CI-only services; no external provider credentials are needed.
