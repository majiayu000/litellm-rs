## Configuration

```yaml
streaming:
  enabled: true

  buffer:
    initial_size: 8192      # Initial buffer size in bytes
    max_size: 1048576       # 1MB max buffer
    chunk_size: 4096        # Read chunk size

  timeouts:
    first_byte: 30000       # 30s timeout for first byte
    between_events: 60000   # 60s timeout between events
    total: 300000           # 5 minute total timeout

  retry:
    enabled: true
    max_attempts: 3
    backoff_ms: 1000
```
