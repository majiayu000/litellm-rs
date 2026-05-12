use crate::core::budget::{
    BudgetLimitKind, BudgetLimitSnapshot, BudgetPersistenceEvent, BudgetPersistenceSender,
    Currency, ResetPeriod,
};
use crate::utils::error::gateway_error::{GatewayError, Result};
use sea_orm::sea_query::OnConflict;
use sea_orm::*;
use std::sync::Arc;
use tokio::{sync::mpsc, task::JoinHandle};
use tracing::{debug, warn};

use super::super::entities::{self, budget_limit_snapshot};
use super::types::SeaOrmDatabase;

fn scope_key(kind: BudgetLimitKind, name: &str) -> String {
    format!("{}:{}", kind.as_str(), name)
}

fn parse_kind(raw: &str) -> Result<BudgetLimitKind> {
    match raw {
        "provider" => Ok(BudgetLimitKind::Provider),
        "model" => Ok(BudgetLimitKind::Model),
        other => Err(GatewayError::Storage(format!(
            "Invalid budget scope type in database: {}",
            other
        ))),
    }
}

fn parse_reset_period(raw: &str) -> Result<ResetPeriod> {
    match raw {
        "daily" => Ok(ResetPeriod::Daily),
        "weekly" => Ok(ResetPeriod::Weekly),
        "monthly" => Ok(ResetPeriod::Monthly),
        "never" => Ok(ResetPeriod::Never),
        other => Err(GatewayError::Storage(format!(
            "Invalid budget reset period in database: {}",
            other
        ))),
    }
}

fn parse_currency(raw: &str) -> Result<Currency> {
    match raw {
        "USD" => Ok(Currency::USD),
        "EUR" => Ok(Currency::EUR),
        "GBP" => Ok(Currency::GBP),
        "JPY" => Ok(Currency::JPY),
        "CNY" => Ok(Currency::CNY),
        other => Err(GatewayError::Storage(format!(
            "Invalid budget currency in database: {}",
            other
        ))),
    }
}

impl budget_limit_snapshot::Model {
    fn to_snapshot(&self) -> Result<BudgetLimitSnapshot> {
        Ok(BudgetLimitSnapshot {
            kind: parse_kind(&self.scope_type)?,
            name: self.scope_name.clone(),
            max_budget: self.max_budget,
            current_spend: self.current_spend,
            soft_limit: self.soft_limit,
            reset_period: parse_reset_period(&self.reset_period)?,
            currency: parse_currency(&self.currency)?,
            enabled: self.enabled,
            last_reset_at: self.last_reset_at.map(|dt| dt.with_timezone(&chrono::Utc)),
            request_count: self.request_count.max(0) as u64,
        })
    }
}

impl SeaOrmDatabase {
    /// Load all persisted provider/model budget snapshots.
    pub async fn load_budget_limit_snapshots(&self) -> Result<Vec<BudgetLimitSnapshot>> {
        let models = entities::BudgetLimitSnapshot::find()
            .all(&self.db)
            .await
            .map_err(GatewayError::from)?;

        models
            .iter()
            .map(budget_limit_snapshot::Model::to_snapshot)
            .collect()
    }

