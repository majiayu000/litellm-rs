## Permission System

### Permission Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionConfig {
    pub default_policy: Policy,
    pub rules: Vec<PermissionRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Policy {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    pub users: Vec<String>,      // User patterns (supports *)
    pub tools: Vec<String>,      // Tool patterns (supports *)
    pub servers: Vec<String>,    // Server patterns (supports *)
    pub policy: Policy,
}
```

### Permission Manager

```rust
pub struct PermissionManager {
    config: PermissionConfig,
}

impl PermissionManager {
    pub fn new(config: PermissionConfig) -> Self {
        Self { config }
    }

    pub fn can_use_tool(&self, user_id: &str, tool_name: &str) -> bool {
        // Check rules in order (first match wins)
        for rule in &self.config.rules {
            if self.matches_pattern(&rule.users, user_id)
                && self.matches_pattern(&rule.tools, tool_name)
            {
                return matches!(rule.policy, Policy::Allow);
            }
        }

        // Fall back to default policy
        matches!(self.config.default_policy, Policy::Allow)
    }

    pub fn can_use_server(&self, user_id: &str, server_name: &str) -> bool {
        for rule in &self.config.rules {
            if self.matches_pattern(&rule.users, user_id)
                && self.matches_pattern(&rule.servers, server_name)
            {
                return matches!(rule.policy, Policy::Allow);
            }
        }

        matches!(self.config.default_policy, Policy::Allow)
    }

    fn matches_pattern(&self, patterns: &[String], value: &str) -> bool {
        patterns.iter().any(|pattern| {
            if pattern == "*" {
                true
            } else if pattern.ends_with('*') {
                value.starts_with(&pattern[..pattern.len() - 1])
            } else if pattern.starts_with('*') {
                value.ends_with(&pattern[1..])
            } else {
                pattern == value
            }
        })
    }
}
```

---
