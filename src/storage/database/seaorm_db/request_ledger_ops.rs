use crate::core::request_ledger::{RequestLedgerRecord, RequestLedgerWriter};
use crate::utils::error::gateway_error::{GatewayError, Result};
use chrono::{DateTime, Duration, Utc};
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ActiveValue::Set, ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::debug;

use super::super::entities::{self, request_ledger};
use super::types::SeaOrmDatabase;

/// Minimum seconds between write-path retention deletes on one sink.
const REQUEST_LEDGER_PRUNE_INTERVAL_SECS: i64 = 3600;

/// Equality and time-window filters for the admin ledger query.
#[derive(Debug, Clone, Default)]
pub struct RequestLedgerListFilter {
    /// Inclusive lower bound on `finished_at`.
    pub finished_after: Option<DateTime<Utc>>,
    /// Exclusive upper bound on `finished_at`.
    pub finished_before: Option<DateTime<Utc>>,
    /// Exact request id.
    pub request_id: Option<String>,
    /// Exact model name.
    pub model: Option<String>,
    /// Exact provider name.
    pub provider: Option<String>,
    /// Exact terminal status.
    pub terminal_status: Option<String>,
    /// Seek `finished_at` from the previous page (DESC).
    pub after_finished_at: Option<DateTime<Utc>>,
    /// Seek `request_id` from the previous page (DESC).
    pub after_request_id: Option<String>,
}

/// Database-backed request ledger writer with bounded retention.
#[derive(Clone)]
pub struct RequestLedgerSink {
    db: Arc<SeaOrmDatabase>,
    retention_days: u32,
    last_prune_unix: Arc<AtomicI64>,
}

impl RequestLedgerSink {
    /// Create a sink that stores terminal rows and prunes by `retention_days`.
    pub fn new(db: Arc<SeaOrmDatabase>, retention_days: u32) -> Self {
        Self {
            db,
            retention_days,
            last_prune_unix: Arc::new(AtomicI64::new(0)),
        }
    }

    fn claim_prune_slot(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
            .unwrap_or(0);
        loop {
            let last = self.last_prune_unix.load(Ordering::Relaxed);
            if last != 0 && now.saturating_sub(last) < REQUEST_LEDGER_PRUNE_INTERVAL_SECS {
                return false;
            }
            match self.last_prune_unix.compare_exchange_weak(
                last,
                now.max(1),
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(_) => continue,
            }
        }
    }
}

