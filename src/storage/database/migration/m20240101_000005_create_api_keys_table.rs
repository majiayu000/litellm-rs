use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let mut table = create_api_keys_table(ApiKeys::Table, ForeignKeyAction::SetNull);
        table.if_not_exists();
        manager.create_table(table).await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_api_keys_key_hash")
                    .table(ApiKeys::Table)
                    .col(ApiKeys::KeyHash)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_api_keys_user_id")
                    .table(ApiKeys::Table)
                    .col(ApiKeys::UserId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_api_keys_team_id")
                    .table(ApiKeys::Table)
                    .col(ApiKeys::TeamId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_api_keys_expires_at")
                    .table(ApiKeys::Table)
                    .col(ApiKeys::ExpiresAt)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApiKeys::Table).to_owned())
            .await
    }
}

pub(super) fn create_api_keys_table<T>(
    table: T,
    owner_delete_action: ForeignKeyAction,
) -> TableCreateStatement
where
    T: IntoTableRef + Clone + 'static,
{
    let mut owner_foreign_key = api_key_owner_foreign_key(table.clone(), owner_delete_action);
    Table::create()
        .table(table)
        .col(ColumnDef::new(ApiKeys::Id).uuid().not_null().primary_key())
        .col(ColumnDef::new(ApiKeys::Name).string_len(255).not_null())
        .col(ColumnDef::new(ApiKeys::KeyHash).string_len(255).not_null())
        .col(ColumnDef::new(ApiKeys::KeyPrefix).string_len(32).not_null())
        .col(ColumnDef::new(ApiKeys::UserId).uuid().null())
        .col(ColumnDef::new(ApiKeys::TeamId).uuid().null())
        .col(ColumnDef::new(ApiKeys::Permissions).text().not_null())
        .col(ColumnDef::new(ApiKeys::RateLimits).text().null())
        .col(
            ColumnDef::new(ApiKeys::ExpiresAt)
                .timestamp_with_time_zone()
                .null(),
        )
        .col(
            ColumnDef::new(ApiKeys::IsActive)
                .boolean()
                .not_null()
                .default(true),
        )
        .col(
            ColumnDef::new(ApiKeys::LastUsedAt)
                .timestamp_with_time_zone()
                .null(),
        )
        .col(ColumnDef::new(ApiKeys::UsageStats).text().not_null())
        .col(ColumnDef::new(ApiKeys::Extra).text().null())
        .col(
            ColumnDef::new(ApiKeys::CreatedAt)
                .timestamp_with_time_zone()
                .not_null()
                .default(Expr::current_timestamp()),
        )
        .col(
            ColumnDef::new(ApiKeys::UpdatedAt)
                .timestamp_with_time_zone()
                .not_null()
                .default(Expr::current_timestamp()),
        )
        .col(
            ColumnDef::new(ApiKeys::Version)
                .integer()
                .not_null()
                .default(1),
        )
        .foreign_key(&mut owner_foreign_key)
        .to_owned()
}

pub(super) fn api_key_owner_foreign_key<T>(
    table: T,
    owner_delete_action: ForeignKeyAction,
) -> ForeignKeyCreateStatement
where
    T: IntoTableRef + 'static,
{
    ForeignKey::create()
        .name("fk_api_keys_user_id")
        .from(table, ApiKeys::UserId)
        .to(Users::Table, Users::Id)
        .on_delete(owner_delete_action)
        .to_owned()
}

#[cfg(feature = "sqlite")]
pub(super) const API_KEY_COLUMNS: [ApiKeys; 16] = [
    ApiKeys::Id,
    ApiKeys::Name,
    ApiKeys::KeyHash,
    ApiKeys::KeyPrefix,
    ApiKeys::UserId,
    ApiKeys::TeamId,
    ApiKeys::Permissions,
    ApiKeys::RateLimits,
    ApiKeys::ExpiresAt,
    ApiKeys::IsActive,
    ApiKeys::LastUsedAt,
    ApiKeys::UsageStats,
    ApiKeys::Extra,
    ApiKeys::CreatedAt,
    ApiKeys::UpdatedAt,
    ApiKeys::Version,
];

#[derive(Clone, Copy, DeriveIden)]
pub(super) enum ApiKeys {
    Table,
    Id,
    Name,
    KeyHash,
    KeyPrefix,
    UserId,
    TeamId,
    Permissions,
    RateLimits,
    ExpiresAt,
    IsActive,
    LastUsedAt,
    UsageStats,
    Extra,
    CreatedAt,
    UpdatedAt,
    Version,
}

#[derive(DeriveIden)]
pub(super) enum Users {
    Table,
    Id,
}
