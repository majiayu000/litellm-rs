use crate::utils::error::gateway_error::{GatewayError, Result};
use chrono::Utc;
use sea_orm::{ActiveValue::Set, EntityTrait, QueryOrder};
use tracing::debug;

use super::super::entities::{self, provider_config_revision};
use super::types::SeaOrmDatabase;

impl SeaOrmDatabase {
    /// Persist one sanitized provider-config revision after a successful apply.
    pub async fn insert_provider_config_revision(
        &self,
        generation: u64,
        actor: &str,
        sanitized_payload: serde_json::Value,
    ) -> Result<()> {
        debug!("Persisting provider config revision generation={generation}");
        let generation = i64::try_from(generation).map_err(|_| {
            GatewayError::Storage("provider config revision generation overflow".into())
        })?;
        let active_model = provider_config_revision::ActiveModel {
            id: Default::default(),
            generation: Set(generation),
            actor: Set(actor.to_string()),
            sanitized_payload: Set(sanitized_payload),
            created_at: Set(Utc::now().into()),
        };
        entities::ProviderConfigRevision::insert(active_model)
            .exec(&self.db)
            .await
            .map_err(GatewayError::from)?;
        Ok(())
    }

    /// Latest sanitized revision, if any admin mutation has succeeded.
    pub async fn latest_provider_config_revision(
        &self,
    ) -> Result<Option<provider_config_revision::Model>> {
        entities::ProviderConfigRevision::find()
            .order_by_desc(provider_config_revision::Column::Generation)
            .one(&self.db)
            .await
            .map_err(GatewayError::from)
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use crate::config::models::storage::DatabaseConfig;
    use crate::storage::database::migration::Migrator;
    use sea_orm::IdenStatic;
    use sea_orm::Iterable;

    async fn test_db() -> SeaOrmDatabase {
        let db = SeaOrmDatabase::new(&DatabaseConfig {
            enabled: false,
            ..DatabaseConfig::default()
        })
        .await
        .expect("in-memory sqlite");
        Migrator::up(db.connection(), None)
            .await
            .expect("migrations");
        db
    }

    #[test]
    fn provider_config_revision_columns_are_sanitized() {
        let names: Vec<String> = provider_config_revision::Column::iter()
            .map(|column| column.as_str().to_lowercase())
            .collect();
        for forbidden in ["api_key", "authorization", "header", "secret", "raw_key"] {
            assert!(
                !names.iter().any(|name| name == forbidden),
                "{forbidden} must not be a revision column: {names:?}"
            );
        }
        assert!(names.iter().any(|name| name == "sanitized_payload"));
        assert!(names.iter().any(|name| name == "generation"));
        assert!(names.iter().any(|name| name == "actor"));
    }

    #[tokio::test]
    async fn insert_provider_config_revision_stores_sanitized_json() {
        let db = test_db().await;
        let payload = serde_json::json!({
            "operation": "create",
            "providers": [{
                "name": "admin-openai",
                "provider_type": "openai",
                "enabled": true,
                "api_key_ref": "LITELLM_TEST_PROVIDER_KEY"
            }]
        });
        db.insert_provider_config_revision(3, "actor-1", payload.clone())
            .await
            .expect("insert");
        let stored = db
            .latest_provider_config_revision()
            .await
            .expect("latest")
            .expect("row");
        assert_eq!(stored.generation, 3);
        assert_eq!(stored.actor, "actor-1");
        assert_eq!(stored.sanitized_payload, payload);
        let serialized = stored.sanitized_payload.to_string();
        assert!(!serialized.contains("sk-"));
        assert!(!serialized.contains("api_key\":"));
    }
}
