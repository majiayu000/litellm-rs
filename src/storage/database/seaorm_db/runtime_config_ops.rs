use super::types::SeaOrmDatabase;
use crate::storage::database::entities::runtime_config::{self, Column, Entity};
use crate::utils::error::gateway_error::{GatewayError, Result};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, sea_query::Expr};

impl SeaOrmDatabase {
    /// Load the authoritative encrypted snapshot, including its revision.
    pub async fn runtime_config(&self) -> Result<runtime_config::Model> {
        Entity::find_by_id(1).one(&self.db).await?.ok_or_else(|| {
            GatewayError::Storage("runtime configuration row is missing; run migrations".into())
        })
    }

    /// Persist only if the caller built its candidate from the latest revision.
    pub async fn commit_runtime_config(&self, expected: u64, ciphertext: String) -> Result<u64> {
        let revision = i64::try_from(expected)
            .ok()
            .and_then(|v| v.checked_add(1))
            .ok_or_else(|| GatewayError::Storage("runtime revision overflow".into()))?;
        let result = Entity::update_many()
            .col_expr(Column::Revision, Expr::value(revision))
            .col_expr(Column::EncryptedPayload, Expr::value(ciphertext))
            .filter(Column::Id.eq(1))
            .filter(Column::Revision.eq(revision - 1))
            .exec(&self.db)
            .await?;
        if result.rows_affected != 1 {
            return Err(GatewayError::Conflict(
                "Runtime revision changed; reload configuration before retrying".into(),
            ));
        }
        Ok(revision as u64)
    }
}
