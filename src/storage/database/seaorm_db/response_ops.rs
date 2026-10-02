//! Owner-scoped, expiring response records shared by gateway replicas.
use super::types::SeaOrmDatabase;
use crate::storage::database::entities::response::{self, Column, Entity};
use crate::utils::error::gateway_error::Result;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, sea_query::Expr,
};

impl SeaOrmDatabase {
    pub async fn insert_response(&self, record: response::Model, now: i64) -> Result<()> {
        Entity::delete_many()
            .filter(Column::ExpiresAt.lte(now))
            .exec(&self.db)
            .await?;
        // Insert-only: a colliding upstream ID must never transfer ownership.
        record.into_active_model().insert(&self.db).await?;
        Ok(())
    }

    pub async fn owned_response(
        &self,
        id: &str,
        owner: &str,
        now: i64,
    ) -> Result<Option<response::Model>> {
        Ok(Entity::find_by_id(id)
            .filter(Column::Owner.eq(owner))
            .filter(Column::ExpiresAt.gt(now))
            .one(&self.db)
            .await?)
    }

    pub async fn delete_owned_response(&self, id: &str, owner: &str, now: i64) -> Result<bool> {
        Ok(Entity::delete_many()
            .filter(Column::Id.eq(id))
            .filter(Column::Owner.eq(owner))
            .filter(Column::ExpiresAt.gt(now))
            .exec(&self.db)
            .await?
            .rows_affected
            == 1)
    }

    pub async fn start_response_worker(
        &self,
        record: &response::Model,
        response_json: String,
        now: i64,
    ) -> Result<bool> {
        Ok(Entity::update_many()
            .col_expr(Column::ResponseJson, Expr::value(response_json))
            .col_expr(Column::Status, Expr::value("in_progress"))
            .col_expr(Column::Revision, Expr::col(Column::Revision).add(1))
            .filter(Column::Id.eq(&record.id))
            .filter(Column::Owner.eq(&record.owner))
            .filter(Column::Revision.eq(record.revision))
            .filter(Column::Status.eq("queued"))
            .filter(Column::LeaseUntil.gt(now))
            .filter(Column::ExpiresAt.gt(now))
            .exec(&self.db)
            .await?
            .rows_affected
            == 1)
    }

    /// A terminal state is immutable: cancellation and completion cannot overwrite
    /// each other, including when the competing callers run on different replicas.
    pub async fn finish_owned_response(
        &self,
        record: &response::Model,
        response_json: String,
        status: &str,
        now: i64,
    ) -> Result<bool> {
        Ok(Entity::update_many()
            .col_expr(Column::ResponseJson, Expr::value(response_json))
            .col_expr(Column::Status, Expr::value(status))
            .col_expr(Column::Revision, Expr::col(Column::Revision).add(1))
            .col_expr(Column::LeaseUntil, Expr::value(Option::<i64>::None))
            .filter(Column::Id.eq(&record.id))
            .filter(Column::Owner.eq(&record.owner))
            .filter(Column::Revision.eq(record.revision))
            .filter(Column::Status.is_in(["queued", "in_progress"]))
            .filter(Column::ExpiresAt.gt(now))
            .exec(&self.db)
            .await?
            .rows_affected
            == 1)
    }

    pub async fn fail_abandoned_response(
        &self,
        record: &response::Model,
        response_json: String,
        now: i64,
    ) -> Result<bool> {
        Ok(Entity::update_many()
            .col_expr(Column::ResponseJson, Expr::value(response_json))
            .col_expr(Column::Status, Expr::value("failed"))
            .col_expr(Column::Revision, Expr::col(Column::Revision).add(1))
            .col_expr(Column::LeaseUntil, Expr::value(Option::<i64>::None))
            .filter(Column::Id.eq(&record.id))
            .filter(Column::Owner.eq(&record.owner))
            .filter(Column::Revision.eq(record.revision))
            .filter(Column::LeaseUntil.lte(now))
            .filter(Column::Status.is_in(["queued", "in_progress"]))
            .filter(Column::ExpiresAt.gt(now))
            .exec(&self.db)
            .await?
            .rows_affected
            == 1)
    }

