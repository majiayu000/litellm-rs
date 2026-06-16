use crate::core::cost::types::{CostError, ModelPricing};

pub(super) fn get_openai_pricing(model: &str) -> Result<ModelPricing, CostError> {
    use chrono::Utc;

    let pricing = match model.to_lowercase().as_str() {
        m if m.contains("gpt-5.5-pro") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.030,
            output_cost_per_1k_tokens: 0.180,
            cache_read_input_token_cost: Some(0.030),
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-5.5") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.005,
            output_cost_per_1k_tokens: 0.030,
            cache_read_input_token_cost: Some(0.0005),
            tiered_pricing: Some(std::collections::HashMap::from([
                ("input_cost_per_token_above_272k_tokens".to_string(), 0.010),
                ("output_cost_per_token_above_272k_tokens".to_string(), 0.045),
                (
                    "cache_read_input_token_cost_above_272k_tokens".to_string(),
                    0.001,
                ),
            ])),
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-5.4-pro") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.030,
            output_cost_per_1k_tokens: 0.180,
            cache_read_input_token_cost: Some(0.030),
            tiered_pricing: Some(std::collections::HashMap::from([
                ("input_cost_per_token_above_272k_tokens".to_string(), 0.060),
                ("output_cost_per_token_above_272k_tokens".to_string(), 0.270),
                (
                    "cache_read_input_token_cost_above_272k_tokens".to_string(),
                    0.060,
                ),
            ])),
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-5.4-mini") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00075,
            output_cost_per_1k_tokens: 0.0045,
            cache_read_input_token_cost: Some(0.000075),
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-5.4-nano") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0002,
            output_cost_per_1k_tokens: 0.00125,
            cache_read_input_token_cost: Some(0.00002),
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-5.4") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0025,
            output_cost_per_1k_tokens: 0.015,
            cache_read_input_token_cost: Some(0.00025),
            tiered_pricing: Some(std::collections::HashMap::from([
                ("input_cost_per_token_above_272k_tokens".to_string(), 0.005),
                (
                    "output_cost_per_token_above_272k_tokens".to_string(),
                    0.0225,
                ),
                (
                    "cache_read_input_token_cost_above_272k_tokens".to_string(),
                    0.0005,
                ),
            ])),
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-5.2-pro") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.021,
            output_cost_per_1k_tokens: 0.168,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-5.2-codex") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00175,
            output_cost_per_1k_tokens: 0.014,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-5-codex") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00125,
            output_cost_per_1k_tokens: 0.010,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-5.2") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00175,
            output_cost_per_1k_tokens: 0.014,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-5.1-thinking") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0025,
            output_cost_per_1k_tokens: 0.020,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-5.1") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00125,
            output_cost_per_1k_tokens: 0.010,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-5-mini") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00025,
            output_cost_per_1k_tokens: 0.002,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-5-nano") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00005,
            output_cost_per_1k_tokens: 0.0004,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-image-2") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.005,
            output_cost_per_1k_tokens: 0.030,
            image_cost_per_token: Some(0.000008),
            cache_read_input_token_cost: Some(0.00125),
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-image-1-mini") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.002,
            output_cost_per_1k_tokens: 0.008,
            image_cost_per_token: Some(0.0000025),
            cache_read_input_token_cost: Some(0.0002),
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-image-1.5") || m.contains("chatgpt-image-latest") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.005,
            output_cost_per_1k_tokens: 0.032,
            image_cost_per_token: Some(0.000008),
            cache_read_input_token_cost: Some(0.00125),
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-image-1") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.005,
            output_cost_per_1k_tokens: 0.020,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("computer-use-preview") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.003,
            output_cost_per_1k_tokens: 0.012,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("o3-pro") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.020,
            output_cost_per_1k_tokens: 0.080,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("o3-deep-research") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.010,
            output_cost_per_1k_tokens: 0.040,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("o3-mini") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0011,
            output_cost_per_1k_tokens: 0.0044,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("o3") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.002,
            output_cost_per_1k_tokens: 0.008,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("o4-mini-deep-research") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.002,
            output_cost_per_1k_tokens: 0.008,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("o4-mini") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0011,
            output_cost_per_1k_tokens: 0.0044,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-audio-1.5") || m == "gpt-audio" => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0025,
            output_cost_per_1k_tokens: 0.010,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-audio-mini") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0006,
            output_cost_per_1k_tokens: 0.0024,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-realtime-1.5") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.004,
            output_cost_per_1k_tokens: 0.016,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-realtime-mini") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0006,
            output_cost_per_1k_tokens: 0.0024,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-4.1") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.002,
            output_cost_per_1k_tokens: 0.008,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-4o-mini") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00015,
            output_cost_per_1k_tokens: 0.0006,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-4o") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.005,
            output_cost_per_1k_tokens: 0.015,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-4-turbo") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.01,
            output_cost_per_1k_tokens: 0.03,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-3.5-turbo") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0005,
            output_cost_per_1k_tokens: 0.0015,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        _ => {
            return Err(CostError::ModelNotSupported {
                model: model.to_string(),
                provider: "openai".to_string(),
            });
        }
    };

    Ok(pricing)
}

