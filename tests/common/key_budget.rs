//! Persistent API-key fixtures shared by HTTP integration tests.
use litellm_rs::{
    core::keys::CreateKeyConfig, server::state::AppState, storage::database::Database,
};
use uuid::Uuid;

pub async fn create(state: &AppState, max_budget: f64) -> Uuid {
    let (id, _) = state
        .key_manager
        .generate_key(CreateKeyConfig {
            name: "integration budget".into(),
            max_budget: Some(max_budget),
            ..Default::default()
        })
        .await
        .expect("create persisted key and budget");
    state
        .key_manager
        .get_key(id)
        .await
        .unwrap()
        .unwrap()
        .budget_id
        .unwrap()
}

pub async fn spend(database: &Database, id: Uuid) -> f64 {
    database
        .api_key_budget(id)
        .await
        .unwrap()
        .unwrap()
        .used_amount as f64
        / 1e9
}
