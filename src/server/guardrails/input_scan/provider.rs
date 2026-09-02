use crate::core::models::openai::{ChatCompletionRequest, ToolChoice};

use super::{FRAGMENT_SEPARATOR, push_fragment, push_json_value};

pub(in crate::server::guardrails) fn payload(request: &ChatCompletionRequest) -> String {
    let mut fragments = Vec::new();
    fragments.extend(request.stop.iter().flatten().cloned());
    fragments.extend(request.modalities.iter().flatten().cloned());
    fragments.extend(request.reasoning_effort.iter().cloned());
    fragments.extend(request.service_tier.iter().cloned());
    fragments.extend(
        request
            .logit_bias
            .iter()
            .flat_map(|bias| bias.keys().cloned()),
    );
    if let Some(audio) = request.audio.as_ref() {
        fragments.push(audio.voice.clone());
        fragments.push(audio.format.clone());
    }
    if let Some(format) = request.response_format.as_ref() {
        fragments.push(format.format_type.clone());
        fragments.extend(format.response_type.iter().cloned());
        if let Some(schema) = format.json_schema.as_ref() {
            push_json_value(&mut fragments, schema);
        }
    }
    for value in [
        request.prediction.as_ref(),
        request.safety_settings.as_ref(),
        request.cache_control.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        push_json_value(&mut fragments, value);
    }
    for (key, value) in &request.extra_body {
        push_fragment(&mut fragments, key);
        push_json_value(&mut fragments, value);
    }
    for message in &request.messages {
        fragments.extend(message.tool_call_id.iter().cloned());
        for call in message.tool_calls.iter().flatten() {
            fragments.push(call.id.clone());
            fragments.push(call.tool_type.clone());
        }
    }
    if let Some(choice) = request.tool_choice.as_ref() {
        match choice {
            ToolChoice::None(value) | ToolChoice::Auto(value) | ToolChoice::Required(value) => {
                fragments.push(value.clone());
            }
            ToolChoice::Specific(value) => {
                fragments.push(value.tool_type.clone());
                fragments.push(value.function.name.clone());
            }
        }
    }
    fragments.join(FRAGMENT_SEPARATOR)
}
