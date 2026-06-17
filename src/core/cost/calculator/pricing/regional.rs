use crate::core::cost::types::{CostError, ModelPricing};

pub(in crate::core::cost::calculator) fn get_minimax_pricing(
    model: &str,
) -> Result<ModelPricing, CostError> {
    use chrono::Utc;

    let normalized_model = model.to_lowercase();

    let pricing = if normalized_model.contains("m3") {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0003,
            output_cost_per_1k_tokens: 0.0012,
            cache_read_input_token_cost: Some(0.00006),
            tiered_pricing: Some(std::collections::HashMap::from([
                ("input_cost_per_token_above_512k_tokens".to_string(), 0.0006),
                (
                    "output_cost_per_token_above_512k_tokens".to_string(),
                    0.0024,
                ),
                (
                    "cache_read_input_token_cost_above_512k_tokens".to_string(),
                    0.00012,
                ),
            ])),
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("highspeed") || normalized_model.contains("m2.5-lightning")
    {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0006,
            output_cost_per_1k_tokens: 0.0024,
            cache_read_input_token_cost: Some(if normalized_model.contains("m2.7") {
                0.00006
            } else {
                0.00003
            }),
            cache_creation_input_token_cost: Some(0.000375),
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("m2.7")
        || normalized_model.contains("m2.5")
        || normalized_model.contains("m2.1")
        || normalized_model.contains("minimax-m2")
    {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0003,
            output_cost_per_1k_tokens: 0.0012,
            cache_read_input_token_cost: Some(if normalized_model.contains("m2.7") {
                0.00006
            } else {
                0.00003
            }),
            cache_creation_input_token_cost: Some(0.000375),
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

pub(in crate::core::cost::calculator) fn get_zhipu_pricing(
    model: &str,
) -> Result<ModelPricing, CostError> {
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
    } else if normalized_model.contains("glm-5.2")
        || normalized_model.contains("glm-5-2")
        || normalized_model.contains("glm-5.1")
        || normalized_model.contains("glm-5-1")
    {
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