#[async_trait::async_trait]
impl RequestLedgerWriter for RequestLedgerSink {
    async fn persist(&self, record: RequestLedgerRecord) -> std::result::Result<(), String> {
        self.db
            .upsert_request_ledger(&record)
            .await
            .map_err(|error| error.to_string())?;
        if self.claim_prune_slot() {
            self.db
                .prune_expired_request_ledger(self.retention_days)
                .await
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

impl SeaOrmDatabase {
    /// Persist one metadata-only terminal request-ledger row and prune expired rows.
    pub async fn store_request_ledger(
        &self,
        record: &RequestLedgerRecord,
        retention_days: u32,
    ) -> Result<()> {
        self.upsert_request_ledger(record).await?;
        self.prune_expired_request_ledger(retention_days).await
    }

    async fn upsert_request_ledger(&self, record: &RequestLedgerRecord) -> Result<()> {
        debug!("Persisting request ledger row {}", record.request_id);

        let active_model = request_ledger::ActiveModel {
            request_id: Set(record.request_id.clone()),
            started_at: Set(record.started_at.into()),
            finished_at: Set(record.finished_at.into()),
            method: Set(record.method.clone()),
            endpoint: Set(record.endpoint.clone()),
            model: Set(record.model.clone()),
            provider: Set(record.provider.clone()),
            deployment: Set(record.deployment.clone()),
            status_code: Set(record.status_code),
            terminal_status: Set(record.terminal_status.clone()),
            latency_ms: Set(record.latency_ms),
            prompt_tokens: Set(record.prompt_tokens),
            completion_tokens: Set(record.completion_tokens),
            total_tokens: Set(record.total_tokens),
            cost: Set(record.cost),
            user_id: Set(record.user_id.clone()),
            api_key_id: Set(record.api_key_id.clone()),
            team_id: Set(record.team_id.clone()),
        };

        entities::RequestLedger::insert(active_model)
            .on_conflict(
                OnConflict::column(request_ledger::Column::RequestId)
                    .update_columns([
                        request_ledger::Column::StartedAt,
                        request_ledger::Column::FinishedAt,
                        request_ledger::Column::Method,
                        request_ledger::Column::Endpoint,
                        request_ledger::Column::Model,
                        request_ledger::Column::Provider,
                        request_ledger::Column::Deployment,
                        request_ledger::Column::StatusCode,
                        request_ledger::Column::TerminalStatus,
                        request_ledger::Column::LatencyMs,
                        request_ledger::Column::PromptTokens,
                        request_ledger::Column::CompletionTokens,
                        request_ledger::Column::TotalTokens,
                        request_ledger::Column::Cost,
                        request_ledger::Column::UserId,
                        request_ledger::Column::ApiKeyId,
                        request_ledger::Column::TeamId,
                    ])
                    .to_owned(),
            )
            .exec(&self.db)
            .await
            .map_err(GatewayError::from)?;
        Ok(())
    }

    async fn prune_expired_request_ledger(&self, retention_days: u32) -> Result<()> {
        let cutoff = Utc::now() - Duration::days(i64::from(retention_days.max(1)));
        entities::RequestLedger::delete_many()
            .filter(request_ledger::Column::FinishedAt.lt(cutoff))
            .exec(&self.db)
            .await
            .map_err(GatewayError::from)?;
        Ok(())
    }

    /// List metadata-only ledger rows newest-first, using `(finished_at, request_id)` seek.
    pub async fn list_request_ledger(
        &self,
        filter: &RequestLedgerListFilter,
        limit: u64,
    ) -> Result<Vec<request_ledger::Model>> {
        let mut query = entities::RequestLedger::find();

        if let Some(request_id) = filter.request_id.as_deref() {
            query = query.filter(request_ledger::Column::RequestId.eq(request_id));
        }
        if let Some(model) = filter.model.as_deref() {
            query = query.filter(request_ledger::Column::Model.eq(model));
        }
        if let Some(provider) = filter.provider.as_deref() {
            query = query.filter(request_ledger::Column::Provider.eq(provider));
        }
        if let Some(status) = filter.terminal_status.as_deref() {
            query = query.filter(request_ledger::Column::TerminalStatus.eq(status));
        }
        if let Some(after) = filter.finished_after {
            query = query.filter(request_ledger::Column::FinishedAt.gte(after));
        }
        if let Some(before) = filter.finished_before {
            query = query.filter(request_ledger::Column::FinishedAt.lt(before));
        }
        if let (Some(finished_at), Some(request_id)) =
            (filter.after_finished_at, filter.after_request_id.as_deref())
        {
            query = query.filter(
                Condition::any()
                    .add(request_ledger::Column::FinishedAt.lt(finished_at))
                    .add(
                        Condition::all()
                            .add(request_ledger::Column::FinishedAt.eq(finished_at))
                            .add(request_ledger::Column::RequestId.lt(request_id)),
                    ),
            );
        }

        query
            .order_by_desc(request_ledger::Column::FinishedAt)
            .order_by_desc(request_ledger::Column::RequestId)
            .limit(limit)
            .all(&self.db)
            .await
            .map_err(GatewayError::from)
    }

    #[cfg(test)]
    pub(crate) async fn find_request_ledger(
        &self,
        request_id: &str,
    ) -> Result<Option<request_ledger::Model>> {
        entities::RequestLedger::find_by_id(request_id.to_string())
            .one(&self.db)
            .await
            .map_err(GatewayError::from)
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use crate::config::models::storage::DatabaseConfig;
    use crate::core::request_ledger::RequestLedgerRecord;
    use crate::storage::database::migration::Migrator;
    use chrono::Utc;
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

    fn record(request_id: &str, finished_at: chrono::DateTime<Utc>) -> RequestLedgerRecord {
        RequestLedgerRecord {
            request_id: request_id.to_string(),
            started_at: finished_at,
            finished_at,
            method: "POST".to_string(),
            endpoint: "/v1/chat/completions".to_string(),
            model: Some("gpt-4".to_string()),
            provider: Some("openai".to_string()),
            deployment: Some("openai".to_string()),
            status_code: 200,
            terminal_status: "completed".to_string(),
            latency_ms: 9,
            prompt_tokens: Some(4),
            completion_tokens: Some(6),
            total_tokens: Some(10),
            cost: Some(0.02),
            user_id: None,
            api_key_id: Some("key-id".to_string()),
            team_id: Some("team-id".to_string()),
        }
    }

    #[test]
    fn request_ledger_columns_are_metadata_only() {
        let names: Vec<String> = request_ledger::Column::iter()
            .map(|column| column.as_str().to_lowercase())
            .collect();
        for forbidden in [
            "body",
            "prompt",
            "authorization",
            "header",
            "secret",
            "api_key",
            "raw_key",
        ] {
            assert!(
                !names.iter().any(|name| name == forbidden),
                "{forbidden} must not be a ledger column: {names:?}"
            );
        }
        assert!(names.iter().any(|name| name == "api_key_id"));
        assert!(names.iter().any(|name| name == "request_id"));
    }

    #[tokio::test]
    async fn store_request_ledger_writes_one_terminal_row() {
        let db = test_db().await;
        db.store_request_ledger(&record("req-store", Utc::now()), 30)
            .await
            .expect("store");
        let stored = db
            .find_request_ledger("req-store")
            .await
            .expect("lookup")
            .expect("row");
        assert_eq!(stored.endpoint, "/v1/chat/completions");
        assert_eq!(stored.model.as_deref(), Some("gpt-4"));
        assert_eq!(stored.api_key_id.as_deref(), Some("key-id"));
        assert!(stored.prompt_tokens.is_some());
    }

    #[tokio::test]
    async fn store_request_ledger_prunes_rows_older_than_retention() {
        let db = test_db().await;
        let old = Utc::now() - Duration::days(40);
        db.store_request_ledger(&record("req-old", old), 365)
            .await
            .expect("old row");
        db.store_request_ledger(&record("req-new", Utc::now()), 30)
            .await
            .expect("new row");
        assert!(
            db.find_request_ledger("req-old")
                .await
                .expect("lookup old")
                .is_none()
        );
        assert!(
            db.find_request_ledger("req-new")
                .await
                .expect("lookup new")
                .is_some()
        );
    }

    #[tokio::test]
    async fn request_ledger_sink_throttles_write_path_prune() {
        let sink = RequestLedgerSink::new(Arc::new(test_db().await), 30);
        assert!(sink.claim_prune_slot());
        assert!(!sink.claim_prune_slot());
    }

    #[tokio::test]
    async fn list_request_ledger_filters_and_pages_by_finished_at() {
        let db = test_db().await;
        let t1 = Utc::now() - Duration::seconds(30);
        let t2 = Utc::now() - Duration::seconds(20);
        let t3 = Utc::now() - Duration::seconds(10);
        db.store_request_ledger(&record("req-a", t1), 30)
            .await
            .expect("a");
        db.store_request_ledger(&record("req-b", t2), 30)
            .await
            .expect("b");
        let mut newest = record("req-c", t3);
        newest.model = Some("claude".to_string());
        newest.provider = Some("anthropic".to_string());
        newest.terminal_status = "failed".to_string();
        db.store_request_ledger(&newest, 30).await.expect("c");

        let gpt = db
            .list_request_ledger(
                &RequestLedgerListFilter {
                    model: Some("gpt-4".to_string()),
                    ..RequestLedgerListFilter::default()
                },
                10,
            )
            .await
            .expect("model filter");
        assert_eq!(
            gpt.iter()
                .map(|row| row.request_id.as_str())
                .collect::<Vec<_>>(),
            vec!["req-b", "req-a"]
        );

        let page1 = db
            .list_request_ledger(&RequestLedgerListFilter::default(), 2)
            .await
            .expect("page 1");
        assert_eq!(
            page1
                .iter()
                .map(|row| row.request_id.as_str())
                .collect::<Vec<_>>(),
            vec!["req-c", "req-b"]
        );

        let page2 = db
            .list_request_ledger(
                &RequestLedgerListFilter {
                    after_finished_at: Some(page1[1].finished_at.with_timezone(&Utc)),
                    after_request_id: Some(page1[1].request_id.clone()),
                    ..RequestLedgerListFilter::default()
                },
                2,
            )
            .await
            .expect("page 2");
        assert_eq!(page2.len(), 1);
        assert_eq!(page2[0].request_id, "req-a");

        let by_id = db
            .list_request_ledger(
                &RequestLedgerListFilter {
                    request_id: Some("req-c".to_string()),
                    ..RequestLedgerListFilter::default()
                },
                10,
            )
            .await
            .expect("id filter");
        assert_eq!(by_id.len(), 1);
        assert_eq!(by_id[0].terminal_status, "failed");
        assert_eq!(by_id[0].provider.as_deref(), Some("anthropic"));
    }
}
