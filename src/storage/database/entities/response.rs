//! Stored Responses API state and its authenticated owner.
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "responses")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub owner: String,
    pub response_json: String,
    pub input_json: String,
    /// None for the chat adapter; native IDs are bound to one deployment.
    pub deployment_id: Option<String>,
    /// Digest of the upstream endpoint/account binding, never the credentials.
    pub deployment_binding: Option<String>,
    pub background: bool,
    pub status: String,
    pub expires_at: i64,
    /// Adapter worker lease; native background work runs at the upstream.
    pub lease_until: Option<i64>,
    pub revision: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
