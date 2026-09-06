use sea_orm::DatabaseConnection;
#[cfg(feature = "sqlite")]
use sea_orm::{ConnectionTrait, DatabaseTransaction, DbBackend, TransactionTrait};
use sea_orm_migration::prelude::*;

mod m20240101_000001_create_users_table;
mod m20240101_000002_create_password_reset_tokens_table;
mod m20240101_000003_create_batches_table;
mod m20240101_000004_create_user_sessions_table;
mod m20240101_000005_create_api_keys_table;
mod m20240201_000001_create_pricing_tables;
mod m20240301_000001_create_user_management_tables;
mod m20240301_000002_create_teams_table;
mod m20240301_000003_create_virtual_keys_table;
mod m20240501_000001_create_budget_limit_snapshots;
mod m20260712_000001_restrict_api_key_owner_deletion;
mod m20260906_000001_create_request_ledger;
mod m20260906_000002_create_provider_config_revisions;
mod m20260906_000003_create_routing_policy_revisions;

/// Database migrator for SeaORM
pub struct Migrator;

impl Migrator {
    pub async fn up(db: &DatabaseConnection, steps: Option<u32>) -> Result<(), DbErr> {
        #[cfg(feature = "sqlite")]
        if db.get_database_backend() == DbBackend::Sqlite {
            let transaction = db.begin().await?;
            let result = <Self as MigratorTrait>::up(&transaction, steps).await;
            return finish_sqlite_migration(transaction, result).await;
        }
        <Self as MigratorTrait>::up(db, steps).await
    }

    pub async fn down(db: &DatabaseConnection, steps: Option<u32>) -> Result<(), DbErr> {
        #[cfg(feature = "sqlite")]
        if db.get_database_backend() == DbBackend::Sqlite {
            let transaction = db.begin().await?;
            let result = <Self as MigratorTrait>::down(&transaction, steps).await;
            return finish_sqlite_migration(transaction, result).await;
        }
        <Self as MigratorTrait>::down(db, steps).await
    }
}

#[cfg(feature = "sqlite")]
async fn finish_sqlite_migration(
    transaction: DatabaseTransaction,
    result: Result<(), DbErr>,
) -> Result<(), DbErr> {
    match result {
        Ok(()) => transaction.commit().await,
        Err(migration_error) => match transaction.rollback().await {
            Ok(()) => Err(migration_error),
            Err(rollback_error) => Err(DbErr::Custom(format!(
                "migration failed: {migration_error}; rollback failed: {rollback_error}"
            ))),
        },
    }
}

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240101_000001_create_users_table::Migration),
            Box::new(m20240101_000002_create_password_reset_tokens_table::Migration),
            Box::new(m20240101_000003_create_batches_table::Migration),
            Box::new(m20240101_000004_create_user_sessions_table::Migration),
            Box::new(m20240101_000005_create_api_keys_table::Migration),
            Box::new(m20240201_000001_create_pricing_tables::Migration),
            Box::new(m20240301_000001_create_user_management_tables::Migration),
            Box::new(m20240301_000002_create_teams_table::Migration),
            Box::new(m20240301_000003_create_virtual_keys_table::Migration),
            Box::new(m20240501_000001_create_budget_limit_snapshots::Migration),
            Box::new(m20260712_000001_restrict_api_key_owner_deletion::Migration),
            Box::new(m20260906_000001_create_request_ledger::Migration),
            Box::new(m20260906_000002_create_provider_config_revisions::Migration),
            Box::new(m20260906_000003_create_routing_policy_revisions::Migration),
        ]
    }
}
