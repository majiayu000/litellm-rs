use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Responses::Table)
                    .col(
                        ColumnDef::new(Responses::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Responses::Owner).string().not_null())
                    .col(ColumnDef::new(Responses::ResponseJson).text().not_null())
                    .col(ColumnDef::new(Responses::InputJson).text().not_null())
                    .col(ColumnDef::new(Responses::DeploymentId).string().null())
                    .col(ColumnDef::new(Responses::DeploymentBinding).string().null())
                    .col(ColumnDef::new(Responses::Background).boolean().not_null())
                    .col(ColumnDef::new(Responses::Status).string().not_null())
                    .col(
                        ColumnDef::new(Responses::ExpiresAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Responses::LeaseUntil).big_integer().null())
                    .col(ColumnDef::new(Responses::Revision).big_integer().not_null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_responses_expires_at")
                    .table(Responses::Table)
                    .col(Responses::ExpiresAt)
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Responses::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Responses {
    Table,
    Id,
    Owner,
    ResponseJson,
    InputJson,
    DeploymentId,
    DeploymentBinding,
    Background,
    Status,
    ExpiresAt,
    LeaseUntil,
    Revision,
}
