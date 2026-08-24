## Best Practices

### 1. Use Defaults Appropriately

Follow the existing pattern in `src/config/models/`: shared default fns in `src/config/models/mod.rs` / `defaults.rs`, referenced via `#[serde(default = "...")]`.

```rust
// Good - named default fn, shared across models
#[serde(default = "default_timeout")]
pub timeout: u64,          // default_timeout() -> 30

// Bad - magic value inline, or no default forcing every YAML to spell it out
pub timeout: u64,
```

Know the real defaults before writing docs or examples: port `8000`, timeout `30`s, `max_body_size` 10 MiB, provider `weight` 1.0, `rpm` 1000, `tpm` 100_000, `max_retries` 3.

### 2. Validate Early

`Config::from_file` and `Config::from_env` already run the full `Validate` tree at the end of loading — callers never validate manually.

```rust
// Good - from_file validates; a bad config aborts startup here
let config = Config::from_file(path).await?;

// Bad - scattering ad-hoc checks through handler code
if config.server.timeout == 0 { panic!("Invalid timeout"); }
```

New rules belong in `src/config/validation/*` as `Validate` implementations, so `gateway validate-config` and both load paths enforce them identically.

### 3. Sensitive Data Handling

Reference secrets through environment substitution; unresolved `${VAR}` is a hard error, so a missing secret fails startup instead of shipping a literal placeholder.

```yaml
# Good - substituted at load; startup fails if unset
auth:
  jwt_secret: "${LITELLM_JWT_SECRET}"

# Bad - committed literal
auth:
  jwt_secret: "my-super-secret-key"
```

Exports are redacted: `Config::to_json` / `to_yaml` replace provider API keys, `jwt_secret`, the API-key HMAC secret, S3/vector-DB credentials, and SSO client secrets with `[REDACTED]` before serializing. Keep new secrets in that redaction list.

### 4. Type Safety

Use typed structs and add `deny_unknown_fields` when the configuration surface
is intended to be strict. Most gateway-facing models do this, but not every
deserialized payload does: `providers[].settings` is deliberately open-ended,
and several provider/callback backend config structs currently accept unknown
fields. Do not promise strict rejection without checking the concrete struct.

```rust
// Good - typed, unknown YAML keys rejected at parse time
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ServerConfig {
    port: u16,
    timeout: u64,
}

// Bad - stringly typed, typos silently accepted
struct ServerConfig {
    port: String,
    timeout: String,
}
```

### 5. Document Configuration

Update `config/gateway.yaml.example` whenever the schema changes — tests (`test_gateway_yaml_example_matches_config_schema` in `src/config/mod.rs`) assert the example parses and validates against the current models, so a stale example fails CI.

```yaml
# Good - documented options matching real fields
server:
  port: 8000            # default 8000
  workers: 4            # omit to auto-detect CPU count
  timeout: 30           # request timeout in seconds, default 30
```
