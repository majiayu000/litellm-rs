## Best Practices

### 1. Agent Registration

```rust
// Good - validate capabilities and endpoint
fn register_agent(&self, agent: AgentCard) -> Result<(), A2AError> {
    // Validate agent has capabilities
    if agent.capabilities.is_empty() {
        return Err(A2AError::invalid_params("Agent must have capabilities"));
    }

    // Validate endpoint is reachable
    self.health_check(&agent).await?;

    self.registry.insert(agent);
    Ok(())
}

// Bad - no validation
fn register_agent(&self, agent: AgentCard) {
    self.registry.insert(agent);
}
```

### 2. Task State Management

```rust
// Good - enforce state transitions
impl Task {
    pub fn transition(&mut self, new_state: TaskState) -> Result<(), A2AError> {
        if !self.state.can_transition_to(new_state) {
            return Err(A2AError::invalid_transition(self.state, new_state));
        }
        self.state = new_state;
        self.updated_at = chrono::Utc::now().timestamp();
        Ok(())
    }
}

// Bad - allow arbitrary state changes
impl Task {
    pub fn set_state(&mut self, state: TaskState) {
        self.state = state;
    }
}
```

### 3. Error Handling

```rust
// Good - provide context and retryability
fn handle_provider_error(err: reqwest::Error, provider: AgentProvider) -> A2AError {
    if err.is_timeout() {
        return A2AError {
            code: A2AError::INTERNAL_ERROR,
            message: format!("Provider {} timed out", provider),
            data: Some(serde_json::json!({ "retryable": true })),
        };
    }

    if err.is_connect() {
        return A2AError {
            code: A2AError::INTERNAL_ERROR,
            message: format!("Cannot connect to provider {}", provider),
            data: Some(serde_json::json!({ "retryable": true })),
        };
    }

    A2AError {
        code: A2AError::INTERNAL_ERROR,
        message: err.to_string(),
        data: None,
    }
}
```

### 4. Capability Matching

```rust
// Good - match by capability semantics
pub fn find_agent_for_task(&self, instruction: &str) -> Option<Arc<AgentCard>> {
    // Extract required capabilities from instruction
    let required_caps = self.extract_capabilities(instruction);

    // Find agent with best match
    self.registry
        .list_online()
        .into_iter()
        .filter(|agent| {
            required_caps.iter().all(|cap| {
                agent.capabilities.iter().any(|c| c.name == *cap)
            })
        })
        .min_by_key(|agent| self.get_agent_load(agent))
}
```
