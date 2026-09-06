//! Shared atomic backend for provider/model budget leases.
//!
//! In-process DashMap remains the default. When a live Redis pool is attached,
//! reserve/settle/cancel run as single-key Lua so multiple gateways cannot
//! overspend. Redis errors fail closed.

use super::BudgetAmount;
#[cfg(feature = "gateway")]
use super::BudgetAmountError;
use super::tracker::BudgetReservationError;
use super::types::ResetPeriod;
use chrono::{Datelike, NaiveDate, Utc, Weekday};
#[cfg(feature = "gateway")]
use std::sync::Arc;
#[cfg(feature = "gateway")]
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::warn;

pub(crate) const DEFAULT_LEASE_TTL_MS: i64 = 600_000;

#[derive(Clone, Copy, Debug)]
pub(crate) enum BudgetLeaseScope {
    Provider,
    Model,
}

impl BudgetLeaseScope {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Provider => "provider",
            Self::Model => "model",
        }
    }

    pub(crate) fn exceeded_error(self) -> BudgetReservationError {
        match self {
            Self::Provider => BudgetReservationError::ProviderBudgetExceeded,
            Self::Model => BudgetReservationError::ModelBudgetExceeded,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) enum BudgetLeaseBackend {
    #[default]
    InProcess,
    #[cfg(feature = "gateway")]
    Redis {
        pool: Arc<crate::storage::redis::RedisPool>,
        lease_ttl_ms: i64,
    },
    #[cfg(test)]
    Unavailable,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LeaseSnapshot {
    pub committed: BudgetAmount,
    pub outstanding: BudgetAmount,
}

#[derive(Debug, Clone)]
pub(crate) struct ReservedLease {
    pub lease_id: String,
    pub period_epoch: i64,
    pub snapshot: LeaseSnapshot,
}

impl BudgetLeaseBackend {
    #[cfg(feature = "gateway")]
    pub(crate) fn redis(pool: Arc<crate::storage::redis::RedisPool>) -> Self {
        Self::redis_with_ttl(pool, DEFAULT_LEASE_TTL_MS)
    }

    #[cfg(feature = "gateway")]
    pub(crate) fn redis_with_ttl(
        pool: Arc<crate::storage::redis::RedisPool>,
        lease_ttl_ms: i64,
    ) -> Self {
        if pool.is_noop() {
            Self::InProcess
        } else {
            Self::Redis {
                pool,
                lease_ttl_ms: lease_ttl_ms.max(1),
            }
        }
    }

    pub(crate) fn is_distributed(&self) -> bool {
        !matches!(self, Self::InProcess)
    }

    pub(crate) fn reserve(
        &self,
        scope: BudgetLeaseScope,
        name: &str,
        amount: BudgetAmount,
        max: BudgetAmount,
        seed_committed: BudgetAmount,
        period_epoch: i64,
    ) -> Result<ReservedLease, BudgetReservationError> {
        match self {
            Self::InProcess => Err(BudgetReservationError::BackendUnavailable),
            #[cfg(test)]
            Self::Unavailable => {
                warn!(
                    scope = scope.as_str(),
                    name, "budget reserve backend unavailable; failing closed"
                );
                Err(BudgetReservationError::BackendUnavailable)
            }
            #[cfg(feature = "gateway")]
            Self::Redis { pool, lease_ttl_ms } => {
                let lease_id = uuid::Uuid::new_v4().to_string();
                let state = run_redis(
                    scope,
                    name,
                    "reserve",
                    pool.budget_reserve(crate::storage::redis::budget::BudgetReserveArgs {
                        key: &crate::storage::redis::RedisPool::budget_lease_key(
                            scope.as_str(),
                            name,
                        ),
                        amount: to_i64(amount)?,
                        max: to_i64(max)?,
                        seed_committed: to_i64(seed_committed)?,
                        period_epoch,
                        lease_id: &lease_id,
                        now_ms: now_ms(),
                        ttl_ms: *lease_ttl_ms,
                    }),
                )?;
                if !state.allowed {
                    return Err(scope.exceeded_error());
                }
                Ok(ReservedLease {
                    lease_id,
                    period_epoch,
                    snapshot: lease_snapshot(state)?,
                })
            }
        }
    }

    pub(crate) fn settle(
        &self,
        scope: BudgetLeaseScope,
        name: &str,
        lease_id: &str,
        reserved: BudgetAmount,
        actual: BudgetAmount,
        period_epoch: i64,
    ) -> Result<LeaseSnapshot, BudgetReservationError> {
        match self {
            Self::InProcess => Err(BudgetReservationError::BackendUnavailable),
            #[cfg(test)]
            Self::Unavailable => Err(BudgetReservationError::BackendUnavailable),
            #[cfg(feature = "gateway")]
            Self::Redis { pool, .. } => {
                let state = run_redis(
                    scope,
                    name,
                    "settle",
                    pool.budget_settle(
                        &crate::storage::redis::RedisPool::budget_lease_key(scope.as_str(), name),
                        to_i64(reserved)?,
                        to_i64(actual)?,
                        period_epoch,
                        lease_id,
                        now_ms(),
                    ),
                )?;
                lease_snapshot(state)
            }
        }
    }

    pub(crate) fn cancel(
        &self,
        scope: BudgetLeaseScope,
        name: &str,
        lease_id: &str,
        reserved: BudgetAmount,
        period_epoch: i64,
    ) -> Result<LeaseSnapshot, BudgetReservationError> {
        match self {
            Self::InProcess => Err(BudgetReservationError::BackendUnavailable),
            #[cfg(test)]
            Self::Unavailable => Ok(LeaseSnapshot {
                committed: BudgetAmount::zero(),
                outstanding: BudgetAmount::zero(),
            }),
            #[cfg(feature = "gateway")]
            Self::Redis { pool, .. } => {
                let state = run_redis(
                    scope,
                    name,
                    "cancel",
                    pool.budget_cancel(
                        &crate::storage::redis::RedisPool::budget_lease_key(scope.as_str(), name),
                        to_i64(reserved)?,
                        period_epoch,
                        lease_id,
                        now_ms(),
                    ),
                )?;
                lease_snapshot(state)
            }
        }
    }

    pub(crate) fn spawn_cancel(
        &self,
        scope: BudgetLeaseScope,
        name: &str,
        lease_id: String,
        reserved: BudgetAmount,
        period_epoch: i64,
    ) {
        #[cfg(feature = "gateway")]
        if let Self::Redis { pool, .. } = self {
            let Ok(reserved) = to_i64(reserved) else {
                return;
            };
            let pool = Arc::clone(pool);
            let key = crate::storage::redis::RedisPool::budget_lease_key(scope.as_str(), name);
            let Ok(handle) = tokio::runtime::Handle::try_current() else {
                warn!(
                    scope = scope.as_str(),
                    name, "budget lease drop-cancel has no runtime; relying on expiry"
                );
                return;
            };
            handle.spawn(async move {
                if let Err(err) = pool
                    .budget_cancel(&key, reserved, period_epoch, &lease_id, now_ms())
                    .await
                {
                    warn!(
                        error = %err,
                        "budget lease drop-cancel failed; relying on expiry"
                    );
                }
            });
            return;
        }
        let _ = (scope, name, lease_id, reserved, period_epoch);
    }

    pub(crate) fn reset(
        &self,
        scope: BudgetLeaseScope,
        name: &str,
        period_epoch: i64,
    ) -> Result<LeaseSnapshot, BudgetReservationError> {
        match self {
            Self::InProcess => Ok(LeaseSnapshot {
                committed: BudgetAmount::zero(),
                outstanding: BudgetAmount::zero(),
            }),
            #[cfg(test)]
            Self::Unavailable => Ok(LeaseSnapshot {
                committed: BudgetAmount::zero(),
                outstanding: BudgetAmount::zero(),
            }),
            #[cfg(feature = "gateway")]
            Self::Redis { pool, .. } => {
                let state = run_redis(
                    scope,
                    name,
                    "reset",
                    pool.budget_reset(
                        &crate::storage::redis::RedisPool::budget_lease_key(scope.as_str(), name),
                        period_epoch,
                        now_ms(),
                        true,
                    ),
                )?;
                lease_snapshot(state)
            }
        }
    }
}

pub(crate) fn budget_period_epoch(period: ResetPeriod, now: chrono::DateTime<Utc>) -> i64 {
    match period {
        ResetPeriod::Never => -1,
        ResetPeriod::Daily => now
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .map(|ndt| ndt.and_utc().timestamp())
            .unwrap_or(0),
        ResetPeriod::Weekly => {
            let iso = now.iso_week();
            NaiveDate::from_isoywd_opt(iso.year(), iso.week(), Weekday::Mon)
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .map(|ndt| ndt.and_utc().timestamp())
                .unwrap_or(0)
        }
        ResetPeriod::Monthly => NaiveDate::from_ymd_opt(now.year(), now.month(), 1)
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|ndt| ndt.and_utc().timestamp())
            .unwrap_or(0),
    }
}

#[cfg(feature = "gateway")]
fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(feature = "gateway")]
fn to_i64(amount: BudgetAmount) -> Result<i64, BudgetReservationError> {
    i64::try_from(amount.as_scaled())
        .map_err(|_| BudgetReservationError::InvalidAmount(BudgetAmountError::Overflow))
}