pub(super) fn get_anthropic_pricing(model: &str) -> Result<ModelPricing, CostError> {
    use chrono::Utc;

    let pricing = match model.to_lowercase().as_str() {
        m if m.contains("claude-opus-4-8") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.005,
            output_cost_per_1k_tokens: 0.025,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-opus-4-7") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.005,
            output_cost_per_1k_tokens: 0.025,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-opus-4-6") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.005,
            output_cost_per_1k_tokens: 0.025,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-opus-4-5") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.005,
            output_cost_per_1k_tokens: 0.025,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-opus-4-1") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.015,
            output_cost_per_1k_tokens: 0.075,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-opus-4") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.015,
            output_cost_per_1k_tokens: 0.075,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-sonnet-4-5") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.003,
            output_cost_per_1k_tokens: 0.015,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-sonnet-4") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.003,
            output_cost_per_1k_tokens: 0.015,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-3-5-sonnet") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.003,
            output_cost_per_1k_tokens: 0.015,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-3-5-haiku") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.001,
            output_cost_per_1k_tokens: 0.005,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-haiku-4-5") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.001,
            output_cost_per_1k_tokens: 0.005,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-3-opus") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.015,
            output_cost_per_1k_tokens: 0.075,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-3-sonnet") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.003,
            output_cost_per_1k_tokens: 0.015,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-3-haiku") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00025,
            output_cost_per_1k_tokens: 0.00125,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-2.1") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.008,
            output_cost_per_1k_tokens: 0.024,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-instant") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0008,
            output_cost_per_1k_tokens: 0.0024,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        _ => {
            return Err(CostError::ModelNotSupported {
                model: model.to_string(),
                provider: "anthropic".to_string(),
            });
        }
    };

    Ok(pricing)
}

pub(super) fn get_azure_pricing(model: &str) -> Result<ModelPricing, CostError> {
    // Azure pricing is typically the same as OpenAI but may have regional differences
    get_openai_pricing(model).map(|mut pricing| {
        pricing.model = model.to_string();
        pricing
    })
}

pub(super) fn get_vertex_ai_pricing(model: &str) -> Result<ModelPricing, CostError> {
    use chrono::Utc;

    let pricing = match model.to_lowercase().as_str() {
        m if m.contains("gemini-pro") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00125,
            output_cost_per_1k_tokens: 0.00375,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gemini-flash") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.000075,
            output_cost_per_1k_tokens: 0.0003,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        _ => {
            return Err(CostError::ModelNotSupported {
                model: model.to_string(),
                provider: "vertex_ai".to_string(),
            });
        }
    };

    Ok(pricing)
}

