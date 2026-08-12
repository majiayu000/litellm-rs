mod gemini25;
mod gemini3;
mod gemini31;
mod gemini35;
mod gemini36;
mod legacy;

use super::GeminiModelRegistry;
use crate::core::types::model::ProviderCapability;

pub(super) fn advanced_text_capabilities() -> Vec<ProviderCapability> {
    vec![
        ProviderCapability::ChatCompletion,
        ProviderCapability::ChatCompletionStream,
        ProviderCapability::ToolCalling,
        ProviderCapability::FunctionCalling,
        ProviderCapability::CodeExecution,
        ProviderCapability::BatchProcessing,
    ]
}

pub(super) fn function_batch_capabilities() -> Vec<ProviderCapability> {
    vec![
        ProviderCapability::ChatCompletion,
        ProviderCapability::ChatCompletionStream,
        ProviderCapability::ToolCalling,
        ProviderCapability::FunctionCalling,
        ProviderCapability::BatchProcessing,
    ]
}

pub(super) fn register_all(registry: &mut GeminiModelRegistry) {
    gemini36::register(registry);
    gemini35::register(registry);
    gemini31::register(registry);
    gemini3::register(registry);
    gemini25::register(registry);
    legacy::register(registry);
}
