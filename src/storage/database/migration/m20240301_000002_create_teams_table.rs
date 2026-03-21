//! Migration: create gateway teams and team_members tables
//!
//! Backs the `TeamRepository` implementation so `TeamManager` state is
//! persisted across restarts. Team and member data is stored as a JSON
//! snapshot alongside the indexed columns required for efficient lookups.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Teams::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Teams::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Teams::Name).string().not_null().unique_key())
                    .col(ColumnDef::new(Teams::Data).text().not_null())
                    .col(
                        ColumnDef::new(Teams::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(TeamMembers::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(TeamMembers::TeamId).string().not_null())
                    .col(ColumnDef::new(TeamMembers::UserId).string().not_null())
                    .col(ColumnDef::new(TeamMembers::Data).text().not_null())
                    .col(
                        ColumnDef::new(TeamMembers::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(TeamMembers::TeamId)
                            .col(TeamMembers::UserId),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TeamMembers::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Teams::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Teams {
    Table,
    Id,
    Name,
    Data,
    CreatedAt,
}

#[derive(DeriveIden)]
enum TeamMembers {
    Table,
    TeamId,
    UserId,
    Data,
    CreatedAt,
}