    /// Only adapter-owned work has a worker lease. Native background work is
    /// reconciled against its bound upstream, not declared failed by this timer.
    pub async fn renew_response_lease(&self, id: &str, owner: &str, now: i64) -> Result<bool> {
        Ok(Entity::update_many()
            .col_expr(Column::LeaseUntil, Expr::value(now + 60))
            .filter(Column::Id.eq(id))
            .filter(Column::Owner.eq(owner))
            .filter(Column::LeaseUntil.is_not_null())
            .filter(Column::LeaseUntil.gt(now))
            .filter(Column::Status.is_in(["queued", "in_progress"]))
            .filter(Column::ExpiresAt.gt(now))
            .exec(&self.db)
            .await?
            .rows_affected
            == 1)
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use crate::config::models::storage::DatabaseConfig;
    use crate::storage::database::Database;

    fn row(id: &str, now: i64) -> response::Model {
        response::Model {
            id: id.into(),
            owner: "owner-a".into(),
            response_json: "{}".into(),
            input_json: "[]".into(),
            deployment_id: Some("deployment-a".into()),
            deployment_binding: Some("binding-a".into()),
            background: true,
            status: "in_progress".into(),
            expires_at: now + 100,
            lease_until: None,
            revision: 0,
        }
    }

    #[tokio::test]
    async fn two_connections_and_restart_preserve_owner_expiry_and_atomic_terminal_state() {
        let dir = tempfile::tempdir().unwrap();
        let config = DatabaseConfig {
            enabled: true,
            url: format!(
                "sqlite://{}?mode=rwc",
                dir.path().join("responses.db").display()
            ),
            ..Default::default()
        };
        let first = Database::new(&config).await.unwrap();
        first.migrate().await.unwrap();
        let second = Database::new(&config).await.unwrap();
        first
            .insert_response(row("response-1", 1000), 1000)
            .await
            .unwrap();
        let record = second
            .owned_response("response-1", "owner-a", 1001)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(record.deployment_id.as_deref(), Some("deployment-a"));
        assert!(
            second
                .owned_response("response-1", "owner-b", 1001)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            !second
                .delete_owned_response("response-1", "owner-b", 1001)
                .await
                .unwrap()
        );
        assert!(
            second
                .finish_owned_response(
                    &record,
                    "{\"status\":\"cancelled\"}".into(),
                    "cancelled",
                    1001
                )
                .await
                .unwrap()
        );
        assert!(
            !first
                .finish_owned_response(&record, "{}".into(), "completed", 1001)
                .await
                .unwrap()
        );
        drop(first);
        drop(second);
        let restarted = Database::new(&config).await.unwrap();
        assert_eq!(
            restarted
                .owned_response("response-1", "owner-a", 1002)
                .await
                .unwrap()
                .unwrap()
                .status,
            "cancelled"
        );
        assert!(
            restarted
                .owned_response("response-1", "owner-a", 1100)
                .await
                .unwrap()
                .is_none()
        );
        restarted
            .insert_response(row("response-2", 1100), 1100)
            .await
            .unwrap();
        assert!(
            restarted
                .delete_owned_response("response-2", "owner-a", 1101)
                .await
                .unwrap()
        );
        assert!(
            restarted
                .owned_response("response-2", "owner-a", 1101)
                .await
                .unwrap()
                .is_none()
        );
    }
    #[tokio::test]
    async fn worker_start_and_expiration_cannot_overwrite_cancellation_or_renewal() {
        let db = Database::new(&DatabaseConfig {
            enabled: false,
            ..Default::default()
        })
        .await
        .unwrap();
        db.migrate().await.unwrap();
        let mut record = row("worker", 1000);
        record.status = "queued".into();
        record.lease_until = Some(1060);
        db.insert_response(record.clone(), 1000).await.unwrap();
        assert!(
            db.start_response_worker(&record, "{}".into(), 1001)
                .await
                .unwrap()
        );
        assert!(
            !db.start_response_worker(&record, "{}".into(), 1001)
                .await
                .unwrap()
        );
        let running = db
            .owned_response("worker", "owner-a", 1001)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(running.status, "in_progress");
        assert!(
            db.renew_response_lease("worker", "owner-a", 1059)
                .await
                .unwrap()
        );
        assert!(
            !db.fail_abandoned_response(&running, "{}".into(), 1060)
                .await
                .unwrap()
        );
        assert!(
            db.finish_owned_response(&running, "{}".into(), "cancelled", 1060)
                .await
                .unwrap()
        );
        assert!(
            !db.renew_response_lease("worker", "owner-a", 1061)
                .await
                .unwrap()
        );
    }
}
