//! OpenAI Fine-tuning Module
//!
//! Fine-tuning job management following the unified architecture

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::providers::unified_provider::ProviderError;

/// OpenAI Fine-tuning Job creation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIFineTuningRequest {
    /// The ID of an uploaded file that contains training data
    pub training_file: String,

    /// The ID of an uploaded file that contains validation data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_file: Option<String>,

    /// The name of the model to fine-tune
    pub model: String,

    /// The hyperparameters used for the fine-tuning job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hyperparameters: Option<FineTuningHyperparameters>,

    /// A string of up to 18 characters for the suffix
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,

    /// Set of key-value pairs for metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,

    /// List of integrations to enable for this fine-tuning job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integrations: Option<Vec<Integration>>,

    /// Seed for deterministic training
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i32>,
}

/// Hyperparameters for fine-tuning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FineTuningHyperparameters {
    /// The number of epochs to train the model for
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n_epochs: Option<u32>,

    /// Batch size to use for training
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_size: Option<u32>,

    /// Learning rate multiplier to use for training
    #[serde(skip_serializing_if = "Option::is_none")]
    pub learning_rate_multiplier: Option<f64>,
}

/// Integration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Integration {
    /// The type of integration
    #[serde(rename = "type")]
    pub integration_type: String,

    /// Configuration for Weights & Biases integration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wandb: Option<WandBConfig>,
}

/// Weights & Biases configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WandBConfig {
    /// The name of the project
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,

    /// Display name for the run
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Entity (team) to use for the run
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity: Option<String>,

    /// Tags for the run
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

/// OpenAI Fine-tuning Job response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIFineTuningJob {
    /// The object identifier
    pub id: String,

    /// The object type (always "fine_tuning.job")
    pub object: String,

    /// The Unix timestamp for when the fine-tuning job was created
    pub created_at: i64,

    /// The Unix timestamp for when the fine-tuning job was finished
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<i64>,

    /// The base model that is being fine-tuned
    pub model: String,

    /// The fine-tuned model name (available after completion)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fine_tuned_model: Option<String>,

    /// The organization that owns the fine-tuning job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization_id: Option<String>,

    /// The current status of the fine-tuning job
    pub status: FineTuningStatus,

    /// The hyperparameters used for the fine-tuning job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hyperparameters: Option<FineTuningHyperparameters>,

    /// The file ID used for training
    pub training_file: String,

    /// The file ID used for validation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_file: Option<String>,

    /// The compiled results file ID(s) for the fine-tuning job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_files: Option<Vec<String>>,

    /// The total number of billable tokens processed by this fine-tuning job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trained_tokens: Option<u64>,

    /// The suffix used to identify the fine-tuned model
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,

    /// Error information if the job failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<FineTuningError>,

    /// Estimated finish time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_finish: Option<i64>,

    /// List of integrations enabled for this job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integrations: Option<Vec<Integration>>,

    /// Seed used for training
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i32>,
}

/// Fine-tuning job status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FineTuningStatus {
    ValidatingFiles,
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

/// Fine-tuning error details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FineTuningError {
    /// Error code
    pub code: String,

    /// Error message
    pub message: String,

    /// Additional error parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
}

/// Fine-tuning job events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FineTuningEvent {
    pub id: String,
    pub object: String,
    pub created_at: i64,
    pub level: EventLevel,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(rename = "type")]
    pub event_type: EventType,
}

/// Event level
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventLevel {
    Info,
    Warn,
    Error,
}

/// Event type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Message,
    Metrics,
}

/// Fine-tuning checkpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FineTuningCheckpoint {
    pub id: String,
    pub object: String,
    pub created_at: i64,
    pub fine_tuned_model_checkpoint: String,
    pub fine_tuning_job_id: String,
    pub metrics: CheckpointMetrics,
    pub step_number: u32,
}

/// Checkpoint metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointMetrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub train_loss: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub train_mean_token_accuracy: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_loss: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_mean_token_accuracy: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_valid_loss: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_valid_mean_token_accuracy: Option<f64>,
}

/// Fine-tuning utilities
pub struct OpenAIFineTuningUtils;

