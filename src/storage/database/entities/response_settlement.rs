//! Durable Responses billing intent; independent of response-content deletion/TTL.
use sea_orm::entity::prelude::*;
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "response_settlements")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub owner: String,
    pub api_key_id: Option<Uuid>,
    pub provider: String,
    pub model: String,
    pub deployment_id: String,
    pub deployment_binding: String,
    pub response_id: Option<String>,
    pub pricing_json: String,
    pub leases_json: String,
    pub reserved: f64,
    pub cost: Option<f64>,
    pub tokens: i64,
    /// pending, actual, or reserved_unknown (never represented as actual usage).
    pub outcome: String,
    pub key_settled: bool,
    pub complete: bool,
    pub deadline: i64,
    pub next_attempt: i64,
    pub lease_until: i64,
    pub revision: i64,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
