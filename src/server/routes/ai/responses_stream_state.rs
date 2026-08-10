use crate::core::models::openai::responses_api::{ResponseFunctionCall, ResponseOutputItem};
use crate::core::types::responses::Usage as ChatUsage;

pub(super) fn response_stream_total_tokens(
    final_usage: Option<&ChatUsage>,
    input_tokens: u32,
    output_tokens: u32,
) -> u32 {
    final_usage.map_or_else(
        || input_tokens.saturating_add(output_tokens),
        |usage| usage.total_tokens,
    )
}

/// Accumulated state for one in-progress tool call during streaming.
pub(super) struct ToolCallAccum {
    pub(super) item_id: String,
    pub(super) call_id: String,
    pub(super) name: String,
    pub(super) arguments: String,
    pub(super) output_index: u32,
}

impl ToolCallAccum {
    pub(super) fn new(item_id: String, call_id: String, name: String, output_index: u32) -> Self {
        Self {
            item_id,
            call_id,
            name,
            arguments: String::new(),
            output_index,
        }
    }

    pub(super) fn completed_item(&self) -> ResponseOutputItem {
        ResponseOutputItem::FunctionCall(ResponseFunctionCall {
            id: self.item_id.clone(),
            name: self.name.clone(),
            arguments: self.arguments.clone(),
            status: "completed".to_string(),
            call_id: Some(self.call_id.clone()),
        })
    }
}

pub(super) fn response_stream_budget_usage(
    final_usage: Option<ChatUsage>,
    input_tokens: u32,
    output_tokens: u32,
) -> (u32, Option<ChatUsage>) {
    let total = response_stream_total_tokens(final_usage.as_ref(), input_tokens, output_tokens);
    let usage = final_usage.or_else(|| {
        (total > 0).then_some(ChatUsage {
            prompt_tokens: input_tokens,
            completion_tokens: output_tokens,
            total_tokens: total,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            thinking_usage: None,
        })
    });
    (total, usage)
}
