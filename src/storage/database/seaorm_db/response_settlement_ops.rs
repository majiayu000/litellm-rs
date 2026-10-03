//! SQL fences and API-key accounting for one native Responses dispatch.
use super::types::SeaOrmDatabase;
use crate::storage::database::entities::response_settlement::{self, Column, Entity};
use crate::utils::error::gateway_error::{GatewayError, Result};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder,
    QuerySelect, TransactionTrait, sea_query::Expr,
};

impl SeaOrmDatabase {
    pub(crate) async fn insert_response_settlement(
        &self,
        row: response_settlement::Model,
    ) -> Result<()> {
        row.into_active_model().insert(&self.db).await?;
        Ok(())
    }

    pub(crate) async fn response_settlement(
        &self,
        id: &str,
    ) -> Result<Option<response_settlement::Model>> {
        Ok(Entity::find_by_id(id).one(&self.db).await?)
    }

    pub(crate) async fn bind_response_settlement(&self, id: &str, response_id: &str) -> Result<()> {
        let current = Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| GatewayError::not_found("Response settlement missing"))?;
        if current.response_id.as_deref() == Some(response_id) {
            return Ok(());
        }
        if current.response_id.is_some() || current.cost.is_some() {
            return Err(GatewayError::conflict(
                "Response settlement already bound or finalized",
            ));
        }
        let changed = Entity::update_many()
            .col_expr(Column::ResponseId, Expr::value(response_id))
            .col_expr(Column::Revision, Expr::col(Column::Revision).add(1))
            .filter(Column::Id.eq(id))
            .filter(Column::Revision.eq(current.revision))
            .exec(&self.db)
            .await?
            .rows_affected;
        if changed != 1 {
            return Err(GatewayError::conflict(
                "Response settlement changed concurrently",
            ));
        }
        Ok(())
    }

    pub(crate) async fn wake_response_settlement(&self, id: &str) -> Result<()> {
        Entity::update_many()
            .col_expr(Column::LeaseUntil, Expr::value(0_i64))
            .col_expr(Column::NextAttempt, Expr::value(0_i64))
            .filter(Column::Id.eq(id))
            .filter(Column::Complete.eq(false))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    pub(crate) async fn pending_response_settlements(
        &self,
        now: i64,
    ) -> Result<Vec<response_settlement::Model>> {
        Ok(Entity::find()
            .filter(Column::Complete.eq(false))
            .filter(Column::NextAttempt.lte(now))
            .filter(Column::LeaseUntil.lte(now))
            .order_by_asc(Column::NextAttempt)
            .limit(32)
            .all(&self.db)
            .await?)
    }

    pub(crate) async fn claim_response_settlement(
        &self,
        row: &mut response_settlement::Model,
        now: i64,
    ) -> Result<bool> {
        let changed = Entity::update_many()
            .col_expr(Column::LeaseUntil, Expr::value(now + 30))
            .col_expr(Column::Revision, Expr::col(Column::Revision).add(1))
            .filter(Column::Id.eq(&row.id))
            .filter(Column::Revision.eq(row.revision))
            .filter(Column::Complete.eq(false))
            .filter(Column::LeaseUntil.lte(now))
            .exec(&self.db)
            .await?
            .rows_affected
            == 1;
        if changed {
            row.revision += 1;
            row.lease_until = now + 30;
        }
        Ok(changed)
    }

    /// Freeze the charge before either backend sees it. Retrying cannot reprice.
    pub(crate) async fn price_response_settlement(
        &self,
        row: &mut response_settlement::Model,
        cost: f64,
        tokens: u64,
        outcome: &str,
    ) -> Result<bool> {
        let changed = Entity::update_many()
            .col_expr(Column::Cost, Expr::value(cost))
            .col_expr(
                Column::Tokens,
                Expr::value(tokens.min(i64::MAX as u64) as i64),
            )
            .col_expr(Column::Outcome, Expr::value(outcome))
            .filter(Column::Id.eq(&row.id))
            .filter(Column::Cost.is_null())
            .exec(&self.db)
            .await?
            .rows_affected
            == 1;
        if changed {
            row.cost = Some(cost);
            row.tokens = tokens.min(i64::MAX as u64) as i64;
            row.outcome = outcome.into();
        }
        Ok(changed)
    }

    /// Receipt and usage are one transaction. A crash commits both or neither.
    pub(crate) async fn settle_response_key(&self, row: &response_settlement::Model) -> Result<()> {
        let txn = self.db.begin().await?;
        let changed = Entity::update_many()
            .col_expr(Column::KeySettled, Expr::value(true))
            .filter(Column::Id.eq(&row.id))
            .filter(Column::Revision.eq(row.revision))
            .filter(Column::KeySettled.eq(false))
            .filter(Column::Cost.is_not_null())
            .exec(&txn)
            .await?
            .rows_affected;
        if changed == 1
            && let Some(id) = row.api_key_id
        {
            let actual = row.outcome == "actual";
            Self::add_api_key_usage(
                &txn,
                id,
                1,
                row.tokens.max(0) as u64,
                if actual { row.cost.unwrap_or(0.0) } else { 0.0 },
                !actual,
            )
            .await?;
        }
        txn.commit().await?;
        Ok(())
    }

    pub(crate) async fn finish_response_settlement_attempt(
        &self,
        row: &response_settlement::Model,
        complete: bool,
        now: i64,
    ) -> Result<()> {
        Entity::update_many()
            .col_expr(Column::Complete, Expr::value(complete))
            .col_expr(Column::LeaseUntil, Expr::value(0_i64))
            .col_expr(Column::NextAttempt, Expr::value(now + 10))
            .filter(Column::Id.eq(&row.id))
            .filter(Column::Revision.eq(row.revision))
            .exec(&self.db)
            .await?;
        Ok(())
    }
}
