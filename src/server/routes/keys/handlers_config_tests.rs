use super::*;

#[test]
fn test_create_key_config_from_request() {
    let request = CreateKeyRequest {
        name: "Test Key".to_string(),
        description: Some("A test".to_string()),
        user_id: None,
        team_id: None,
        budget_id: None,
        max_budget: None,
        permissions: None,
        rate_limits: None,
        expires_at: None,
        metadata: None,
    };

    let config = CreateKeyConfig {
        name: request.name.clone(),
        description: request.description.clone(),
        user_id: request.user_id,
        team_id: request.team_id,
        budget_id: request.budget_id,
        max_budget: None,
        permissions: request.permissions.clone().unwrap_or_default(),
        rate_limits: request.rate_limits.clone().unwrap_or_default(),
        expires_at: request.expires_at,
        metadata: request.metadata.clone().unwrap_or(serde_json::Value::Null),
    };

    assert_eq!(config.name, "Test Key");
    assert!(config.description.is_some());
}
