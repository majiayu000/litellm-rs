use std::future::Future;

use crate::core::budget::UnifiedBudgetReservation;
use crate::core::providers::ProviderError;

use super::super::budgeted::{BudgetContext, BudgetReservations, BudgetedCall};

pub(super) async fn reserve_call_settle_media_job<
    T,
    Reserve,
    Call,
    CallFuture,
    Settle,
    SettleFuture,
>(
    budgeted: BudgetedCall,
    reserve: Reserve,
    call: Call,
    settle: Settle,
) -> Result<(T, u64), ProviderError>
where
    Reserve: FnOnce(&BudgetContext) -> Result<Option<UnifiedBudgetReservation>, ProviderError>,
    Call: FnOnce() -> CallFuture,
    CallFuture: Future<Output = Result<T, ProviderError>>,
    Settle: FnOnce(BudgetReservations, BudgetContext) -> SettleFuture,
    SettleFuture: Future<Output = u64>,
{
    let (mut reservations, budget) = budgeted.reserve_for_call(reserve)?;
    match call().await {
        Ok(value) => {
            let tokens_used = settle(reservations, budget).await;
            Ok((value, tokens_used))
        }
        Err(error) if is_accepted_native_task_error(&error) => {
            settle(reservations, budget).await;
            Err(error)
        }
        Err(error) => {
            reservations.cancel();
            Err(error)
        }
    }
}

#[cfg(feature = "providers-extended")]
fn is_accepted_native_task_error(error: &ProviderError) -> bool {
    crate::core::providers::bfl::is_post_submit_error(error)
        || crate::core::providers::stability::is_post_submit_error(error)
}

#[cfg(not(feature = "providers-extended"))]
fn is_accepted_native_task_error(_error: &ProviderError) -> bool {
    false
}
