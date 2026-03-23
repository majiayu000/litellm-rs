//! A2A Agent Registry
//!
//! Registry for managing and discovering agents.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::config::AgentConfig;
use super::error::{A2AError, A2AResult};

/// Agent registry entry
#[derive(Debug, Clone)]
pub struct AgentEntry {
    /// Agent configuration
    pub config: AgentConfig,

    /// Agent state
    pub state: AgentState,

    /// Last health check timestamp
    pub last_health_check: Option<chrono::DateTime<chrono::Utc>>,

    /// Total invocation count
    pub invocation_count: u64,

    /// Total cost (USD)
    pub total_cost: f64,
}

impl AgentEntry {
    /// Create a new agent entry
    pub fn new(config: AgentConfig) -> Self {
        Self {
            config,
            state: AgentState::Unknown,
            last_health_check: None,
            invocation_count: 0,
            total_cost: 0.0,
        }
    }

    /// Update invocation stats
    pub fn record_invocation(&mut self, cost: f64) {
        self.invocation_count += 1;
        self.total_cost += cost;
    }
}

/// Agent state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentState {
    /// State unknown (not checked)
    #[default]
    Unknown,

    /// Agent is healthy
    Healthy,

    /// Agent is degraded (slow response)
    Degraded,

    /// Agent is unhealthy (errors)
    Unhealthy,

    /// Agent is disabled
    Disabled,
}

impl AgentState {
    /// Check if agent is available for requests
    pub fn is_available(&self) -> bool {
        matches!(self, AgentState::Healthy | AgentState::Degraded)
    }
}

