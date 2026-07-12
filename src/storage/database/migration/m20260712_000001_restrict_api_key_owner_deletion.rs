#[cfg(any(feature = "postgres", feature = "sqlite"))]
use super::m20240101_000005_create_api_keys_table::ApiKeys;
#[cfg(feature = "postgres")]
use super::m20240101_000005_create_api_keys_table::api_key_owner_foreign_key;
#[cfg(feature = "sqlite")]
use super::m20240101_000005_create_api_keys_table::{API_KEY_COLUMNS, create_api_keys_table};
#[cfg(any(feature = "postgres", feature = "sqlite"))]
use sea_orm::DbBackend;
#[cfg(feature = "sqlite")]
use sea_orm::TransactionTrait;
#[cfg(feature = "sqlite")]
use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

#[cfg(feature = "postgres")]
const OWNER_FOREIGN_KEY: &str = "fk_api_keys_user_id";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        replace_owner_foreign_key(manager, ForeignKeyAction::Restrict).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        replace_owner_foreign_key(manager, ForeignKeyAction::SetNull).await
    }
}

async fn replace_owner_foreign_key(
    manager: &SchemaManager<'_>,
    _action: ForeignKeyAction,
) -> Result<(), DbErr> {
    match manager.get_database_backend() {
        #[cfg(feature = "postgres")]
        DbBackend::Postgres => replace_postgres_owner_foreign_key(manager, _action).await,
        #[cfg(feature = "sqlite")]
        DbBackend::Sqlite => rebuild_sqlite_api_keys(manager, _action).await,
        #[allow(unreachable_patterns)]
        backend => Err(DbErr::Custom(format!(
            "unsupported database backend for API-key owner migration: {backend:?}"
        ))),
    }
}

#[cfg(feature = "postgres")]
async fn replace_postgres_owner_foreign_key(
    manager: &SchemaManager<'_>,
    action: ForeignKeyAction,
) -> Result<(), DbErr> {
    manager
        .drop_foreign_key(
            ForeignKey::drop()
                .name(OWNER_FOREIGN_KEY)
                .table(ApiKeys::Table)
                .to_owned(),
        )
        .await?;
    manager
        .create_foreign_key(api_key_owner_foreign_key(ApiKeys::Table, action))
        .await
}

#[cfg(feature = "sqlite")]
async fn rebuild_sqlite_api_keys(
    manager: &SchemaManager<'_>,
    action: ForeignKeyAction,
) -> Result<(), DbErr> {
    let transaction = manager.get_connection().begin().await?;
    let transaction_manager = SchemaManager::new(&transaction);
    let result = rebuild_sqlite_api_keys_in_transaction(&transaction_manager, action).await;

    match result {
        Ok(()) => transaction.commit().await,
        Err(migration_error) => match transaction.rollback().await {
            Ok(()) => Err(migration_error),
            Err(rollback_error) => Err(DbErr::Custom(format!(
                "API-key owner migration failed: {migration_error}; rollback failed: {rollback_error}"
            ))),
        },
    }
}

#[cfg(feature = "sqlite")]
async fn rebuild_sqlite_api_keys_in_transaction(
    manager: &SchemaManager<'_>,
    action: ForeignKeyAction,
) -> Result<(), DbErr> {
    manager
        .create_table(create_api_keys_table(ApiKeysReplacement::Table, action))
        .await?;

    let select = Query::select()
        .columns(API_KEY_COLUMNS)
        .from(ApiKeys::Table)
        .to_owned();
    let insert = Query::insert()
        .into_table(ApiKeysReplacement::Table)
        .columns(API_KEY_COLUMNS)
        .select_from(select)
        .map_err(|error| DbErr::Custom(error.to_string()))?
        .to_owned();
    manager.exec_stmt(insert).await?;

    manager
        .drop_table(Table::drop().table(ApiKeys::Table).to_owned())
        .await?;
    manager
        .rename_table(
            Table::rename()
                .table(ApiKeysReplacement::Table, ApiKeys::Table)
                .to_owned(),
        )
        .await?;
    create_api_key_indexes(manager).await?;

    let violations = manager
        .get_connection()
        .query_all(Statement::from_string(
            DbBackend::Sqlite,
            "PRAGMA foreign_key_check(\"api_keys\")".to_string(),
        ))
        .await?;
    if violations.is_empty() {
        Ok(())
    } else {
        Err(DbErr::Custom(format!(
            "API-key owner migration produced {} foreign-key violation(s)",
            violations.len()
        )))
    }
}

#[cfg(feature = "sqlite")]
async fn create_api_key_indexes(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    for (name, column, unique) in [
        ("idx_api_keys_key_hash", ApiKeys::KeyHash, true),
        ("idx_api_keys_user_id", ApiKeys::UserId, false),
        ("idx_api_keys_team_id", ApiKeys::TeamId, false),
        ("idx_api_keys_expires_at", ApiKeys::ExpiresAt, false),
    ] {
        let mut index = Index::create();
        index
            .if_not_exists()
            .name(name)
            .table(ApiKeys::Table)
            .col(column);
        if unique {
            index.unique();
        }
        manager.create_index(index.to_owned()).await?;
    }
    Ok(())
}

#[cfg(feature = "sqlite")]
#[derive(Clone, Copy, DeriveIden)]
enum ApiKeysReplacement {
    #[sea_orm(iden = "api_keys_owner_fk_replacement")]
    Table,
}

#[cfg(all(test, feature = "postgres"))]
mod tests {
    use super::*;

    #[test]
    fn owner_restrict_postgres_ddl_uses_named_constraint() {
        let drop_sql = ForeignKey::drop()
            .name(OWNER_FOREIGN_KEY)
            .table(ApiKeys::Table)
            .to_owned()
            .to_string(PostgresQueryBuilder);
        let create_sql = api_key_owner_foreign_key(ApiKeys::Table, ForeignKeyAction::Restrict)
            .to_string(PostgresQueryBuilder);

        assert!(drop_sql.contains("DROP CONSTRAINT \"fk_api_keys_user_id\""));
        assert!(create_sql.contains("CONSTRAINT \"fk_api_keys_user_id\""));
        assert!(create_sql.contains("ON DELETE RESTRICT"));
    }
}
