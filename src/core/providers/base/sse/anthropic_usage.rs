use serde_json::Value;

use crate::core::providers::anthropic::client::build_usage_from_parts;
use crate::core::types::responses::Usage;

#[derive(Debug, Default)]
pub(super) struct AnthropicUsageState {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
    invalid: bool,
}

impl AnthropicUsageState {
    pub(super) fn merge(&mut self, usage: &Value) {
        if !usage.is_object() {
            self.invalid = true;
            return;
        }
        for (field, previous) in [
            ("input_tokens", &mut self.input_tokens),
            ("output_tokens", &mut self.output_tokens),
            (
                "cache_creation_input_tokens",
                &mut self.cache_creation_input_tokens,
            ),
            ("cache_read_input_tokens", &mut self.cache_read_input_tokens),
        ] {
            // Deltas are cumulative. Omission keeps the preceding value;
            // explicit zero replaces it rather than being treated as absent.
            if let Some(value) = usage.get(field) {
                if let Some(value) = value.as_u64().filter(|value| u32::try_from(*value).is_ok()) {
                    *previous = Some(value);
                } else {
                    self.invalid = true;
                    *previous = None;
                }
            }
        }
    }

    pub(super) fn terminal_usage(&self, terminal: &Value) -> Option<Usage> {
        // A start/intermediate output counter is not a terminal total. Missing
        // or malformed counters must retain admission, not become real zero.
        if self.invalid
            || terminal
                .get("output_tokens")
                .and_then(Value::as_u64)
                .is_none()
        {
            return None;
        }
        let total = self
            .input_tokens?
            .checked_add(self.cache_creation_input_tokens.unwrap_or(0))?
            .checked_add(self.cache_read_input_tokens.unwrap_or(0))?
            .checked_add(self.output_tokens?)?;
        u32::try_from(total).ok()?;
        Some(build_usage_from_parts(
            self.input_tokens,
            self.output_tokens,
            self.cache_creation_input_tokens,
            self.cache_read_input_tokens,
        ))
    }
}
