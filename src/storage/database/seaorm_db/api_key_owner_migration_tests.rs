use super::super::{entities, migration::Migrator};
use super::types::SeaOrmDatabase;
use crate::config::models::storage::DatabaseConfig;
use crate::core::models::user::types::{User, UserStatus};
use crate::core::models::{ApiKey, Metadata, UsageStats};
use chrono::Utc;
use sea_orm::{ConnectionTrait, EntityTrait};
#[cfg(feature = "sqlite")]
use sea_orm::{DbBackend, Statement};
#[cfg(feature = "sqlite")]
use sea_orm_migration::MigratorTrait;
use std::collections::HashMap;
use std::error::Error;
use uuid::Uuid;

const PRE_OWNER_RESTRICT_MIGRATION_COUNT: u32 = 10;
type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn owner_restrict_legacy_postgres_bootstrap_uses_restrict() {
    let bootstrap = include_str!("../../../../deployment/scripts/init-db.sql");
    assert!(bootstrap.contains("user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE RESTRICT"));
}

#[cfg(feature = "sqlite")]
async fn create_pre_owner_restrict_sqlite() -> TestResult<SeaOrmDatabase> {
    let db = SeaOrmDatabase::new(&DatabaseConfig {
        enabled: false,
        ..DatabaseConfig::default()
    })
    .await?;
    Migrator::up(db.connection(), Some(PRE_OWNER_RESTRICT_MIGRATION_COUNT)).await?;
    Ok(db)
}

fn user(username: &str) -> User {
    let mut user = User::new(
        username.to_string(),
        format!("{username}@example.com"),
        "hash".to_string(),
    );
    user.status = UserStatus::Active;
    user
}

fn api_key(name: &str, user_id: Option<Uuid>) -> ApiKey {
    let now = Utc::now();
    ApiKey {
        metadata: Metadata {
            id: Uuid::new_v4(),
            created_at: now,
            updated_at: now,
            version: 1,
            extra: HashMap::from([(
                "migration_fixture".to_string(),
                serde_json::Value::Bool(true),
            )]),
        },
        name: name.to_string(),
        key_hash: format!("hash-{name}"),
        key_prefix: format!("gw-{name}"),
        user_id,
        team_id: None,
        permissions: vec!["api.chat".to_string()],
        rate_limits: None,
        expires_at: Some(now + chrono::Duration::days(30)),
        is_active: true,
        last_used_at: None,
        usage_stats: UsageStats {
            total_requests: 7,
            total_tokens: 11,
            total_cost: 1.25,
            last_reset: now,
            ..UsageStats::default()
        },
    }
}

async fn stored_key(db: &SeaOrmDatabase, key_id: Uuid) -> TestResult<entities::api_key::Model> {
    entities::ApiKey::find_by_id(key_id)
        .one(db.connection())
        .await?
        .ok_or_else(|| "migration fixture API key is missing".into())
}