impl OpenAIFineTuningUtils {
    /// Get supported fine-tuning models
    pub fn get_supported_models() -> Vec<&'static str> {
        vec![
            "gpt-3.5-turbo",
            "gpt-3.5-turbo-1106",
            "gpt-3.5-turbo-0613",
            "gpt-4o-mini-2024-07-18",
            "gpt-4-0613",
            "babbage-002",
            "davinci-002",
        ]
    }

    /// Check if model supports fine-tuning
    pub fn supports_fine_tuning(model_id: &str) -> bool {
        Self::get_supported_models().contains(&model_id)
    }

    /// Create fine-tuning job request
    pub fn create_job_request(
        training_file: String,
        model: String,
        suffix: Option<String>,
        hyperparameters: Option<FineTuningHyperparameters>,
    ) -> OpenAIFineTuningRequest {
        OpenAIFineTuningRequest {
            training_file,
            validation_file: None,
            model,
            hyperparameters,
            suffix,
            metadata: None,
            integrations: None,
            seed: None,
        }
    }

    /// Create job with Weights & Biases integration
    pub fn create_job_with_wandb(
        training_file: String,
        model: String,
        project: String,
        tags: Vec<String>,
    ) -> OpenAIFineTuningRequest {
        let wandb_integration = Integration {
            integration_type: "wandb".to_string(),
            wandb: Some(WandBConfig {
                project: Some(project),
                name: None,
                entity: None,
                tags: Some(tags),
            }),
        };

        OpenAIFineTuningRequest {
            training_file,
            validation_file: None,
            model,
            hyperparameters: None,
            suffix: None,
            metadata: None,
            integrations: Some(vec![wandb_integration]),
            seed: None,
        }
    }

    /// Validate fine-tuning request
    pub fn validate_request(request: &OpenAIFineTuningRequest) -> Result<(), ProviderError> {
        // Check if model supports fine-tuning
        if !Self::supports_fine_tuning(&request.model) {
            return Err(ProviderError::ModelNotFound {
                provider: "openai",
                model: request.model.clone(),
            });
        }

        // Check suffix length
        if let Some(suffix) = &request.suffix {
            if suffix.len() > 40 {
                return Err(ProviderError::InvalidRequest {
                    provider: "openai",
                    message: "Suffix must be 40 characters or less".to_string(),
                });
            }

            // Check suffix contains only alphanumeric, hyphens, and underscores
            if !suffix
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
            {
                return Err(ProviderError::InvalidRequest {
                    provider: "openai",
                    message:
                        "Suffix can only contain alphanumeric characters, hyphens, and underscores"
                            .to_string(),
                });
            }
        }

        // Validate hyperparameters
        if let Some(hyperparams) = &request.hyperparameters {
            if let Some(n_epochs) = hyperparams.n_epochs
                && (n_epochs == 0 || n_epochs > 50)
            {
                return Err(ProviderError::InvalidRequest {
                    provider: "openai",
                    message: "n_epochs must be between 1 and 50".to_string(),
                });
            }

            if let Some(batch_size) = hyperparams.batch_size
                && ![1, 2, 4, 8, 16, 32].contains(&batch_size)
            {
                return Err(ProviderError::InvalidRequest {
                    provider: "openai",
                    message: "batch_size must be one of: 1, 2, 4, 8, 16, 32".to_string(),
                });
            }

            if let Some(lr_multiplier) = hyperparams.learning_rate_multiplier
                && (lr_multiplier <= 0.0 || lr_multiplier > 10.0)
            {
                return Err(ProviderError::InvalidRequest {
                    provider: "openai",
                    message: "learning_rate_multiplier must be between 0 and 10".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Estimate cost for fine-tuning
    pub fn estimate_cost(model: &str, num_tokens: u64) -> Result<f64, ProviderError> {
        let cost_per_1k_tokens = match model {
            "gpt-3.5-turbo" | "gpt-3.5-turbo-1106" | "gpt-3.5-turbo-0613" => 0.008,
            "gpt-4o-mini-2024-07-18" => 0.0003,
            "gpt-4-0613" => 0.03,
            "babbage-002" => 0.0004,
            "davinci-002" => 0.006,
            _ => {
                return Err(ProviderError::InvalidRequest {
                    provider: "openai",
                    message: format!("Unknown fine-tuning model: {}", model),
                });
            }
        };

        Ok((num_tokens as f64 / 1000.0) * cost_per_1k_tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supports_fine_tuning() {
        assert!(OpenAIFineTuningUtils::supports_fine_tuning("gpt-3.5-turbo"));
        assert!(OpenAIFineTuningUtils::supports_fine_tuning("babbage-002"));
        assert!(!OpenAIFineTuningUtils::supports_fine_tuning("gpt-4"));
        assert!(!OpenAIFineTuningUtils::supports_fine_tuning("dall-e-3"));
    }

    #[test]
    fn test_create_job_request() {
        let request = OpenAIFineTuningUtils::create_job_request(
            "file-123".to_string(),
            "gpt-3.5-turbo".to_string(),
            Some("my-model".to_string()),
            None,
        );

        assert_eq!(request.training_file, "file-123");
        assert_eq!(request.model, "gpt-3.5-turbo");
        assert_eq!(request.suffix, Some("my-model".to_string()));
    }

    #[test]
    fn test_validate_request() {
        let valid_request = OpenAIFineTuningUtils::create_job_request(
            "file-123".to_string(),
            "gpt-3.5-turbo".to_string(),
            Some("valid_suffix".to_string()),
            None,
        );
        assert!(OpenAIFineTuningUtils::validate_request(&valid_request).is_ok());

        // Test invalid model
        let invalid_model = OpenAIFineTuningRequest {
            training_file: "file-123".to_string(),
            validation_file: None,
            model: "gpt-4".to_string(),
            hyperparameters: None,
            suffix: None,
            metadata: None,
            integrations: None,
            seed: None,
        };
        assert!(OpenAIFineTuningUtils::validate_request(&invalid_model).is_err());

        // Test invalid suffix
        let mut invalid_suffix = valid_request.clone();
        invalid_suffix.suffix = Some("a".repeat(50)); // Too long
        assert!(OpenAIFineTuningUtils::validate_request(&invalid_suffix).is_err());
    }

    #[test]
    fn test_estimate_cost() {
        let cost = OpenAIFineTuningUtils::estimate_cost("gpt-3.5-turbo", 10000).unwrap();
        assert_eq!(cost, 0.08); // 10k tokens * $0.008/1k tokens

        let cost = OpenAIFineTuningUtils::estimate_cost("babbage-002", 5000).unwrap();
        assert_eq!(cost, 0.002); // 5k tokens * $0.0004/1k tokens

        assert!(OpenAIFineTuningUtils::estimate_cost("unknown-model", 1000).is_err());
    }
}
