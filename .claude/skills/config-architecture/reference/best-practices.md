## Best Practices

### 1. Use Defaults Appropriately

```rust
// Good - sensible defaults
#[serde(default = "default_timeout")]
pub timeout: u64,

fn default_timeout() -> u64 { 120 }

// Bad - no default, requires user to specify
pub timeout: u64,  // Will fail if not specified
```

### 2. Validate Early

```rust
// Good - validate at load time
let config = loader.load_from_file(path)?;
config.validate()?; // Fail fast

// Bad - validate at use time
let config = loader.load_from_file(path)?;
// Later in code...
if config.timeout == 0 { panic!("Invalid timeout"); }
```

### 3. Sensitive Data Handling

```rust
// Good - use environment variables
auth:
  jwt:
    secret: ${JWT_SECRET}

// Bad - hardcoded secrets
auth:
  jwt:
    secret: "my-super-secret-key"
```

### 4. Type Safety

```rust
// Good - typed configuration
#[derive(Deserialize)]
struct ServerConfig {
    port: u16,  // Port must be valid u16
    timeout: Duration,
}

// Bad - stringly typed
#[derive(Deserialize)]
struct ServerConfig {
    port: String,  // Could be anything
    timeout: String,
}
```

### 5. Document Configuration

```yaml
# Good - documented options
server:
  # Port to listen on (default: 8080)
  port: 8080

  # Number of worker threads (0 = auto-detect based on CPU cores)
  workers: 0

  # Request timeout in seconds (default: 300)
  request_timeout: 300
```
