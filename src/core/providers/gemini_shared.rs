pub const GEMINI_10_PRO_CONTEXT_WINDOW: u32 = 32_000;
pub const GEMINI_15_FLASH_CONTEXT_WINDOW: u32 = 1_048_576;
pub const GEMINI_15_PRO_CONTEXT_WINDOW: u32 = 2_097_152;
pub const GEMINI_20_FLASH_CONTEXT_WINDOW: u32 = 1_000_000;
pub const GEMINI_20_FLASH_THINKING_CONTEXT_WINDOW: u32 = 32_000;
pub const GEMINI_25_CONTEXT_WINDOW: u32 = 1_048_576;
pub const GEMINI_30_CONTEXT_WINDOW: u32 = 1_000_000;
pub const GEMINI_30_IMAGE_CONTEXT_WINDOW: u32 = 65_536;
pub const GEMINI_31_CONTEXT_WINDOW: u32 = 1_048_576;

pub fn gemini_context_window(model_name: &str) -> Option<u32> {
    let model_lower = model_name.to_ascii_lowercase();
    let is_exact_gemini_37 = model_name == "gemini-3.7-flash"
        || model_name.split_once('/').is_some_and(|(provider, model)| {
            model == "gemini-3.7-flash"
                && ["gemini", "google", "vertex_ai"]
                    .iter()
                    .any(|approved| provider.eq_ignore_ascii_case(approved))
        });

    if is_exact_gemini_37
        || model_lower.contains("gemini-3.6-flash")
        || model_lower.contains("gemini-3.5-flash-lite")
        || model_lower.contains("gemini-3.5-flash")
        || model_lower.contains("gemini-3.1-flash-lite")
        || model_lower.contains("gemini-3.1-flash")
        || model_lower.contains("gemini-3.1-pro")
    {
        Some(GEMINI_31_CONTEXT_WINDOW)
    } else if model_lower.contains("gemini-3") && model_lower.contains("deep-think") {
        Some(GEMINI_30_CONTEXT_WINDOW)
    } else if model_lower.contains("gemini-3") && model_lower.contains("image") {
        Some(GEMINI_30_IMAGE_CONTEXT_WINDOW)
    } else if model_lower.contains("gemini-3-flash") || model_lower.contains("gemini-3.0-flash") {
        Some(GEMINI_31_CONTEXT_WINDOW)
    } else if model_lower.contains("gemini-3-pro") || model_lower.contains("gemini-3.0-pro") {
        Some(GEMINI_30_CONTEXT_WINDOW)
    } else if model_lower.contains("gemini-2.5-flash-lite")
        || model_lower.contains("gemini-2.5-flash")
        || model_lower.contains("gemini-2.5-pro")
    {
        Some(GEMINI_25_CONTEXT_WINDOW)
    } else if model_lower.contains("gemini-2.0-flash-thinking") {
        Some(GEMINI_20_FLASH_THINKING_CONTEXT_WINDOW)
    } else if model_lower.contains("gemini-2.0-flash") || model_lower.contains("gemini-2-flash") {
        Some(GEMINI_20_FLASH_CONTEXT_WINDOW)
    } else if model_lower.contains("gemini-1.5-pro") || model_lower.contains("gemini-15-pro") {
        Some(GEMINI_15_PRO_CONTEXT_WINDOW)
    } else if model_lower.contains("gemini-1.5-flash-8b")
        || model_lower.contains("gemini-1.5-flash")
        || model_lower.contains("gemini-15-flash")
    {
        Some(GEMINI_15_FLASH_CONTEXT_WINDOW)
    } else if model_lower.contains("gemini-1.0-pro")
        || model_lower.contains("gemini-pro")
        || model_lower.contains("gemini-1.0-pro-vision")
    {
        Some(GEMINI_10_PRO_CONTEXT_WINDOW)
    } else {
        None
    }
}
