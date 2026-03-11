//! MCP Permission Control
//!
//! Access control for MCP servers and tools based on API keys, teams, and organizations.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::error::{McpError, McpResult};

/// Permission level for MCP access
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PermissionLevel {
    /// No access
    Deny,
    /// Read-only access (list tools, view info)
    Read,
    /// Execute access (can call tools)
    #[default]
    Execute,
    /// Full access (including admin operations)
    Admin,
}

impl PermissionLevel {
    /// Check if this level allows reading
    pub fn can_read(&self) -> bool {
        !matches!(self, PermissionLevel::Deny)
    }

    /// Check if this level allows execution
    pub fn can_execute(&self) -> bool {
        matches!(self, PermissionLevel::Execute | PermissionLevel::Admin)
    }

    /// Check if this level allows admin operations
    pub fn is_admin(&self) -> bool {
        matches!(self, PermissionLevel::Admin)
    }
}

/// Permission rule for a specific server or tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    /// Server name pattern (supports wildcards: * for any)
    pub server_pattern: String,

    /// Tool name pattern (supports wildcards, None means all tools)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_pattern: Option<String>,

    /// Permission level
    pub level: PermissionLevel,
}

impl PermissionRule {
    /// Create a new permission rule
    pub fn new(server: impl Into<String>, level: PermissionLevel) -> Self {
        Self {
            server_pattern: server.into(),
            tool_pattern: None,
            level,
        }
    }

    /// Add tool pattern
    pub fn for_tool(mut self, tool: impl Into<String>) -> Self {
        self.tool_pattern = Some(tool.into());
        self
    }

    /// Check if this rule matches a server/tool combination
    pub fn matches(&self, server: &str, tool: Option<&str>) -> bool {
        // Check server pattern
        if !pattern_matches(&self.server_pattern, server) {
            return false;
        }

        // Check tool pattern if specified
        match (&self.tool_pattern, tool) {
            (Some(pattern), Some(tool_name)) => pattern_matches(pattern, tool_name),
            (Some(_), None) => false, // Rule requires tool but none provided
            (None, _) => true,        // Rule applies to all tools
        }
    }
}

/// Check if a pattern matches a value (supports * wildcard)
fn pattern_matches(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    if pattern.contains('*') {
        // Simple wildcard matching
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.len() == 2 {
            let (prefix, suffix) = (parts[0], parts[1]);
            return value.starts_with(prefix) && value.ends_with(suffix);
        }
    }

    pattern == value
}

/// Permission policy for an API key, team, or organization
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PermissionPolicy {
    /// Policy name
    pub name: String,

    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Default permission level for unspecified servers
    #[serde(default)]
    pub default_level: PermissionLevel,

    /// Specific permission rules (evaluated in order)
    #[serde(default)]
    pub rules: Vec<PermissionRule>,

    /// Allowed servers (whitelist, if empty all are allowed)
    #[serde(default, skip_serializing_if = "HashSet::is_empty")]
    pub allowed_servers: HashSet<String>,

    /// Denied servers (blacklist)
    #[serde(default, skip_serializing_if = "HashSet::is_empty")]
    pub denied_servers: HashSet<String>,

    /// Rate limit override (requests per minute)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_rpm: Option<u32>,
}

impl PermissionPolicy {
    /// Create a new policy with a name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Create an allow-all policy
    pub fn allow_all(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            default_level: PermissionLevel::Execute,
            ..Default::default()
        }
    }

    /// Create a deny-all policy
    pub fn deny_all(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            default_level: PermissionLevel::Deny,
            ..Default::default()
        }
    }

    /// Add a permission rule
    pub fn with_rule(mut self, rule: PermissionRule) -> Self {
        self.rules.push(rule);
        self
    }

    /// Allow a specific server
    pub fn allow_server(mut self, server: impl Into<String>) -> Self {
        self.allowed_servers.insert(server.into());
        self
    }

    /// Deny a specific server
    pub fn deny_server(mut self, server: impl Into<String>) -> Self {
        self.denied_servers.insert(server.into());
        self
    }

    /// Check permission for a server access
    pub fn check_server_access(&self, server: &str) -> PermissionLevel {
        // Check deny list first
        if self.denied_servers.contains(server) {
            return PermissionLevel::Deny;
        }

        // Check allow list if not empty
        if !self.allowed_servers.is_empty() && !self.allowed_servers.contains(server) {
            return PermissionLevel::Deny;
        }

        // Check rules in order
        for rule in &self.rules {
            if rule.matches(server, None) {
                return rule.level;
            }
        }

        // Return default level
        self.default_level
    }

    /// Check permission for a tool access
    pub fn check_tool_access(&self, server: &str, tool: &str) -> PermissionLevel {
        // First check server-level access
        let server_level = self.check_server_access(server);
        if server_level == PermissionLevel::Deny {
            return PermissionLevel::Deny;
        }

        // Check tool-specific rules
        for rule in &self.rules {
            if rule.matches(server, Some(tool)) {
                return rule.level;
            }
        }

        // Fall back to server-level permission
        server_level
    }
}

