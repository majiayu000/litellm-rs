use crate::core::models::openai::ChatCompletionRequest;
use crate::core::models::openai::responses_api::{
    ResponseInput, ResponseInputContent, ResponseInputContentPart, ResponseInputItem, ResponseTool,
    ResponsesApiRequest,
};
use crate::core::types::codex::wire::{CodexToolOutput, CodexToolOutputContent};
use crate::utils::error::gateway_error::GatewayError;

pub(super) fn payload(
    request: &ResponsesApiRequest,
    continuation: Option<&ChatCompletionRequest>,
) -> Result<String, GatewayError> {
    let mut normalized = request.clone();
    normalize_url_carriers(&mut normalized);
    let value = serde_json::to_value(normalized).map_err(|cause| {
        GatewayError::Internal(format!(
            "Responses guardrail could not project request: {cause}"
        ))
    })?;
    let mut fragments = Vec::new();
    collect_json_projection(&value, &mut fragments);
    collect_adjacent_text(request, &mut fragments);
    if let Some(continuation) = continuation {
        fragments.push(super::input_scan::payload(continuation)?);
    }
    Ok(fragments.join(super::FRAGMENT_SEPARATOR))
}

fn normalize_url_carriers(request: &mut ResponsesApiRequest) {
    if let ResponseInput::Items(items) = &mut request.input {
        for item in items {
            match item {
                ResponseInputItem::Message(message) => {
                    if let ResponseInputContent::Parts(parts) = &mut message.content {
                        for part in parts {
                            match part {
                                ResponseInputContentPart::InputImage {
                                    image_url: Some(url),
                                    ..
                                }
                                | ResponseInputContentPart::InputAudio { audio_url: url } => {
                                    normalize_url(url)
                                }
                                _ => {}
                            }
                        }
                    }
                }
                ResponseInputItem::FunctionCallOutput(output) => {
                    normalize_tool_output(&mut output.output)
                }
                ResponseInputItem::CustomToolCallOutput(output) => {
                    normalize_tool_output(&mut output.output)
                }
                _ => {}
            }
        }
    }
    for tool in request
        .tools
        .iter_mut()
        .flatten()
        .chain(request.additional_tools.iter_mut().flatten())
    {
        if let ResponseTool::Mcp(tool) = tool {
            normalize_url(&mut tool.server_url);
        }
    }
}

fn normalize_tool_output(output: &mut CodexToolOutput) {
    let CodexToolOutput::ContentItems(items) = output else {
        return;
    };
    for item in items {
        match item {
            CodexToolOutputContent::InputImage { image_url, .. } => normalize_url(image_url),
            CodexToolOutputContent::InputAudio { audio_url } => normalize_url(audio_url),
            _ => {}
        }
    }
}

fn normalize_url(url: &mut String) {
    *url = super::image_url::projected(url).into_owned();
}

fn collect_json_projection(value: &serde_json::Value, fragments: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) => {
            push(fragments, text);
            if let Ok(decoded) = serde_json::from_str(text) {
                collect_json_projection(&decoded, fragments);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_json_projection(value, fragments);
            }
        }
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                push(fragments, key);
                collect_json_projection(value, fragments);
            }
        }
        serde_json::Value::Number(number) => fragments.push(number.to_string()),
        serde_json::Value::Bool(_) | serde_json::Value::Null => {}
    }
}

fn collect_adjacent_text(request: &ResponsesApiRequest, fragments: &mut Vec<String>) {
    let ResponseInput::Items(items) = &request.input else {
        return;
    };
    for item in items {
        let ResponseInputItem::Message(message) = item else {
            continue;
        };
        let ResponseInputContent::Parts(parts) = &message.content else {
            continue;
        };
        let mut adjacent = String::new();
        for part in parts {
            match part {
                ResponseInputContentPart::InputText { text }
                | ResponseInputContentPart::OutputText { text } => {
                    if !text.is_empty() {
                        if !adjacent.is_empty() {
                            adjacent.push('\n');
                        }
                        adjacent.push_str(text);
                    }
                }
                _ => {
                    push(fragments, &adjacent);
                    adjacent.clear();
                }
            }
        }
        push(fragments, &adjacent);
    }
}

fn push(fragments: &mut Vec<String>, text: &str) {
    if !text.is_empty() {
        fragments.push(text.to_string());
    }
}
