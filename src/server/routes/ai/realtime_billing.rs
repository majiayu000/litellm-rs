//! Realtime uses modality-specific cache rates absent from chat Usage.
use crate::core::budget::{BudgetReservation, UnifiedBudgetReservation};
use crate::core::pricing_service::LiteLLMModelInfo;
use crate::core::providers::ProviderError;
use crate::server::state::AppState;
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone)]
pub(super) struct Rates {
    input: [f64; 2],
    cached: [f64; 2],
    output: [f64; 2],
    pub max_input: u32,
    pub max_output: u32,
    pub model_max_output: u32,
}
impl Rates {
    pub fn load(info: &LiteLLMModelInfo, output_limit: Option<u32>) -> Result<Self, String> {
        let rate = |value: Option<f64>| {
            value
                .filter(|v| v.is_finite() && *v >= 0.0)
                .ok_or_else(|| "Realtime requires complete text/audio/cache pricing".to_string())
        };
        let extra = |key: &str| info.extra.get(key).and_then(Value::as_f64);
        let max_input = info
            .max_input_tokens
            .filter(|v| *v > 0)
            .ok_or("Missing realtime input limit")?;
        let max_output = info
            .max_output_tokens
            .filter(|v| *v > 0)
            .ok_or("Missing realtime output limit")?;
        let model_max_output = max_output;
        let max_output = output_limit.map_or(max_output, |limit| limit.min(max_output));
        if max_output == 0 {
            return Err("Realtime output limit must be positive".into());
        }
        Ok(Self {
            input: [
                rate(info.input_cost_per_token)?,
                rate(extra("input_cost_per_audio_token"))?,
            ],
            cached: [
                rate(extra("cache_read_input_token_cost"))?,
                rate(extra("cache_read_input_audio_token_cost"))?,
            ],
            output: [
                rate(info.output_cost_per_token)?,
                rate(extra("output_cost_per_audio_token"))?,
            ],
            max_input,
            max_output,
            model_max_output,
        })
    }
    pub fn wire_output_limit(&self, limit: u32) -> Result<Value, &'static str> {
        if (1..=4096).contains(&limit) {
            Ok(Value::from(limit))
        } else if limit == self.model_max_output {
            Ok(Value::from("inf"))
        } else {
            Err("Realtime output policy cannot be represented by the upstream token limit")
        }
    }
    pub fn bound(&self, max_output: u32) -> f64 {
        self.max_input as f64
            * self
                .input
                .into_iter()
                .chain(self.cached)
                .fold(0.0, f64::max)
            + max_output.min(self.model_max_output) as f64
                * self.output.into_iter().fold(0.0, f64::max)
    }
    pub fn cost(&self, usage: &Value, max_output: u32) -> Result<Option<(f64, u64)>, String> {
        // Optional usage cannot establish exact modality pricing. Known limits
        // still apply before selecting the conservative settlement fallback.
        if usage["input_tokens"]
            .as_u64()
            .is_some_and(|v| v > self.max_input as u64)
            || usage["output_tokens"]
                .as_u64()
                .is_some_and(|v| v > max_output.min(self.model_max_output) as u64)
            || usage["input_token_details"]["image_tokens"]
                .as_u64()
                .is_some_and(|v| v != 0)
        {
            return Err("Realtime usage exceeds reserved text/audio scope".into());
        }
        if usage.is_null() {
            return Ok(None);
        }
        if !usage.is_object() {
            return Err("Invalid realtime usage object".into());
        }
        let missing = |pointer| usage.pointer(pointer).is_none_or(Value::is_null);
        if [
            "/input_tokens",
            "/output_tokens",
            "/total_tokens",
            "/input_token_details/text_tokens",
            "/input_token_details/audio_tokens",
            "/input_token_details/cached_tokens",
            "/output_token_details/text_tokens",
            "/output_token_details/audio_tokens",
        ]
        .into_iter()
        .any(missing)
            || (usage["input_token_details"]["cached_tokens"]
                .as_u64()
                .is_some_and(|v| v > 0)
                && [
                    "/input_token_details/cached_tokens_details/text_tokens",
                    "/input_token_details/cached_tokens_details/audio_tokens",
                ]
                .into_iter()
                .any(missing))
        {
            return Ok(None);
        }
        let count = |v: &Value| {
            v.as_u64()
                .ok_or_else(|| "Missing or invalid realtime usage".to_string())
        };
        let input = &usage["input_token_details"];
        let output = &usage["output_token_details"];
        let cached = &input["cached_tokens_details"];
        let i = [
            count(&input["text_tokens"])?,
            count(&input["audio_tokens"])?,
        ];
        let o = [
            count(&output["text_tokens"])?,
            count(&output["audio_tokens"])?,
        ];
        let c = if count(&input["cached_tokens"])? == 0 {
            [0, 0]
        } else {
            [
                count(&cached["text_tokens"])?,
                count(&cached["audio_tokens"])?,
            ]
        };
        let input_total = count(&usage["input_tokens"])?;
        let output_total = count(&usage["output_tokens"])?;
        let total = count(&usage["total_tokens"])?;
        if i[0].checked_add(i[1]) != Some(input_total)
            || o[0].checked_add(o[1]) != Some(output_total)
            || c[0].checked_add(c[1]) != Some(count(&input["cached_tokens"])?)
            || input_total.checked_add(output_total) != Some(total)
            || c.iter().zip(i).any(|(cached, input)| *cached > input)
        {
            return Err("Realtime usage exceeds reserved text/audio scope".into());
        }
        let cost = (0..2)
            .map(|n| {
                (i[n] - c[n]) as f64 * self.input[n]
                    + c[n] as f64 * self.cached[n]
                    + o[n] as f64 * self.output[n]
            })
            .sum();
        Ok(Some((cost, total)))
    }
}

