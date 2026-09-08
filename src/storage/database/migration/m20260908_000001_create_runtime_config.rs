//! One authoritative encrypted runtime configuration with a monotonic revision.
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RuntimeConfig::Table)
                    .col(
                        ColumnDef::new(RuntimeConfig::Id)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RuntimeConfig::Revision)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RuntimeConfig::EncryptedPayload)
                            .text()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .exec_stmt(
                Query::insert()
                    .into_table(RuntimeConfig::Table)
                    .columns([RuntimeConfig::Id, RuntimeConfig::Revision])
                    .values_panic([1.into(), 0.into()])
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(RuntimeConfig::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum RuntimeConfig {
    Table,
    Id,
    Revision,
    EncryptedPayload,
}
