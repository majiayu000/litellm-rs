use serde_json::Value;

use crate::core::providers::anthropic::client::build_usage_from_parts;
use crate::core::types::responses::Usage;

#[derive(Debug, Default)]
pub(super) struct AnthropicUsageState {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
}

impl AnthropicUsageState {
    pub(super) fn merge(&mut self, usage: &Value) -> Usage {
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
            if let Some(value) = usage.get(field).and_then(Value::as_u64) {
                *previous = Some(value);
            }
        }
        build_usage_from_parts(
            self.input_tokens,
            self.output_tokens,
            self.cache_creation_input_tokens,
            self.cache_read_input_tokens,
        )
    }
}
