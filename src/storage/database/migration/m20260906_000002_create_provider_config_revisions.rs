//! Migration: create sanitized provider configuration revision log.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ProviderConfigRevisions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProviderConfigRevisions::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ProviderConfigRevisions::Generation)
                            .big_integer()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(ProviderConfigRevisions::Actor)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProviderConfigRevisions::SanitizedPayload)
                            .json()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProviderConfigRevisions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_provider_config_revisions_created_at")
                    .table(ProviderConfigRevisions::Table)
                    .col(ProviderConfigRevisions::CreatedAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ProviderConfigRevisions::Table)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum ProviderConfigRevisions {
    Table,
    Id,
    Generation,
    Actor,
    SanitizedPayload,
    CreatedAt,
}
