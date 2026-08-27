//! Current OpenAI model entries added after the legacy static catalog.

use super::{OpenAIModelFamily, StaticModelEntry};

pub(super) fn entries() -> Vec<StaticModelEntry> {
    vec![
        (
            "gpt-5.6",
            "GPT-5.6",
            OpenAIModelFamily::GPT56Sol,
            1_050_000,
            Some(128_000),
            0.004,
            0.020,
        ),
        (
            "gpt-5.6-sol",
            "GPT-5.6 Sol",
            OpenAIModelFamily::GPT56Sol,
            1_050_000,
            Some(128_000),
            0.004,
            0.020,
        ),
        (
            "gpt-5.6-terra",
            "GPT-5.6 Terra",
            OpenAIModelFamily::GPT56Terra,
            1_050_000,
            Some(128_000),
            0.002,
            0.012,
        ),
        (
            "gpt-5.6-luna",
            "GPT-5.6 Luna",
            OpenAIModelFamily::GPT56Luna,
            1_050_000,
            Some(128_000),
            0.0002,
            0.0012,
        ),
        (
            "gpt-realtime-2",
            "GPT Realtime 2",
            OpenAIModelFamily::Realtime,
            128_000,
            Some(32_000),
            0.004,
            0.024,
        ),
        (
            "gpt-realtime-2.1",
            "GPT Realtime 2.1",
            OpenAIModelFamily::Realtime,
            128_000,
            Some(32_000),
            0.004,
            0.024,
        ),
        (
            "gpt-realtime-2.1-mini",
            "GPT Realtime 2.1 Mini",
            OpenAIModelFamily::Realtime,
            128_000,
            Some(32_000),
            0.0006,
            0.0024,
        ),
    ]
}