async fn assert_owner_restrict_upgrade_contract(db: &SeaOrmDatabase) -> TestResult {
    let owner = user("owner-restrict");
    let no_key_user = user("owner-no-key");
    db.create_user(&owner).await?;
    db.create_user(&no_key_user).await?;

    let owned_key = api_key("owned-migration-key", Some(owner.id()));
    let global_key = api_key("global-migration-key", None);
    db.create_api_key(&owned_key).await?;
    db.create_api_key(&global_key).await?;
    let owned_before = stored_key(db, owned_key.metadata.id).await?;
    let global_before = stored_key(db, global_key.metadata.id).await?;

    db.migrate().await?;
    db.migrate().await?;
    assert_eq!(stored_key(db, owned_key.metadata.id).await?, owned_before);
    assert_eq!(stored_key(db, global_key.metadata.id).await?, global_before);

    for _ in 0..2 {
        let result = entities::User::delete_by_id(owner.id())
            .exec(db.connection())
            .await;
        assert!(
            result.is_err(),
            "owned API key must restrict owner deletion"
        );
    }
    assert!(
        entities::User::find_by_id(owner.id())
            .one(db.connection())
            .await?
            .is_some()
    );
    assert_eq!(stored_key(db, owned_key.metadata.id).await?, owned_before);

    let deleted = entities::User::delete_by_id(no_key_user.id())
        .exec(db.connection())
        .await?;
    assert_eq!(deleted.rows_affected, 1);

    let mut duplicate_hash = api_key("duplicate-hash", None);
    duplicate_hash.key_hash = global_key.key_hash;
    assert!(db.create_api_key(&duplicate_hash).await.is_err());
    assert_eq!(stored_key(db, global_key.metadata.id).await?, global_before);

    Migrator::down(db.connection(), Some(1)).await?;
    let deleted_owner = entities::User::delete_by_id(owner.id())
        .exec(db.connection())
        .await?;
    assert_eq!(deleted_owner.rows_affected, 1);
    let mut owned_after_rollback = owned_before;
    owned_after_rollback.user_id = None;
    assert_eq!(
        stored_key(db, owned_key.metadata.id).await?,
        owned_after_rollback
    );
    assert_eq!(stored_key(db, global_key.metadata.id).await?, global_before);
    Ok(())
}

#[cfg(feature = "sqlite")]
async fn sqlite_object_exists(
    db: &SeaOrmDatabase,
    object_type: &str,
    name: &str,
) -> TestResult<bool> {
    let row = db
        .connection()
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT 1 FROM sqlite_master WHERE type = ? AND name = ?",
            [object_type.into(), name.into()],
        ))
        .await?;
    Ok(row.is_some())
}

#[cfg(feature = "sqlite")]
#[tokio::test]
async fn owner_restrict_sqlite_fresh_schema_blocks_owner_delete() -> TestResult {
    let db = SeaOrmDatabase::new(&DatabaseConfig {
        enabled: false,
        ..DatabaseConfig::default()
    })
    .await?;
    db.migrate().await?;
    assert_owner_restrict_upgrade_contract(&db).await
}

#[cfg(feature = "sqlite")]
#[tokio::test]
async fn owner_restrict_sqlite_upgrade_preserves_keys_and_blocks_owner_delete() -> TestResult {
    let db = create_pre_owner_restrict_sqlite().await?;
    assert_owner_restrict_upgrade_contract(&db).await
}

#[cfg(feature = "sqlite")]
#[tokio::test]
async fn owner_restrict_sqlite_upgrade_rolls_back_on_dangling_owner() -> TestResult {
    let db = create_pre_owner_restrict_sqlite().await?;
    db.connection()
        .execute_unprepared("PRAGMA foreign_keys = OFF")
        .await?;
    let dangling_key = api_key("dangling-owner", Some(Uuid::new_v4()));
    db.create_api_key(&dangling_key).await?;
    db.connection()
        .execute_unprepared("PRAGMA foreign_keys = ON")
        .await?;
    let before = stored_key(&db, dangling_key.metadata.id).await?;

    assert!(db.migrate().await.is_err());
    assert!(!sqlite_object_exists(&db, "table", "api_keys_owner_fk_replacement").await?);
    assert!(sqlite_object_exists(&db, "index", "idx_api_keys_key_hash").await?);
    assert_eq!(stored_key(&db, dangling_key.metadata.id).await?, before);
    let pending = Migrator::get_pending_migrations(db.connection()).await?;
    assert_eq!(pending.len(), 1);
    assert_eq!(
        pending[0].name(),
        "m20260712_000001_restrict_api_key_owner_deletion"
    );
    Ok(())
}