/// Permission manager for MCP access control
#[derive(Debug, Default)]
pub struct PermissionManager {
    /// Policies by API key
    key_policies: HashMap<String, PermissionPolicy>,

    /// Policies by team ID
    team_policies: HashMap<String, PermissionPolicy>,

    /// Policies by organization ID
    org_policies: HashMap<String, PermissionPolicy>,

    /// Default policy for unauthenticated requests
    default_policy: PermissionPolicy,
}

impl PermissionManager {
    /// Create a new permission manager
    pub fn new() -> Self {
        Self {
            default_policy: PermissionPolicy::deny_all("default"),
            ..Default::default()
        }
    }

    /// Create a permission manager that allows all by default
    pub fn allow_all() -> Self {
        Self {
            default_policy: PermissionPolicy::allow_all("default"),
            ..Default::default()
        }
    }

    /// Set policy for an API key
    pub fn set_key_policy(&mut self, key: impl Into<String>, policy: PermissionPolicy) {
        self.key_policies.insert(key.into(), policy);
    }

    /// Set policy for a team
    pub fn set_team_policy(&mut self, team_id: impl Into<String>, policy: PermissionPolicy) {
        self.team_policies.insert(team_id.into(), policy);
    }

    /// Set policy for an organization
    pub fn set_org_policy(&mut self, org_id: impl Into<String>, policy: PermissionPolicy) {
        self.org_policies.insert(org_id.into(), policy);
    }

    /// Set default policy
    pub fn set_default_policy(&mut self, policy: PermissionPolicy) {
        self.default_policy = policy;
    }

    /// Get the effective policy for a request
    pub fn get_effective_policy(
        &self,
        api_key: Option<&str>,
        team_id: Option<&str>,
        org_id: Option<&str>,
    ) -> &PermissionPolicy {
        // Priority: API key > Team > Organization > Default
        if let Some(key) = api_key
            && let Some(policy) = self.key_policies.get(key)
        {
            return policy;
        }

        if let Some(team) = team_id
            && let Some(policy) = self.team_policies.get(team)
        {
            return policy;
        }

        if let Some(org) = org_id
            && let Some(policy) = self.org_policies.get(org)
        {
            return policy;
        }

        &self.default_policy
    }

    /// Check if access to a server is allowed
    pub fn check_server_access(
        &self,
        server: &str,
        api_key: Option<&str>,
        team_id: Option<&str>,
        org_id: Option<&str>,
    ) -> McpResult<PermissionLevel> {
        let policy = self.get_effective_policy(api_key, team_id, org_id);
        let level = policy.check_server_access(server);

        if level == PermissionLevel::Deny {
            return Err(McpError::AuthorizationError {
                server_name: server.to_string(),
                tool_name: None,
                message: "Access denied by permission policy".to_string(),
            });
        }

        Ok(level)
    }