/// Agent registry for discovery and management
#[derive(Debug)]
pub struct AgentRegistry {
    /// Registered agents
    agents: RwLock<HashMap<String, AgentEntry>>,
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentRegistry {
    /// Create a new registry
    pub fn new() -> Self {
        Self {
            agents: RwLock::new(HashMap::new()),
        }
    }

    /// Register an agent
    pub async fn register(&self, config: AgentConfig) -> A2AResult<()> {
        let name = config.name.clone();

        config
            .validate()
            .map_err(|e| A2AError::ConfigurationError { message: e })?;

        let mut agents = self.agents.write().await;
        if agents.contains_key(&name) {
            return Err(A2AError::AgentAlreadyExists { agent_name: name });
        }

        agents.insert(name, AgentEntry::new(config));
        Ok(())
    }

    /// Unregister an agent
    pub async fn unregister(&self, name: &str) -> Option<AgentEntry> {
        self.agents.write().await.remove(name)
    }

    /// Get an agent by name
    pub async fn get(&self, name: &str) -> Option<AgentEntry> {
        self.agents.read().await.get(name).cloned()
    }

    /// Get an agent for routing, auto-triggering a health check if state is Unknown.
    ///
    /// When an agent has never been health-checked (state == Unknown), this
    /// method probes its URL before returning. This ensures the router never
    /// receives an agent whose availability is undetermined.
    pub async fn get_for_routing(&self, name: &str) -> Option<AgentEntry> {
        let needs_check = {
            let agents = self.agents.read().await;
            agents
                .get(name)
                .is_some_and(|e| e.state == AgentState::Unknown && e.config.enabled)
        };

        if needs_check {
            self.check_agent_health(name).await;
        }

        self.agents.read().await.get(name).cloned()
    }

    /// Get agent configuration
    pub async fn get_config(&self, name: &str) -> Option<AgentConfig> {
        self.agents.read().await.get(name).map(|e| e.config.clone())
    }

    /// Update agent state
    pub async fn update_state(&self, name: &str, state: AgentState) {
        if let Some(entry) = self.agents.write().await.get_mut(name) {
            entry.state = state;
            entry.last_health_check = Some(chrono::Utc::now());
        }
    }

    /// Record an invocation
    pub async fn record_invocation(&self, name: &str, cost: f64) {
        if let Some(entry) = self.agents.write().await.get_mut(name) {
            entry.record_invocation(cost);
        }
    }

    /// List all agent names
    pub async fn list_names(&self) -> Vec<String> {
        self.agents.read().await.keys().cloned().collect()
    }

    /// List all available agents (healthy or degraded state)
    pub async fn list_available(&self) -> Vec<AgentConfig> {
        self.agents
            .read()
            .await
            .values()
            .filter(|e| e.state.is_available() && e.config.enabled)
            .map(|e| e.config.clone())
            .collect()
    }

    /// List agents by tags
    pub async fn list_by_tag(&self, tag: &str) -> Vec<AgentConfig> {
        self.agents
            .read()
            .await
            .values()
            .filter(|e| e.config.tags.contains(&tag.to_string()))
            .map(|e| e.config.clone())
            .collect()
    }

    /// Get agent count
    pub async fn count(&self) -> usize {
        self.agents.read().await.len()
    }

    /// Get registry statistics
    pub async fn stats(&self) -> RegistryStats {
        let agents = self.agents.read().await;

        let mut healthy_agents = 0;
        let mut degraded_agents = 0;
        let mut unhealthy_agents = 0;
        let mut disabled_agents = 0;
        let mut unknown_agents = 0;
        let mut enabled_agents = 0;
        let mut total_invocations = 0;
        let mut total_cost = 0.0;

        for entry in agents.values() {
            match entry.state {
                AgentState::Healthy => healthy_agents += 1,
                AgentState::Degraded => degraded_agents += 1,
                AgentState::Unhealthy => unhealthy_agents += 1,
                AgentState::Disabled => disabled_agents += 1,
                AgentState::Unknown => unknown_agents += 1,
            }

            if entry.config.enabled {
                enabled_agents += 1;
            }

            total_invocations += entry.invocation_count;
            total_cost += entry.total_cost;
        }

        RegistryStats {
            total_agents: agents.len(),
            enabled_agents,
            healthy_agents,
            degraded_agents,
            unhealthy_agents,
            disabled_agents,
            unknown_agents,
            total_invocations,
            total_cost,
        }
    }

    /// Probe a single agent's health via HTTP GET to its URL
    pub async fn check_agent_health(&self, name: &str) {
        let url = {
            let agents = self.agents.read().await;
            match agents.get(name) {
                Some(entry) if entry.config.enabled => entry.config.url.clone(),
                _ => return,
            }
        };

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        let state = match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => AgentState::Healthy,
            Ok(resp) if resp.status().is_server_error() => AgentState::Unhealthy,
            Ok(_) => AgentState::Degraded,
            Err(_) => AgentState::Unhealthy,
        };

        self.update_state(name, state).await;
    }

    /// Run health checks on all registered agents
    pub async fn check_all_agents_health(&self) {
        let names = self.list_names().await;
        for name in names {
            self.check_agent_health(&name).await;
        }
    }

    /// Start a periodic health check background task
    pub fn start_health_check_task(
        registry: Arc<AgentRegistry>,
        interval_secs: u64,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
            // First tick fires immediately, skip it so the caller can
            // trigger an initial probe explicitly instead
            interval.tick().await;

            loop {
                interval.tick().await;
                tracing::debug!("Running periodic A2A agent health checks");
                registry.check_all_agents_health().await;
            }
        })
    }
}

/// Registry statistics
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct RegistryStats {
    /// Total registered agents
    pub total_agents: usize,
    /// Enabled agents
    pub enabled_agents: usize,
    /// Healthy agents
    pub healthy_agents: usize,
    /// Degraded agents
    pub degraded_agents: usize,
    /// Unhealthy agents
    pub unhealthy_agents: usize,
    /// Disabled agents
    pub disabled_agents: usize,
    /// Unknown state agents
    pub unknown_agents: usize,
    /// Total invocations across all agents
    pub total_invocations: u64,
    /// Total cost across all agents
    pub total_cost: f64,
}

