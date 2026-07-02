use serde::{Deserialize, Serialize};

/// Usage statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Usage {
    /// Prompt token count
    pub prompt_tokens: u32,
    /// Completion token count
    pub completion_tokens: u32,
    /// Total token count
    pub total_tokens: u32,
}

/// Cost information
#[derive(Debug, Clone)]
pub struct Cost {
    /// Cost amount
    pub amount: f64,
    /// Currency type
    pub currency: String,
    /// Cost breakdown
    pub breakdown: CostBreakdown,
}

/// Cost breakdown
#[derive(Debug, Clone)]
pub struct CostBreakdown {
    /// Input cost
    pub input_cost: f64,
    /// Output cost
    pub output_cost: f64,
    /// Total cost
    pub total_cost: f64,
}
