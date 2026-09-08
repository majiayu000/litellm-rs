# Runtime configuration revisions

Opt in to shared runtime configuration using the same PostgreSQL database, Redis service,
and dedicated encryption key on every gateway replica:

```yaml
storage:
  config_sync_key_env: LITELLM_CONFIG_SYNC_KEY
  database:
    enabled: true
    url: "${DATABASE_URL}"
    fallback_to_sqlite: false
  redis:
    enabled: true
    allow_degraded: false
    url: "${REDIS_URL}"
```

Set `LITELLM_CONFIG_SYNC_KEY` through the deployment's Secret/environment. Generate at least
32 random bytes (for example, `openssl rand -hex 32`), retain the key with database backups,
and use a separate key from JWT signing or API-key hashing. Only the **environment variable
name** belongs in the YAML. Run `gateway --config gateway.yaml database migrate` before
starting with a database role that cannot run migrations. Missing/short keys, missing schema,
or failure to decrypt the authoritative snapshot fail startup explicitly.

The first successful runtime mutation commits revision 1 from the bootstrap configuration.
Subsequent admin provider/routing mutations build a complete candidate before a database
compare-and-swap commits the encrypted snapshot. Stale concurrent writers receive HTTP 409
and must reload before retrying. Rejected candidates leave the authoritative revision unchanged.
Each process applies its own runtime bundle atomically; a failing replica retains its previous
bundle without rolling back replicas that applied successfully.

The snapshot contains providers (including resolved credentials), router policy, aliases,
guardrails, and response-cache settings. Listener ports, auth settings, database/Redis URLs,
and telemetry remain node-local. Environment-backed provider credentials are captured at
mutation time; updating the environment alone does not change a stored snapshot. To rotate
credentials, update them through the provider API using the new environment reference.

Redis channel `litellm-rs:config:revisions` carries only decimal revision IDs. It is a wake-up
hint; the PostgreSQL row is the authority. Subscribers connect before fetching the latest
revision, ignore old/duplicate notifications, and reconcile every second while connected.
After a disconnect they reconnect and fetch the latest database snapshot. Global Redis
Pub/Sub works through any reachable configured Cluster seed. A committed update whose
notification fails still records the sanitized revision/audit event, then returns an explicit error identifying the committed revision; do not assume
that an error means the mutation was rolled back. Missed notifications are repaired by the
periodic database read/reconnect. Database outages preserve the last active runtime and are
visible in diagnostics; they never write to a local fallback store.

`GET /admin/routing/revision` requires the same Admin authorization as the existing routing
API. It returns `active_revision`, `observed_revision`, `last_apply_error`, and
`last_sync_error`, without keys, ciphertext, provider data, or raw backend error messages.
An observed revision ahead of the active revision with an apply error identifies a node that
failed to apply. Do not serve that node as current until the error is repaired.

The encryption key must remain stable for the lifetime of the stored snapshot; automatic key
rotation/backfill is not implemented. Databases without an authoritative snapshot continue to
bootstrap from their normal gateway configuration until the first mutation. Existing sanitized
provider/routing revision records remain audit records, not a configuration restore format.

## Design decision

Adopt the existing AES-256-GCM helper, SeaORM, and atomic runtime builder. Use one PostgreSQL
row with a monotonic CAS revision, not Redis as a second configuration store. Redis Pub/Sub
is [at-most-once](https://redis.io/docs/latest/develop/pubsub/#delivery-semantics), so subscribe-
then-read plus periodic database reconciliation is required for lost notifications. No gossip,
new event bus, database polling options, or second gateway configuration schema is added.
