use sea_orm::entity::prelude::*;

/// Sanitized routing-policy revision applied through the admin API.
///
/// Secret values must never be stored.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "routing_policy_revisions")]
pub struct Model {
    /// Auto-increment row id.
    #[sea_orm(primary_key)]
    pub id: i32,
    /// Runtime generation published by a successful `apply_runtime`.
    #[sea_orm(unique)]
    pub generation: i64,
    /// Acting admin user id (or `anonymous` when auth is disabled).
    pub actor: String,
    /// Sanitized routing policy JSON (no secret values).
    #[sea_orm(column_type = "Json")]
    pub sanitized_payload: Json,
    /// Revision timestamp.
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
