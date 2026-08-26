## Configuration

The single streaming-related config knob is
`gateway.server.stream_idle_timeout`
(`src/config/models/server.rs`). In YAML it sits under the top-level
`server:` section of the gateway config file:

```yaml
server:
  stream_idle_timeout: 300   # seconds between allowed idle gaps; 0 disables
```

- Type `u64`, unit seconds, default `300` (5 minutes).
- Routes read it as
  `state.config.load().gateway.server.stream_idle_timeout` and enforce it by
  wrapping each provider-stream poll in `tokio::time::timeout`
  (`src/server/routes/ai/chat_streaming.rs`; same knob in
  `completions_streaming.rs` and `responses_stream.rs`).
- On expiry the route emits an SSE error frame (`"server_error"` /
  `"timeout"`), records `ProviderError::timeout`, and closes the connection.
- One timeout covers all gaps between chunks; there is no separate first-byte
  or total-duration limit.

Things that do not exist (do not invent config for them):

- No `streaming:` YAML section — no buffer sizes, timeouts, or retry knobs.
- Buffer limits are compile-time constants:
  `MAX_CHUNK_BUFFER_SIZE = 10_000` for the chunk queue; the parser line buffer
  is uncapped.
- No streaming retry configuration.
