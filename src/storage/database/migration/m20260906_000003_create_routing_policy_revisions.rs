//! Migration: create sanitized routing policy revision log.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RoutingPolicyRevisions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RoutingPolicyRevisions::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RoutingPolicyRevisions::Generation)
                            .big_integer()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(RoutingPolicyRevisions::Actor)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RoutingPolicyRevisions::SanitizedPayload)
                            .json()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RoutingPolicyRevisions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_routing_policy_revisions_created_at")
                    .table(RoutingPolicyRevisions::Table)
                    .col(RoutingPolicyRevisions::CreatedAt)
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
                    .table(RoutingPolicyRevisions::Table)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum RoutingPolicyRevisions {
    Table,
    Id,
    Generation,
    Actor,
    SanitizedPayload,
    CreatedAt,
}