    /// Upsert one provider/model budget snapshot.
    pub async fn upsert_budget_limit_snapshot(&self, snapshot: &BudgetLimitSnapshot) -> Result<()> {
        debug!("Persisting budget snapshot {}", snapshot.scope_key());

        let now = chrono::Utc::now();
        let request_count = snapshot.request_count.min(i64::MAX as u64) as i64;
        let active_model = budget_limit_snapshot::ActiveModel {
            scope_key: Set(snapshot.scope_key()),
            scope_type: Set(snapshot.kind.as_str().to_string()),
            scope_name: Set(snapshot.name.clone()),
            max_budget: Set(snapshot.max_budget),
            current_spend: Set(snapshot.current_spend),
            soft_limit: Set(snapshot.soft_limit),
            reset_period: Set(snapshot.reset_period.to_string()),
            currency: Set(snapshot.currency.to_string()),
            enabled: Set(snapshot.enabled),
            request_count: Set(request_count),
            last_reset_at: Set(snapshot.last_reset_at.map(Into::into)),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };

        entities::BudgetLimitSnapshot::insert(active_model)
            .on_conflict(
                OnConflict::column(budget_limit_snapshot::Column::ScopeKey)
                    .update_columns([
                        budget_limit_snapshot::Column::ScopeType,
                        budget_limit_snapshot::Column::ScopeName,
                        budget_limit_snapshot::Column::MaxBudget,
                        budget_limit_snapshot::Column::CurrentSpend,
                        budget_limit_snapshot::Column::SoftLimit,
                        budget_limit_snapshot::Column::ResetPeriod,
                        budget_limit_snapshot::Column::Currency,
                        budget_limit_snapshot::Column::Enabled,
                        budget_limit_snapshot::Column::RequestCount,
                        budget_limit_snapshot::Column::LastResetAt,
                        budget_limit_snapshot::Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec(&self.db)
            .await
            .map_err(GatewayError::from)?;

        Ok(())
    }

    /// Delete one persisted provider/model budget snapshot.
    pub async fn delete_budget_limit_snapshot(
        &self,
        kind: BudgetLimitKind,
        name: &str,
    ) -> Result<()> {
        debug!("Deleting budget snapshot {}", scope_key(kind, name));
        entities::BudgetLimitSnapshot::delete_by_id(scope_key(kind, name))
            .exec(&self.db)
            .await
            .map_err(GatewayError::from)?;

        Ok(())
    }

    /// Start a lightweight background worker that persists budget mutations.
    pub fn start_budget_limit_persistence_task(
        self: Arc<Self>,
    ) -> (BudgetPersistenceSender, JoinHandle<()>) {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let handle = tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                let result = match event {
                    BudgetPersistenceEvent::Upsert(snapshot) => {
                        self.upsert_budget_limit_snapshot(&snapshot).await
                    }
                    BudgetPersistenceEvent::Delete { kind, name } => {
                        self.delete_budget_limit_snapshot(kind, &name).await
                    }
                };

                if let Err(err) = result {
                    warn!("Failed to persist budget limit snapshot: {}", err);
                }
            }
        });
        (tx, handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::storage::DatabaseConfig;

    #[tokio::test]
    async fn persists_budget_limit_snapshot_round_trip() {
        let db = SeaOrmDatabase::new(&DatabaseConfig::default())
            .await
            .expect("database should initialize");
        db.migrate().await.expect("migrations should run");

        let snapshot = BudgetLimitSnapshot {
            kind: BudgetLimitKind::Provider,
            name: "openai".to_string(),
            max_budget: 100.0,
            current_spend: 42.0,
            soft_limit: 80.0,
            reset_period: ResetPeriod::Monthly,
            currency: Currency::USD,
            enabled: true,
            last_reset_at: None,
            request_count: 7,
        };

        db.upsert_budget_limit_snapshot(&snapshot)
            .await
            .expect("snapshot should persist");

        let snapshots = db
            .load_budget_limit_snapshots()
            .await
            .expect("snapshots should load");
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].kind, BudgetLimitKind::Provider);
        assert_eq!(snapshots[0].name, "openai");
        assert_eq!(snapshots[0].current_spend, 42.0);
        assert_eq!(snapshots[0].request_count, 7);

        db.delete_budget_limit_snapshot(BudgetLimitKind::Provider, "openai")
            .await
            .expect("snapshot should delete");
        assert!(
            db.load_budget_limit_snapshots()
                .await
                .expect("snapshots should load")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn budget_persistence_worker_drains_queued_events_when_sender_drops() {
        let db = Arc::new(
            SeaOrmDatabase::new(&DatabaseConfig::default())
                .await
                .expect("database should initialize"),
        );
        db.migrate().await.expect("migrations should run");

        let (tx, handle) = Arc::clone(&db).start_budget_limit_persistence_task();
        tx.send(BudgetPersistenceEvent::Upsert(BudgetLimitSnapshot {
            kind: BudgetLimitKind::Provider,
            name: "openai".to_string(),
            max_budget: 100.0,
            current_spend: 12.5,
            soft_limit: 80.0,
            reset_period: ResetPeriod::Monthly,
            currency: Currency::USD,
            enabled: true,
            last_reset_at: None,
            request_count: 1,
        }))
        .expect("worker should accept queued event");

        drop(tx);
        handle.await.expect("worker should exit after sender drop");

        let snapshots = db
            .load_budget_limit_snapshots()
            .await
            .expect("snapshot should load");
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].name, "openai");
        assert_eq!(snapshots[0].current_spend, 12.5);
        assert_eq!(snapshots[0].request_count, 1);
    }
}