#[cfg(feature = "gateway")]
fn from_i64(value: i64) -> BudgetAmount {
    BudgetAmount::from_scaled(i128::from(value.max(0)))
}

#[cfg(feature = "gateway")]
fn lease_snapshot(
    state: crate::storage::redis::budget::BudgetLeaseState,
) -> Result<LeaseSnapshot, BudgetReservationError> {
    Ok(LeaseSnapshot {
        committed: from_i64(state.committed),
        outstanding: from_i64(state.outstanding),
    })
}

#[cfg(feature = "gateway")]
fn run_redis<T>(
    scope: BudgetLeaseScope,
    name: &str,
    operation: &'static str,
    fut: impl std::future::Future<Output = crate::utils::error::gateway_error::Result<T>>,
) -> Result<T, BudgetReservationError> {
    let handle = tokio::runtime::Handle::try_current().map_err(|_| {
        warn!(
            scope = scope.as_str(),
            name, operation, "budget redis called without a tokio runtime; failing closed"
        );
        BudgetReservationError::BackendUnavailable
    })?;
    tokio::task::block_in_place(|| handle.block_on(fut)).map_err(|err| {
        warn!(
            scope = scope.as_str(),
            name,
            operation,
            error = %err,
            "budget redis operation failed; failing closed"
        );
        BudgetReservationError::BackendUnavailable
    })
}
