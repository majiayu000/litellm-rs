//! Authoritative API-key lifetime budgets. Amounts are integer nanodollars.
use super::SeaOrmDatabase;
use crate::core::budget::{BudgetAmount, BudgetReservationError};
use crate::storage::database::entities::{
    api_key_budget as budget, api_key_budget_reservation as receipt,
};
use crate::utils::error::gateway_error::{GatewayError, Result};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, Set,
    TransactionTrait, sea_query::Expr,
};
use uuid::Uuid;

pub(crate) fn units(amount: f64) -> Result<i64> {
    let amount =
        BudgetAmount::from_f64(amount).map_err(|e| GatewayError::validation(e.to_string()))?;
    i64::try_from(amount.as_scaled())
        .map_err(|_| GatewayError::validation("Budget amount is too large"))
}

/// Called in the same transaction as the key mutation; changing a cap never resets spend.
pub(super) async fn write_budget(tx: &DatabaseTransaction, id: Uuid, max: f64) -> Result<()> {
    let max = units(max)?;
    if max <= 0 {
        return Err(GatewayError::validation("max_budget must be positive"));
    }
    let changed = budget::Entity::update_many()
        .col_expr(budget::Column::MaxAmount, Expr::value(max))
        .filter(budget::Column::Id.eq(id))
        .exec(tx)
        .await?
        .rows_affected;
    if changed == 0 {
        budget::ActiveModel {
            id: Set(id),
            max_amount: Set(max),
            used_amount: Set(0),
        }
        .insert(tx)
        .await?;
    }
    Ok(())
}

impl SeaOrmDatabase {
    pub async fn api_key_budget(&self, id: Uuid) -> Result<Option<budget::Model>> {
        Ok(budget::Entity::find_by_id(id).one(&self.db).await?)
    }

    pub(crate) async fn reserve_api_key_budget(
        &self,
        id: Uuid,
        amount: f64,
    ) -> std::result::Result<Uuid, BudgetReservationError> {
        let amount = units(amount).map_err(|_| {
            BudgetReservationError::InvalidAmount(crate::core::budget::BudgetAmountError::Overflow)
        })?;
        let result: Result<Option<Uuid>> = async {
            let tx = self.db.begin().await?;
            let changed = budget::Entity::update_many()
                .col_expr(
                    budget::Column::UsedAmount,
                    Expr::col(budget::Column::UsedAmount).add(amount),
                )
                .filter(budget::Column::Id.eq(id))
                .filter(
                    Expr::col(budget::Column::UsedAmount)
                        .lte(Expr::col(budget::Column::MaxAmount).sub(amount)),
                )
                .exec(&tx)
                .await?
                .rows_affected;
            if changed != 1 {
                tx.rollback().await?;
                return Ok(None);
            }
            let reservation_id = Uuid::new_v4();
            receipt::ActiveModel {
                id: Set(reservation_id),
                budget_id: Set(id),
                reserved_amount: Set(amount),
                charged_amount: Set(None),
                created_at: Set(chrono::Utc::now().timestamp()),
            }
            .insert(&tx)
            .await?;
            tx.commit().await?;
            Ok(Some(reservation_id))
        }
        .await;
        match result {
            Ok(Some(id)) => Ok(id),
            Ok(None) => Err(BudgetReservationError::BudgetExceeded),
            Err(error) => {
                tracing::error!(%error, "API key budget reserve failed");
                Err(BudgetReservationError::BackendUnavailable)
            }
        }
    }

    /// The receipt is a durable idempotency fence, including after a gateway restart.
    pub(crate) async fn settle_api_key_budget(&self, id: Uuid, actual: f64) -> Result<()> {
        let actual = units(actual)?;
        let tx = self.db.begin().await?;
        let row = receipt::Entity::find_by_id(id)
            .one(&tx)
            .await?
            .ok_or_else(|| GatewayError::not_found("API key budget reservation missing"))?;
        if actual > row.reserved_amount {
            return Err(GatewayError::validation("Actual spend exceeds reservation"));
        }
        let changed = receipt::Entity::update_many()
            .col_expr(receipt::Column::ChargedAmount, Expr::value(actual))
            .filter(receipt::Column::Id.eq(id))
            .filter(receipt::Column::ChargedAmount.is_null())
            .exec(&tx)
            .await?
            .rows_affected;
        if changed == 0 {
            let latest = receipt::Entity::find_by_id(id)
                .one(&tx)
                .await?
                .ok_or_else(|| GatewayError::not_found("API key budget reservation missing"))?;
            if latest.charged_amount != Some(actual) {
                return Err(GatewayError::conflict(
                    "Reservation already settled with a different amount",
                ));
            }
        } else {
            let refund = row.reserved_amount - actual;
            let changed = budget::Entity::update_many()
                .col_expr(
                    budget::Column::UsedAmount,
                    Expr::col(budget::Column::UsedAmount).sub(refund),
                )
                .filter(budget::Column::Id.eq(row.budget_id))
                .filter(budget::Column::UsedAmount.gte(refund))
                .exec(&tx)
                .await?
                .rows_affected;
            if changed != 1 {
                return Err(GatewayError::internal(
                    "API key budget accounting invariant failed",
                ));
            }
        }
        tx.commit().await?;
        Ok(())
    }
}