#[cfg(feature = "sqlite")]
#[tokio::test]
async fn owner_restrict_sqlite_ledger_insert_failure_rolls_back_schema() -> TestResult {
    let db = create_pre_owner_restrict_sqlite().await?;
    let owner = user("ledger-insert-owner");
    db.create_user(&owner).await?;
    let owned_key = api_key("ledger-insert-key", Some(owner.id()));
    db.create_api_key(&owned_key).await?;
    db.connection()
        .execute_unprepared(
            "CREATE TRIGGER fail_owner_restrict_ledger_insert \
             BEFORE INSERT ON seaql_migrations \
             WHEN NEW.version = 'm20260712_000001_restrict_api_key_owner_deletion' \
             BEGIN SELECT RAISE(ABORT, 'injected ledger insert failure'); END",
        )
        .await?;

    assert!(db.migrate().await.is_err());
    let pending = Migrator::get_pending_migrations(db.connection()).await?;
    assert_eq!(pending.len(), 1);
    assert_eq!(
        pending[0].name(),
        "m20260712_000001_restrict_api_key_owner_deletion"
    );
    let deleted = entities::User::delete_by_id(owner.id())
        .exec(db.connection())
        .await?;
    assert_eq!(deleted.rows_affected, 1);
    assert_eq!(stored_key(&db, owned_key.metadata.id).await?.user_id, None);
    Ok(())
}

#[cfg(feature = "sqlite")]
#[tokio::test]
async fn owner_restrict_sqlite_ledger_delete_failure_rolls_back_schema() -> TestResult {
    let db = create_pre_owner_restrict_sqlite().await?;
    let owner = user("ledger-delete-owner");
    db.create_user(&owner).await?;
    let owned_key = api_key("ledger-delete-key", Some(owner.id()));
    db.create_api_key(&owned_key).await?;
    db.migrate().await?;
    db.connection()
        .execute_unprepared(
            "CREATE TRIGGER fail_owner_restrict_ledger_delete \
             BEFORE DELETE ON seaql_migrations \
             WHEN OLD.version = 'm20260712_000001_restrict_api_key_owner_deletion' \
             BEGIN SELECT RAISE(ABORT, 'injected ledger delete failure'); END",
        )
        .await?;

    assert!(Migrator::down(db.connection(), Some(1)).await.is_err());
    let applied = Migrator::get_applied_migrations(db.connection()).await?;
    assert_eq!(
        applied.last().map(|migration| migration.name()),
        Some("m20260712_000001_restrict_api_key_owner_deletion")
    );
    assert!(
        entities::User::delete_by_id(owner.id())
            .exec(db.connection())
            .await
            .is_err()
    );
    assert_eq!(
        stored_key(&db, owned_key.metadata.id).await?.user_id,
        Some(owner.id())
    );
    Ok(())
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn owner_restrict_postgres_upgrade_preserves_keys_and_blocks_owner_delete() -> TestResult {
    const TEST_SCHEMA: &str = "gh961_owner_restrict_test";
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(value) => value,
        Err(_) => {
            eprintln!("skipping PostgreSQL migration test because DATABASE_URL is not set");
            return Ok(());
        }
    };
    let admin = sea_orm::Database::connect(&database_url).await?;
    admin
        .execute_unprepared("DROP SCHEMA IF EXISTS gh961_owner_restrict_test CASCADE")
        .await?;
    admin
        .execute_unprepared("CREATE SCHEMA gh961_owner_restrict_test")
        .await?;

    let mut scoped_url = url::Url::parse(&database_url)?;
    scoped_url
        .query_pairs_mut()
        .append_pair("options", &format!("--search_path={TEST_SCHEMA}"));
    let db = SeaOrmDatabase::new(&DatabaseConfig {
        url: scoped_url.into(),
        max_connections: 1,
        connection_timeout: 5,
        enabled: true,
        fallback_to_sqlite: false,
        ..DatabaseConfig::default()
    })
    .await?;
    Migrator::up(db.connection(), Some(PRE_OWNER_RESTRICT_MIGRATION_COUNT)).await?;

    let contract_result = assert_owner_restrict_upgrade_contract(&db).await;
    db.close().await?;
    admin
        .execute_unprepared("DROP SCHEMA gh961_owner_restrict_test CASCADE")
        .await?;
    admin.close().await?;
    contract_result
}
