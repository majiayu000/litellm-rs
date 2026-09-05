use sea_orm::entity::prelude::*;

/// Metadata-only terminal request ledger row.
///
/// This entity must never grow prompt, response body, Authorization, raw API
/// key, or provider secret columns.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "request_ledger")]
pub struct Model {
    /// Gateway request id (one terminal row per request).
    #[sea_orm(primary_key, auto_increment = false)]
    pub request_id: String,
    /// Request start timestamp.
    pub started_at: DateTimeWithTimeZone,
    /// Terminal timestamp.
    pub finished_at: DateTimeWithTimeZone,
    /// HTTP method.
    pub method: String,
    /// Request path.
    pub endpoint: String,
    /// Served model when known.
    pub model: Option<String>,
    /// Selected provider when known.
    pub provider: Option<String>,
    /// Selected deployment when known.
    pub deployment: Option<String>,
    /// HTTP status code.
    pub status_code: i32,
    /// `completed`, `failed`, or `cancelled`.
    pub terminal_status: String,
    /// End-to-end latency in milliseconds.
    pub latency_ms: i64,
    /// Prompt tokens when settled.
    pub prompt_tokens: Option<i64>,
    /// Completion tokens when settled.
    pub completion_tokens: Option<i64>,
    /// Total tokens when settled.
    pub total_tokens: Option<i64>,
    /// Settled cost when priced.
    pub cost: Option<f64>,
    /// Authenticated user id.
    pub user_id: Option<String>,
    /// API key id (never the raw secret).
    pub api_key_id: Option<String>,
    /// Team id.
    pub team_id: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