pub(super) fn get_deepseek_pricing(model: &str) -> Result<ModelPricing, CostError> {
    use chrono::Utc;

    let pricing = match model.to_lowercase().as_str() {
        m if m.contains("deepseek-v4-pro") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.000435,
            output_cost_per_1k_tokens: 0.00087,
            cache_read_input_token_cost: Some(0.000003625),
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("deepseek-v4-flash")
            || m.contains("deepseek-chat")
            || m.contains("deepseek-reasoner") =>
        {
            ModelPricing {
                model: model.to_string(),
                input_cost_per_1k_tokens: 0.00014,
                output_cost_per_1k_tokens: 0.00028,
                cache_read_input_token_cost: Some(0.0000028),
                currency: "USD".to_string(),
                updated_at: Utc::now(),
                ..Default::default()
            }
        }
        _ => {
            return Err(CostError::ModelNotSupported {
                model: model.to_string(),
                provider: "deepseek".to_string(),
            });
        }
    };

    Ok(pricing)
}

pub(super) fn get_moonshot_pricing(model: &str) -> Result<ModelPricing, CostError> {
    use chrono::Utc;

    let normalized_model = model.to_lowercase();

    let pricing = if normalized_model.contains("kimi-k2.6") {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00095,
            output_cost_per_1k_tokens: 0.004,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("kimi-k2.5") {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0006,
            output_cost_per_1k_tokens: 0.003,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("kimi-k2-thinking-turbo")
        || normalized_model.contains("kimi-k2-turbo-preview")
    {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00115,
            output_cost_per_1k_tokens: 0.008,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("kimi-k2-thinking")
        || normalized_model.contains("kimi-k2-0905-preview")
        || normalized_model.contains("kimi-k2-0711-preview")
    {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0006,
            output_cost_per_1k_tokens: 0.0025,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("moonshot-v1-8k") {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0002,
            output_cost_per_1k_tokens: 0.002,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("moonshot-v1-32k") {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.001,
            output_cost_per_1k_tokens: 0.003,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("moonshot-v1-128k") {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.002,
            output_cost_per_1k_tokens: 0.005,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else {
        return Err(CostError::ModelNotSupported {
            model: model.to_string(),
            provider: "moonshot".to_string(),
        });
    };

    Ok(pricing)
}

pub(super) fn get_minimax_pricing(model: &str) -> Result<ModelPricing, CostError> {
    use chrono::Utc;

    let normalized_model = model.to_lowercase();

    let pricing = if normalized_model.contains("m2.5-lightning") {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0003,
            output_cost_per_1k_tokens: 0.0024,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("m2.5")
        || normalized_model.contains("m2.1")
        || normalized_model.contains("minimax-m2")
    {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0003,
            output_cost_per_1k_tokens: 0.0012,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else {
        return Err(CostError::ModelNotSupported {
            model: model.to_string(),
            provider: "minimax".to_string(),
        });
    };

    Ok(pricing)
}

pub(super) fn get_zhipu_pricing(model: &str) -> Result<ModelPricing, CostError> {
    use chrono::Utc;

    let normalized_model = model.to_lowercase();

    let pricing = if normalized_model.contains("glm-5-code") {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0012,
            output_cost_per_1k_tokens: 0.005,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("glm-5.1") || normalized_model.contains("glm-5-1") {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0014,
            output_cost_per_1k_tokens: 0.0044,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("glm-5-turbo") || normalized_model.contains("glm-5v") {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0012,
            output_cost_per_1k_tokens: 0.004,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("glm-5") {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.001,
            output_cost_per_1k_tokens: 0.0032,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if (normalized_model.contains("glm-4.7-flash")
        || normalized_model.contains("glm-4.5-flash")
        || normalized_model.contains("glm-4.6v-flash"))
        && !normalized_model.contains("flashx")
    {
        // Free tier flash models
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0,
            output_cost_per_1k_tokens: 0.0,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("glm-4.7")
        || normalized_model.contains("glm-4-7")
        || normalized_model.contains("glm-4.6")
        || normalized_model.contains("glm-4.5")
    {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0006,
            output_cost_per_1k_tokens: 0.0022,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("glm-4-flash") {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00005,
            output_cost_per_1k_tokens: 0.0001,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("glm-4-plus")
        || normalized_model.contains("glm-4-air")
        || normalized_model.contains("glm-4")
    {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0001,
            output_cost_per_1k_tokens: 0.0003,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else {
        return Err(CostError::ModelNotSupported {
            model: model.to_string(),
            provider: "zhipu".to_string(),
        });
    };

    Ok(pricing)
}
