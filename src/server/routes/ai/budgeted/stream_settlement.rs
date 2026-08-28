use tokio::sync::{mpsc, oneshot};

use crate::core::models::openai::Usage;

use super::SettledStream;

enum StreamSettlementTerminal {
    Completion {
        usage: Option<Usage>,
        saw_upstream_output: bool,
        completed: oneshot::Sender<()>,
    },
    Disconnect {
        usage: Option<Usage>,
        completed: oneshot::Sender<()>,
    },
    Aborted {
        usage: Option<Usage>,
        saw_upstream_output: bool,
    },
}

/// Producer-side settlement handle backed by an independent actor.
///
/// The actor owns the reservations before output is polled. Dropping or aborting
/// the producer only sends a synchronous terminal notification; `Drop` never
/// launches asynchronous work that the runtime could discard.
pub(in crate::server::routes::ai) struct AbortSafeSettledStream {
    terminal: Option<mpsc::UnboundedSender<StreamSettlementTerminal>>,
    last_usage: Option<Usage>,
    saw_upstream_output: bool,
}

impl SettledStream {
    pub(in crate::server::routes::ai) fn into_abort_safe(self) -> AbortSafeSettledStream {
        let (terminal, mut commands) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut settlement = Some(self);
            let Some(command) = commands.recv().await else {
                return;
            };
            match command {
                StreamSettlementTerminal::Completion {
                    usage,
                    saw_upstream_output,
                    completed,
                } => {
                    if let Some(settlement) = settlement.take() {
                        settlement
                            .record_completion(usage.as_ref(), saw_upstream_output)
                            .await;
                    }
                    if completed.send(()).is_err() {
                        tracing::debug!("stream producer ended before completion acknowledgement");
                    }
                }
                StreamSettlementTerminal::Disconnect { usage, completed } => {
                    if let Some(settlement) = settlement.as_mut() {
                        settlement.record_disconnect(usage.as_ref()).await;
                    }
                    if completed.send(()).is_err() {
                        tracing::debug!("stream producer ended before disconnect acknowledgement");
                    }
                }
                StreamSettlementTerminal::Aborted {
                    usage,
                    saw_upstream_output,
                } => {
                    if (usage.is_some() || saw_upstream_output)
                        && let Some(settlement) = settlement.as_mut()
                    {
                        settlement.record_disconnect(usage.as_ref()).await;
                    }
                }
            }
        });
        AbortSafeSettledStream {
            terminal: Some(terminal),
            last_usage: None,
            saw_upstream_output: false,
        }
    }
}

impl AbortSafeSettledStream {
    pub(in crate::server::routes::ai) fn observe(
        &mut self,
        usage: Option<&Usage>,
        saw_upstream_output: bool,
    ) {
        if let Some(usage) = usage {
            self.last_usage = Some(usage.clone());
        }
        self.saw_upstream_output |= saw_upstream_output;
    }

    pub(in crate::server::routes::ai) async fn record_completion(
        mut self,
        usage: Option<&Usage>,
        saw_upstream_output: bool,
    ) {
        self.observe(usage, saw_upstream_output);
        let (completed, wait_for_completion) = oneshot::channel();
        let Some(terminal) = self.terminal.take() else {
            return;
        };
        if terminal
            .send(StreamSettlementTerminal::Completion {
                usage: self.last_usage.clone(),
                saw_upstream_output: self.saw_upstream_output,
                completed,
            })
            .is_err()
        {
            tracing::error!("stream settlement actor ended before completion");
            return;
        }
        if wait_for_completion.await.is_err() {
            tracing::error!("stream settlement actor dropped completion acknowledgement");
        }
    }

    pub(in crate::server::routes::ai) async fn record_disconnect(&mut self, usage: Option<&Usage>) {
        self.observe(usage, false);
        let (completed, wait_for_completion) = oneshot::channel();
        let Some(terminal) = self.terminal.take() else {
            return;
        };
        if terminal
            .send(StreamSettlementTerminal::Disconnect {
                usage: self.last_usage.clone(),
                completed,
            })
            .is_err()
        {
            tracing::error!("stream settlement actor ended before disconnect");
            return;
        }
        if wait_for_completion.await.is_err() {
            tracing::error!("stream settlement actor dropped disconnect acknowledgement");
        }
    }
}