pub(super) struct Pending {
    provider: Option<UnifiedBudgetReservation>,
    key: Option<BudgetReservation>,
    bound: f64,
    pub max_output: u32,
}
impl Pending {
    pub fn reserve(
        state: &AppState,
        provider: &str,
        model: &str,
        key_budget: Option<Uuid>,
        bound: f64,
        max_output: u32,
    ) -> Result<Self, ProviderError> {
        let reservation = state
            .budget_limits
            .reserve_spend(provider, model, bound)
            .map_err(|error| {
                super::spend::reservation_error_to_provider_error(error, provider, model)
            })?;
        let key =
            super::spend::reserve_api_key_budget(&state.budget_manager, key_budget, Some(bound))?;
        Ok(Self {
            provider: Some(reservation),
            key,
            bound,
            max_output,
        })
    }
    pub async fn settle(
        mut self,
        state: &AppState,
        key_id: Option<Uuid>,
        actual: Option<(f64, u64)>,
    ) -> Result<u64, String> {
        let (cost, tokens) = actual.unwrap_or((self.bound, 0));
        let provider_error = self.provider.take().and_then(|reservation| {
            reservation
                .settle(cost)
                .err()
                .map(|_| "Realtime budget settlement failed".to_string())
        });
        let key_error = self.key.take().and_then(|reservation| {
            reservation
                .settle(cost)
                .err()
                .map(|_| "Realtime key budget settlement failed".to_string())
        });
        if let Some(key_id) = key_id
            && let Err(error) = state
                .budgeted
                .key_manager()
                .record_usage(key_id, tokens, cost)
                .await
        {
            // Attempt usage persistence even when a separate budget ledger failed.
            tracing::error!(%error, %key_id, tokens, cost, "Realtime key usage recording failed");
        }
        match (provider_error, key_error) {
            (Some(provider), Some(key)) => Err(format!("{provider}; {key}")),
            (Some(error), None) | (None, Some(error)) => Err(error),
            (None, None) => Ok(tokens),
        }
    }
}
impl Drop for Pending {
    fn drop(&mut self) {
        // An aborted transport task must not refund a potentially billable response.
        if let Some(reservation) = self.provider.take()
            && let Err(error) = reservation.settle(self.bound)
        {
            tracing::error!(
                ?error,
                "Realtime aborted response fallback settlement failed"
            );
        }
        if let Some(reservation) = self.key.take()
            && let Err(error) = reservation.settle(self.bound)
        {
            tracing::error!(
                ?error,
                "Realtime aborted response fallback settlement failed"
            );
        }
    }
}