    /// Check if access to a tool is allowed
    pub fn check_tool_access(
        &self,
        server: &str,
        tool: &str,
        api_key: Option<&str>,
        team_id: Option<&str>,
        org_id: Option<&str>,
    ) -> McpResult<PermissionLevel> {
        let policy = self.get_effective_policy(api_key, team_id, org_id);
        let level = policy.check_tool_access(server, tool);

        if level == PermissionLevel::Deny {
            return Err(McpError::AuthorizationError {
                server_name: server.to_string(),
                tool_name: Some(tool.to_string()),
                message: "Access denied by permission policy".to_string(),
            });
        }

        if !level.can_execute() {
            return Err(McpError::AuthorizationError {
                server_name: server.to_string(),
                tool_name: Some(tool.to_string()),
                message: "Execute permission required".to_string(),
            });
        }

        Ok(level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_level_hierarchy() {
        assert!(!PermissionLevel::Deny.can_read());
        assert!(PermissionLevel::Read.can_read());
        assert!(PermissionLevel::Execute.can_read());
        assert!(PermissionLevel::Admin.can_read());

        assert!(!PermissionLevel::Deny.can_execute());
        assert!(!PermissionLevel::Read.can_execute());
        assert!(PermissionLevel::Execute.can_execute());
        assert!(PermissionLevel::Admin.can_execute());

        assert!(!PermissionLevel::Execute.is_admin());
        assert!(PermissionLevel::Admin.is_admin());
    }

    #[test]
    fn test_pattern_matching() {
        assert!(pattern_matches("*", "anything"));
        assert!(pattern_matches("github", "github"));
        assert!(!pattern_matches("github", "gitlab"));

        assert!(pattern_matches("git*", "github"));
        assert!(pattern_matches("git*", "gitlab"));
        assert!(!pattern_matches("git*", "mercurial"));

        assert!(pattern_matches("*_mcp", "github_mcp"));
        assert!(!pattern_matches("*_mcp", "github"));
    }

    #[test]
    fn test_permission_rule_matching() {
        let rule = PermissionRule::new("github", PermissionLevel::Execute);
        assert!(rule.matches("github", None));
        assert!(!rule.matches("gitlab", None));

        let rule_with_tool =
            PermissionRule::new("github", PermissionLevel::Execute).for_tool("get_repo");
        assert!(rule_with_tool.matches("github", Some("get_repo")));
        assert!(!rule_with_tool.matches("github", Some("delete_repo")));
        assert!(!rule_with_tool.matches("github", None));
    }

    #[test]
    fn test_policy_deny_list() {
        let policy = PermissionPolicy::allow_all("test").deny_server("dangerous_server");

        assert_eq!(
            policy.check_server_access("github"),
            PermissionLevel::Execute
        );
        assert_eq!(
            policy.check_server_access("dangerous_server"),
            PermissionLevel::Deny
        );
    }

    #[test]
    fn test_policy_allow_list() {
        let policy = PermissionPolicy::deny_all("test")
            .allow_server("github")
            .with_rule(PermissionRule::new("github", PermissionLevel::Execute));

        assert_eq!(
            policy.check_server_access("github"),
            PermissionLevel::Execute
        );
        assert_eq!(policy.check_server_access("gitlab"), PermissionLevel::Deny);
    }

    #[test]
    fn test_policy_rules_order() {
        let policy = PermissionPolicy::new("test")
            .with_rule(PermissionRule::new("*", PermissionLevel::Read))
            .with_rule(PermissionRule::new("github", PermissionLevel::Execute));

        // First matching rule wins
        assert_eq!(
            policy.check_server_access("github"),
            PermissionLevel::Read // "*" matches first
        );
    }

    #[test]
    fn test_policy_tool_access() {
        // Tool-specific rules should come before general rules
        let policy = PermissionPolicy::new("test")
            .with_rule(PermissionRule::new("github", PermissionLevel::Deny).for_tool("delete_repo"))
            .with_rule(PermissionRule::new("github", PermissionLevel::Execute));

        assert_eq!(
            policy.check_tool_access("github", "get_repo"),
            PermissionLevel::Execute
        );
        assert_eq!(
            policy.check_tool_access("github", "delete_repo"),
            PermissionLevel::Deny
        );
    }

    #[test]
    fn test_permission_manager_key_policy() {
        let mut manager = PermissionManager::new();
        manager.set_key_policy("sk-test123", PermissionPolicy::allow_all("test_key"));

        let level = manager.check_server_access("github", Some("sk-test123"), None, None);
        assert!(level.is_ok());
        assert_eq!(level.unwrap(), PermissionLevel::Execute);
    }

    #[test]
    fn test_permission_manager_default_deny() {
        let manager = PermissionManager::new();

        let result = manager.check_server_access("github", None, None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_permission_manager_priority() {
        let mut manager = PermissionManager::new();

        // Set org policy (lowest priority)
        manager.set_org_policy(
            "org1",
            PermissionPolicy::new("org").with_rule(PermissionRule::new("*", PermissionLevel::Read)),
        );

        // Set key policy (highest priority)
        manager.set_key_policy(
            "sk-admin",
            PermissionPolicy::new("admin")
                .with_rule(PermissionRule::new("*", PermissionLevel::Admin)),
        );

        // Key policy takes precedence
        let policy = manager.get_effective_policy(Some("sk-admin"), None, Some("org1"));
        assert_eq!(policy.name, "admin");

        // Falls back to org policy
        let policy = manager.get_effective_policy(None, None, Some("org1"));
        assert_eq!(policy.name, "org");
    }

    #[test]
    fn test_check_tool_access_requires_execute() {
        let mut manager = PermissionManager::new();
        manager.set_key_policy(
            "sk-reader",
            PermissionPolicy::new("reader")
                .with_rule(PermissionRule::new("*", PermissionLevel::Read)),
        );

        // Read-only should not allow tool execution
        let result =
            manager.check_tool_access("github", "create_issue", Some("sk-reader"), None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_permission_policy_serialization() {
        let policy = PermissionPolicy::new("test")
            .with_rule(PermissionRule::new("github", PermissionLevel::Execute));

        let json = serde_json::to_string(&policy).unwrap();
        let deserialized: PermissionPolicy = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, "test");
        assert_eq!(deserialized.rules.len(), 1);
    }
}
