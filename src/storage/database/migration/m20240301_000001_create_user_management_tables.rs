//! Migration: create user management tables
//!
//! Creates the `um_users`, `um_teams`, and `um_organizations` tables used by
//! the user_management module. Each row stores a JSON snapshot of the domain
//! object so the schema stays stable while the domain evolves.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UmUsers::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UmUsers::UserId)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(UmUsers::Email)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(UmUsers::Data).text().not_null())
                    .col(
                        ColumnDef::new(UmUsers::Spend)
                            .double()
                            .not_null()
                            .default(0.0),
                    )
                    .col(
                        ColumnDef::new(UmUsers::CreatedAt)
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
                    .table(UmTeams::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UmTeams::TeamId)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(UmTeams::Data).text().not_null())
                    .col(
                        ColumnDef::new(UmTeams::Spend)
                            .double()
                            .not_null()
                            .default(0.0),
                    )
                    .col(
                        ColumnDef::new(UmTeams::CreatedAt)
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
                    .table(UmOrganizations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UmOrganizations::OrganizationId)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(UmOrganizations::Data).text().not_null())
                    .col(
                        ColumnDef::new(UmOrganizations::Spend)
                            .double()
                            .not_null()
                            .default(0.0),
                    )
                    .col(
                        ColumnDef::new(UmOrganizations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UmOrganizations::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(UmTeams::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(UmUsers::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum UmUsers {
    Table,
    UserId,
    Email,
    Data,
    Spend,
    CreatedAt,
}

#[derive(DeriveIden)]
enum UmTeams {
    Table,
    TeamId,
    Data,
    Spend,
    CreatedAt,
}

#[derive(DeriveIden)]
enum UmOrganizations {
    Table,
    OrganizationId,
    Data,
    Spend,
    CreatedAt,
}
