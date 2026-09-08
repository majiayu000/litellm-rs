use sea_orm::entity::prelude::*;

/// Singleton authoritative runtime configuration. Payload is AES-GCM ciphertext.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "runtime_config")]
pub struct Model {
    /// Singleton key (1).
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    /// Monotonic compare-and-swap revision.
    pub revision: i64,
    /// None only before the first committed runtime mutation.
    pub encrypted_payload: Option<String>,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