impl Drop for AbortSafeSettledStream {
    fn drop(&mut self) {
        let Some(terminal) = self.terminal.take() else {
            return;
        };
        if terminal
            .send(StreamSettlementTerminal::Aborted {
                usage: self.last_usage.clone(),
                saw_upstream_output: self.saw_upstream_output,
            })
            .is_err()
        {
            tracing::error!("stream settlement actor ended before producer abort notification");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::future::pending;
    use std::time::Duration;

    use crate::core::budget::UnifiedBudgetLimits;
    use crate::core::models::openai::Usage;
    use crate::core::pricing_service::PricingUsage;

    use super::super::tests::{limited_budget, settled_stream};

    fn usage(prompt_tokens: u32, completion_tokens: u32) -> Usage {
        Usage {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            thinking_usage: None,
        }
    }

    async fn wait_for_provider_spend(limits: &UnifiedBudgetLimits, expected: f64) {
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let actual = limits
                    .providers
                    .get_provider_usage("openai")
                    .map(|usage| usage.current_spend)
                    .unwrap_or_default();
                if (actual - expected).abs() < f64::EPSILON {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("settlement actor should finish within the test deadline");
    }

    #[tokio::test]
    async fn abort_after_output_settles_reserved_cost() {
        let limits = limited_budget();
        let reservation = limits
            .reserve_spend("openai", "gpt-4", 0.25)
            .expect("reservation should fit test budget");
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let handle = tokio::spawn({
            let limits = limits.clone();
            async move {
                let mut settlement = settled_stream(limits, Some(reservation)).into_abort_safe();
                settlement.observe(None, true);
                ready_tx.send(()).expect("test receiver should be waiting");
                pending::<()>().await;
            }
        });

        ready_rx.await.expect("producer should reach output state");
        handle.abort();
        assert!(
            handle
                .await
                .expect_err("producer should be aborted")
                .is_cancelled()
        );
        wait_for_provider_spend(limits.as_ref(), 0.25).await;
    }

    #[tokio::test]
    async fn abort_after_known_usage_settles_actual_cost() {
        let limits = limited_budget();
        let reservation = limits
            .reserve_spend("openai", "gpt-4", 0.25)
            .expect("reservation should fit test budget");
        let known_usage = usage(1, 1);
        let expected = settled_stream(limits.clone(), None)
            .request_pricing
            .calculate_settlement(&PricingUsage::new(1, 1))
            .expect("gpt-4 usage should be priced");
        let expected = expected.total_cost;
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let handle = tokio::spawn({
            let limits = limits.clone();
            async move {
                let mut settlement = settled_stream(limits, Some(reservation)).into_abort_safe();
                settlement.observe(Some(&known_usage), true);
                ready_tx.send(()).expect("test receiver should be waiting");
                pending::<()>().await;
            }
        });

        ready_rx.await.expect("producer should observe final usage");
        handle.abort();
        assert!(
            handle
                .await
                .expect_err("producer should be aborted")
                .is_cancelled()
        );
        wait_for_provider_spend(limits.as_ref(), expected).await;
    }

    #[tokio::test]
    async fn explicit_disconnect_then_drop_settles_only_once() {
        let limits = limited_budget();
        let reservation = limits
            .reserve_spend("openai", "gpt-4", 0.25)
            .expect("reservation should fit test budget");
        let known_usage = usage(1, 1);
        let expected = settled_stream(limits.clone(), None)
            .request_pricing
            .calculate_settlement(&PricingUsage::new(1, 1))
            .expect("gpt-4 usage should be priced");
        let expected = expected.total_cost;
        let mut settlement = settled_stream(limits.clone(), Some(reservation)).into_abort_safe();

        settlement.record_disconnect(Some(&known_usage)).await;
        drop(settlement);

        wait_for_provider_spend(limits.as_ref(), expected).await;
    }

    #[tokio::test]
    async fn abort_before_output_releases_reservation() {
        let limits = limited_budget();
        let reservation = limits
            .reserve_spend("openai", "gpt-4", 0.25)
            .expect("reservation should fit test budget");
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let handle = tokio::spawn({
            let limits = limits.clone();
            async move {
                let _settlement = settled_stream(limits, Some(reservation)).into_abort_safe();
                ready_tx.send(()).expect("test receiver should be waiting");
                pending::<()>().await;
            }
        });

        ready_rx.await.expect("producer should hold reservation");
        handle.abort();
        assert!(
            handle
                .await
                .expect_err("producer should be aborted")
                .is_cancelled()
        );
        wait_for_provider_spend(limits.as_ref(), 0.0).await;
    }
}
