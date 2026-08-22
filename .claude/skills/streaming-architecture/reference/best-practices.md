## Best Practices

### 1. Handle Incomplete Events

```rust
// Good - buffer incomplete data
pub fn feed(&mut self, bytes: &[u8]) -> Vec<SSEEvent> {
    self.buffer.extend(bytes);
    self.extract_complete_events() // Only return complete events
}

// Bad - assume complete data
pub fn feed(&mut self, bytes: &[u8]) -> Vec<SSEEvent> {
    let text = String::from_utf8_lossy(bytes);
    self.parse_all(&text) // May split events incorrectly
}
```

### 2. Preserve Stream Order

```rust
// Good - maintain ordering
let stream = input.map(|chunk| {
    self.process_chunk(chunk) // Process in order
});

// Bad - parallel processing breaks order
let stream = input
    .map(|chunk| async move { self.process_chunk(chunk).await })
    .buffer_unordered(10); // Order not guaranteed!
```

### 3. Clean Resource Handling

```rust
// Good - cleanup on drop
impl Drop for StreamProcessor {
    fn drop(&mut self) {
        self.parser.reset();
        // Signal stream end if needed
    }
}
```

### 4. Backpressure Handling

```rust
// Good - respect backpressure
let stream = input
    .map(|chunk| self.process(chunk))
    .buffer_unordered(1); // Limit concurrent processing

// Bad - unbounded buffering
let stream = input
    .map(|chunk| self.process(chunk))
    .buffer_unordered(1000); // May exhaust memory
```