/// Thread-safe registry handle
pub type AgentRegistryHandle = Arc<AgentRegistry>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_registry_creation() {
        let registry = AgentRegistry::new();
        assert_eq!(registry.count().await, 0);
    }

    #[tokio::test]
    async fn test_register_agent() {
        let registry = AgentRegistry::new();

        let config = AgentConfig::new("test-agent", "https://example.com/agent");
        registry.register(config).await.unwrap();

        assert_eq!(registry.count().await, 1);
        assert!(registry.get("test-agent").await.is_some());
    }

    #[tokio::test]
    async fn test_register_duplicate() {
        let registry = AgentRegistry::new();

        let config = AgentConfig::new("test-agent", "https://example.com/agent");
        registry.register(config.clone()).await.unwrap();

        let result = registry.register(config).await;
        assert!(matches!(result, Err(A2AError::AgentAlreadyExists { .. })));
    }

    #[tokio::test]
    async fn test_unregister_agent() {
        let registry = AgentRegistry::new();

        let config = AgentConfig::new("test-agent", "https://example.com/agent");
        registry.register(config).await.unwrap();

        let removed = registry.unregister("test-agent").await;
        assert!(removed.is_some());
        assert_eq!(registry.count().await, 0);
    }

    #[tokio::test]
    async fn test_update_state() {
        let registry = AgentRegistry::new();

        let config = AgentConfig::new("test-agent", "https://example.com/agent");
        registry.register(config).await.unwrap();

        registry
            .update_state("test-agent", AgentState::Healthy)
            .await;

        let entry = registry.get("test-agent").await.unwrap();
        assert_eq!(entry.state, AgentState::Healthy);
        assert!(entry.last_health_check.is_some());
    }

    #[tokio::test]
    async fn test_record_invocation() {
        let registry = AgentRegistry::new();

        let config = AgentConfig::new("test-agent", "https://example.com/agent");
        registry.register(config).await.unwrap();

        registry.record_invocation("test-agent", 0.01).await;
        registry.record_invocation("test-agent", 0.02).await;

        let entry = registry.get("test-agent").await.unwrap();
        assert_eq!(entry.invocation_count, 2);
        assert!((entry.total_cost - 0.03).abs() < 0.0001);
    }

    #[tokio::test]
    async fn test_list_available() {
        let registry = AgentRegistry::new();

        // Add enabled agent
        let config1 = AgentConfig::new("agent1", "https://example.com/agent1");
        registry.register(config1).await.unwrap();
        registry.update_state("agent1", AgentState::Healthy).await;

        // Add disabled agent
        let mut config2 = AgentConfig::new("agent2", "https://example.com/agent2");
        config2.enabled = false;
        registry.register(config2).await.unwrap();

        let available = registry.list_available().await;
        assert_eq!(available.len(), 1);
        assert_eq!(available[0].name, "agent1");
    }

    #[tokio::test]
    async fn test_list_by_tag() {
        let registry = AgentRegistry::new();

        let mut config1 = AgentConfig::new("agent1", "https://example.com/agent1");
        config1.tags = vec!["production".to_string()];
        registry.register(config1).await.unwrap();

        let mut config2 = AgentConfig::new("agent2", "https://example.com/agent2");
        config2.tags = vec!["staging".to_string()];
        registry.register(config2).await.unwrap();

        let production = registry.list_by_tag("production").await;
        assert_eq!(production.len(), 1);
        assert_eq!(production[0].name, "agent1");
    }

    #[tokio::test]
    async fn test_registry_stats() {
        let registry = AgentRegistry::new();

        let config1 = AgentConfig::new("agent1", "https://example.com/agent1");
        registry.register(config1).await.unwrap();
        registry.update_state("agent1", AgentState::Healthy).await;
        registry.record_invocation("agent1", 0.10).await;

        let config2 = AgentConfig::new("agent2", "https://example.com/agent2");
        registry.register(config2).await.unwrap();
        registry.update_state("agent2", AgentState::Unhealthy).await;

        let stats = registry.stats().await;
        assert_eq!(stats.total_agents, 2);
        assert_eq!(stats.healthy_agents, 1);
        assert_eq!(stats.unhealthy_agents, 1);
        assert_eq!(stats.total_invocations, 1);
    }

    #[test]
    fn test_agent_state_availability() {
        assert!(AgentState::Healthy.is_available());
        assert!(AgentState::Degraded.is_available());
        assert!(!AgentState::Unknown.is_available());
        assert!(!AgentState::Unhealthy.is_available());
        assert!(!AgentState::Disabled.is_available());
    }

    #[tokio::test]
    async fn test_unknown_agent_excluded_from_available() {
        let registry = AgentRegistry::new();

        let config = AgentConfig::new("agent1", "https://example.com/agent1");
        registry.register(config).await.unwrap();
        // New agents start as Unknown
        let entry = registry.get("agent1").await.unwrap();
        assert_eq!(entry.state, AgentState::Unknown);

        // Unknown agents must not appear in available list
        let available = registry.list_available().await;
        assert!(available.is_empty());

        // After marking healthy, agent becomes available
        registry.update_state("agent1", AgentState::Healthy).await;
        let available = registry.list_available().await;
        assert_eq!(available.len(), 1);
    }

    #[tokio::test]
    async fn test_get_for_routing_triggers_health_check() {
        let registry = AgentRegistry::new();

        // Use an unreachable URL so the health check marks it Unhealthy
        let config = AgentConfig::new("probe-agent", "https://192.0.2.1/health");
        registry.register(config).await.unwrap();

        // Starts as Unknown
        let entry = registry.get("probe-agent").await.unwrap();
        assert_eq!(entry.state, AgentState::Unknown);

        // get_for_routing should trigger a health check
        let entry = registry.get_for_routing("probe-agent").await.unwrap();
        assert_ne!(entry.state, AgentState::Unknown);
        assert!(entry.last_health_check.is_some());
    }

    #[tokio::test]
    async fn test_get_for_routing_skips_known_state() {
        let registry = AgentRegistry::new();

        let config = AgentConfig::new("known-agent", "https://192.0.2.1/health");
        registry.register(config).await.unwrap();
        registry
            .update_state("known-agent", AgentState::Healthy)
            .await;

        // Already Healthy, should not re-probe (returns immediately)
        let entry = registry.get_for_routing("known-agent").await.unwrap();
        assert_eq!(entry.state, AgentState::Healthy);
    }

    #[tokio::test]
    async fn test_check_agent_health_unreachable() {
        let registry = AgentRegistry::new();

        // Use a URL that will fail to connect
        let config = AgentConfig::new("bad-agent", "https://192.0.2.1/health");
        registry.register(config).await.unwrap();

        registry.check_agent_health("bad-agent").await;

        let entry = registry.get("bad-agent").await.unwrap();
        assert_eq!(entry.state, AgentState::Unhealthy);
        assert!(entry.last_health_check.is_some());
    }

    #[tokio::test]
    async fn test_check_agent_health_skips_disabled() {
        let registry = AgentRegistry::new();

        let mut config = AgentConfig::new("disabled-agent", "https://example.com/agent");
        config.enabled = false;
        registry.register(config).await.unwrap();

        registry.check_agent_health("disabled-agent").await;

        // State should remain Unknown (skipped)
        let entry = registry.get("disabled-agent").await.unwrap();
        assert_eq!(entry.state, AgentState::Unknown);
    }

    #[tokio::test]
    async fn test_check_agent_health_nonexistent() {
        let registry = AgentRegistry::new();
        // Should not panic for unknown agent name
        registry.check_agent_health("does-not-exist").await;
    }

    #[tokio::test]
    async fn test_start_health_check_task_runs() {
        let registry = Arc::new(AgentRegistry::new());
        let handle = AgentRegistry::start_health_check_task(registry.clone(), 1);

        // Let it tick at least once
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        handle.abort();
    }
}
