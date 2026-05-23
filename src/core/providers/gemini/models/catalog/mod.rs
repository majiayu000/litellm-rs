mod gemini25;
mod gemini3;
mod gemini31;
mod gemini35;
mod legacy;

use super::GeminiModelRegistry;

pub(super) fn register_all(registry: &mut GeminiModelRegistry) {
    gemini35::register(registry);
    gemini31::register(registry);
    gemini3::register(registry);
    gemini25::register(registry);
    legacy::register(registry);
}
