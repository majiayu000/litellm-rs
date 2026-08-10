use crate::core::models::openai::responses_api::{
    ResponseFunctionCall, ResponseOutputItem, ResponseStreamEvent,
};
use crate::core::types::codex::wire::CodexCustomToolCall;
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
    pub(super) custom: bool,
}

impl ToolCallAccum {
    pub(super) fn new(
        item_id: String,
        call_id: String,
        name: String,
        output_index: u32,
        custom: bool,
    ) -> Self {
        Self {
            item_id,
            call_id,
            name,
            arguments: String::new(),
            output_index,
            custom,
        }
    }

    pub(super) fn output_item(&self, status: &str) -> ResponseOutputItem {
        if self.custom {
            return ResponseOutputItem::CustomToolCall(CodexCustomToolCall {
                id: Some(self.item_id.clone()),
                call_id: self.call_id.clone(),
                name: self.name.clone(),
                namespace: None,
                input: super::custom_tool_input(&self.arguments),
                status: Some(status.to_string()),
                internal_chat_message_metadata_passthrough: None,
            });
        }
        ResponseOutputItem::FunctionCall(ResponseFunctionCall {
            id: self.item_id.clone(),
            name: self.name.clone(),
            arguments: if status == "in_progress" {
                String::new()
            } else {
                self.arguments.clone()
            },
            status: status.to_string(),
            call_id: Some(self.call_id.clone()),
        })
    }

    pub(super) fn delta_event(&self, delta: String) -> Option<ResponseStreamEvent> {
        (!self.custom).then(|| ResponseStreamEvent::ResponseFunctionCallArgumentsDelta {
            output_index: self.output_index,
            item_id: self.item_id.clone(),
            delta,
        })
    }

    pub(super) fn done_events(&self) -> Vec<ResponseStreamEvent> {
        if self.custom {
            let input = super::custom_tool_input(&self.arguments);
            return vec![
                ResponseStreamEvent::ResponseCustomToolCallInputDelta {
                    output_index: self.output_index,
                    item_id: self.item_id.clone(),
                    delta: input.clone(),
                },
                ResponseStreamEvent::ResponseCustomToolCallInputDone {
                    output_index: self.output_index,
                    item_id: self.item_id.clone(),
                    input,
                },
            ];
        }
        vec![ResponseStreamEvent::ResponseFunctionCallArgumentsDone {
            output_index: self.output_index,
            item_id: self.item_id.clone(),
            name: self.name.clone(),
            arguments: self.arguments.clone(),
        }]
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
