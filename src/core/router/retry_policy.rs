//! Contextual retry policy for router execution.

use super::config::RouterConfig;
use super::deployment::DeploymentConfig;
use super::execution::{calculate_retry_delay, calculate_retry_delay_for_schedule};
use crate::core::providers::ProviderError;
use crate::core::providers::failure::{ProviderFailureFacts, ProviderFailureKind};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryOperation {
    Unary,
    Streaming,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamRetryStage {
    NotStreaming,
    BeforeFirstChunk,
    AfterChunksEmitted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestIdempotency {
    Idempotent,
    NonIdempotent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryContext {
    pub operation: RetryOperation,
    pub stream_stage: StreamRetryStage,
    pub idempotency: RequestIdempotency,
    pub attempt: u32,
    pub max_attempts: u32,
    pub retry_budget_remaining: u32,
    pub deadline_remaining: Option<Duration>,
}

impl RetryContext {
    pub fn unary(attempt: u32, max_attempts: u32) -> Self {
        Self::new(
            RetryOperation::Unary,
            StreamRetryStage::NotStreaming,
            RequestIdempotency::Idempotent,
            attempt,
            max_attempts,
        )
    }

    pub fn stream_pre_output(attempt: u32, max_attempts: u32) -> Self {
        Self::new(
            RetryOperation::Streaming,
            StreamRetryStage::BeforeFirstChunk,
            RequestIdempotency::Idempotent,
            attempt,
            max_attempts,
        )
    }

    pub fn stream_after_chunks(attempt: u32, max_attempts: u32) -> Self {
        Self::new(
            RetryOperation::Streaming,
            StreamRetryStage::AfterChunksEmitted,
            RequestIdempotency::NonIdempotent,
            attempt,
            max_attempts,
        )
    }

    pub fn with_deadline_remaining(mut self, deadline_remaining: Duration) -> Self {
        self.deadline_remaining = Some(deadline_remaining);
        self
    }

    fn new(
        operation: RetryOperation,
        stream_stage: StreamRetryStage,
        idempotency: RequestIdempotency,
        attempt: u32,
        max_attempts: u32,
    ) -> Self {
        Self {
            operation,
            stream_stage,
            idempotency,
            attempt,
            max_attempts,
            retry_budget_remaining: max_attempts.saturating_sub(attempt),
            deadline_remaining: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryDecisionReason {
    RetryableFailure,
    AttemptsExhausted,
    RetryBudgetExhausted,
    NonIdempotentRequest,
    StreamAlreadyEmitted,
    DeadlineWouldExpire,
    NonRetryableFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryDecision {
    pub should_retry: bool,
    pub delay: Option<Duration>,
    pub reason: RetryDecisionReason,
}

impl RetryDecision {
    fn retry(delay: Duration) -> Self {
        Self {
            should_retry: true,
            delay: Some(delay),
            reason: RetryDecisionReason::RetryableFailure,
        }
    }

    fn stop(reason: RetryDecisionReason) -> Self {
        Self {
            should_retry: false,
            delay: None,
            reason,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RetryPolicy;

impl RetryPolicy {
    pub fn decide(
        &self,
        config: &RouterConfig,
        error: &ProviderError,
        context: RetryContext,
    ) -> RetryDecision {
        self.decide_with_fallback_delay(error, context, || {
            calculate_retry_delay(config, context.attempt)
        })
    }

    pub fn decide_for_deployment(
        &self,
        router_config: &RouterConfig,
        deployment_config: &DeploymentConfig,
        error: &ProviderError,
        context: RetryContext,
    ) -> RetryDecision {
        self.decide_with_fallback_delay(error, context, || {
            deployment_config
                .retry_schedule
                .as_ref()
                .map(|schedule| calculate_retry_delay_for_schedule(schedule, context.attempt))
                .unwrap_or_else(|| calculate_retry_delay(router_config, context.attempt))
        })
    }

    fn decide_with_fallback_delay<F>(
        &self,
        error: &ProviderError,
        context: RetryContext,
        fallback_delay: F,
    ) -> RetryDecision
    where
        F: FnOnce() -> Duration,
    {
        let facts = ProviderFailureFacts::from_error(error);

        if context.attempt >= context.max_attempts {
            return RetryDecision::stop(RetryDecisionReason::AttemptsExhausted);
        }

        if context.retry_budget_remaining == 0 {
            return RetryDecision::stop(RetryDecisionReason::RetryBudgetExhausted);
        }

        if context.stream_stage == StreamRetryStage::AfterChunksEmitted {
            return RetryDecision::stop(RetryDecisionReason::StreamAlreadyEmitted);
        }

        if context.idempotency == RequestIdempotency::NonIdempotent {
            return RetryDecision::stop(RetryDecisionReason::NonIdempotentRequest);
        }

        if !failure_is_retryable(facts, context) {
            return RetryDecision::stop(RetryDecisionReason::NonRetryableFailure);
        }

        let delay = facts.retry_hint.retry_after.unwrap_or_else(fallback_delay);

        if let Some(deadline_remaining) = context.deadline_remaining
            && delay > deadline_remaining
        {
            return RetryDecision::stop(RetryDecisionReason::DeadlineWouldExpire);
        }

        RetryDecision::retry(delay)
    }
}

fn failure_is_retryable(facts: ProviderFailureFacts, context: RetryContext) -> bool {
    match facts.kind {
        ProviderFailureKind::RateLimit
        | ProviderFailureKind::Timeout
        | ProviderFailureKind::ProviderUnavailable
        | ProviderFailureKind::Network
        | ProviderFailureKind::DeploymentError => true,
        ProviderFailureKind::ApiError => facts
            .upstream_status
            .is_some_and(|status| status == 429 || (500..=599).contains(&status)),
        ProviderFailureKind::Streaming => {
            context.operation == RetryOperation::Streaming
                && context.stream_stage == StreamRetryStage::BeforeFirstChunk
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::bedrock::BedrockErrorMapper;
    use crate::core::router::deployment::RetrySchedule;

    fn deployment_config_with_schedule() -> DeploymentConfig {
        DeploymentConfig {
            retry_schedule: Some(RetrySchedule {
                base_delay_ms: 250,
                max_delay_ms: 900,
                backoff_multiplier: 2.0,
                jitter_ratio: 0.0,
            }),
            ..DeploymentConfig::default()
        }
    }

    #[test]
    fn deployment_schedule_controls_retry_delay() {
        let error = ProviderError::rate_limit("openai", None);
        let deployment = deployment_config_with_schedule();

        let first = RetryPolicy.decide_for_deployment(
            &RouterConfig::default(),
            &deployment,
            &error,
            RetryContext::unary(1, 4),
        );
        let second = RetryPolicy.decide_for_deployment(
            &RouterConfig::default(),
            &deployment,
            &error,
            RetryContext::unary(2, 4),
        );
        let capped = RetryPolicy.decide_for_deployment(
            &RouterConfig::default(),
            &deployment,
            &error,
            RetryContext::unary(3, 4),
        );

        assert_eq!(first.delay, Some(Duration::from_millis(250)));
        assert_eq!(second.delay, Some(Duration::from_millis(500)));
        assert_eq!(capped.delay, Some(Duration::from_millis(900)));
    }

    #[test]
    fn deployment_without_schedule_uses_router_delay() {
        let router = RouterConfig {
            retry_after_secs: 5,
            ..RouterConfig::default()
        };
        let error = ProviderError::rate_limit("openai", None);

        let decision = RetryPolicy.decide_for_deployment(
            &router,
            &DeploymentConfig::default(),
            &error,
            RetryContext::unary(1, 2),
        );

        assert_eq!(decision.delay, Some(Duration::from_secs(5)));
    }

    #[test]
    fn retry_after_hint_overrides_deployment_schedule() {
        let error = ProviderError::rate_limit("openai", Some(60));

        let decision = RetryPolicy.decide_for_deployment(
            &RouterConfig::default(),
            &deployment_config_with_schedule(),
            &error,
            RetryContext::unary(1, 2),
        );

        assert_eq!(decision.delay, Some(Duration::from_secs(60)));
    }

    #[test]
    fn streaming_error_after_emitted_chunks_is_not_retried() {
        let error = ProviderError::streaming_error("openai", "chat", Some(1), None, "broken");
        let decision = RetryPolicy.decide(
            &RouterConfig::default(),
            &error,
            RetryContext::stream_after_chunks(1, 2),
        );

        assert!(!decision.should_retry);
        assert_eq!(decision.reason, RetryDecisionReason::StreamAlreadyEmitted);
    }

    #[test]
    fn pre_output_streaming_error_may_retry_when_budget_permits() {
        let error = ProviderError::streaming_error("openai", "chat", None, None, "connect reset");
        let decision = RetryPolicy.decide(
            &RouterConfig::default(),
            &error,
            RetryContext::stream_pre_output(1, 2),
        );

        assert!(decision.should_retry);
        assert_eq!(decision.delay, Some(Duration::from_secs(1)));
    }

    #[test]
    fn retry_after_hint_controls_rate_limit_delay() {
        let config = RouterConfig {
            retry_after_secs: 5,
            ..Default::default()
        };
        let error = ProviderError::rate_limit("openai", Some(60));
        let decision = RetryPolicy.decide(&config, &error, RetryContext::unary(1, 2));

        assert!(decision.should_retry);
        assert_eq!(decision.delay, Some(Duration::from_secs(60)));
    }

    #[test]
    fn deadline_blocks_retry_delay_that_cannot_fit() {
        let error = ProviderError::rate_limit("openai", Some(60));
        let context = RetryContext::unary(1, 2).with_deadline_remaining(Duration::from_secs(10));
        let decision = RetryPolicy.decide(&RouterConfig::default(), &error, context);

        assert!(!decision.should_retry);
        assert_eq!(decision.reason, RetryDecisionReason::DeadlineWouldExpire);
    }

    #[test]
    fn upstream_5xx_api_error_is_retryable_fact_for_policy() {
        let error = ProviderError::api_error("anthropic", 503, "overloaded");
        let decision =
            RetryPolicy.decide(&RouterConfig::default(), &error, RetryContext::unary(1, 2));

        assert!(decision.should_retry);
    }

    #[test]
    fn failed_dependency_api_error_is_retryable_but_not_not_found() {
        let not_ready =
            BedrockErrorMapper::map_service_error("ModelNotReadyException", "model not ready")
                .expect("modeled Bedrock service error");
        let model_error = ProviderError::api_error(
            "bedrock",
            424,
            "ModelNotReadyException: misleading ordinary HTTP message",
        );
        let missing = ProviderError::api_error("bedrock", 404, "resource not found");
        let unrelated = ProviderError::api_error("custom_httpx", 424, "failed dependency");

        let retry = RetryPolicy.decide(
            &RouterConfig::default(),
            &not_ready,
            RetryContext::unary(1, 2),
        );
        let stop = RetryPolicy.decide(
            &RouterConfig::default(),
            &missing,
            RetryContext::unary(1, 2),
        );
        let model_error_stop = RetryPolicy.decide(
            &RouterConfig::default(),
            &model_error,
            RetryContext::unary(1, 2),
        );
        let unrelated_stop = RetryPolicy.decide(
            &RouterConfig::default(),
            &unrelated,
            RetryContext::unary(1, 2),
        );

        assert!(retry.should_retry);
        assert!(!model_error_stop.should_retry);
        assert_eq!(
            model_error_stop.reason,
            RetryDecisionReason::NonRetryableFailure
        );
        assert!(!stop.should_retry);
        assert_eq!(stop.reason, RetryDecisionReason::NonRetryableFailure);
        assert!(!unrelated_stop.should_retry);
        assert_eq!(
            unrelated_stop.reason,
            RetryDecisionReason::NonRetryableFailure
        );
    }

    #[test]
    fn deployment_error_remains_retryable_for_provider_parity() {
        let error = ProviderError::deployment_error("azure", "deployment not ready");
        let decision =
            RetryPolicy.decide(&RouterConfig::default(), &error, RetryContext::unary(1, 2));

        assert!(decision.should_retry);
    }
}
