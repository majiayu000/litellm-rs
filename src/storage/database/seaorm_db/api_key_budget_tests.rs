use super::SeaOrmDatabase;
use super::api_key_budget_ops::write_budget;
use crate::config::models::storage::DatabaseConfig;
use crate::core::budget::BudgetReservationError;
use crate::core::budget::{BudgetManager, BudgetReservation};
use crate::core::keys::{CreateKeyConfig, DatabaseKeyRepository, KeyManager, UpdateKeyConfig};
use crate::storage::StorageLayer;
use std::sync::Arc;
use uuid::Uuid;

async fn database(url: &str) -> Arc<SeaOrmDatabase> {
    let db = SeaOrmDatabase::new(&DatabaseConfig {
        enabled: true,
        url: url.into(),
        max_connections: 1,
        fallback_to_sqlite: false,
        ..Default::default()
    })
    .await
    .unwrap();
    Arc::new(db)
}

#[tokio::test]
async fn persistent_key_budget_shared_admission_restart_and_idempotent_late_settlement() {
    let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        directory.path().join("budget.db").display()
    );
    let a = database(&url).await;
    a.migrate().await.unwrap();
    let b = database(&url).await;
    let id = Uuid::new_v4();
    let tx = a.db.begin().await.unwrap();
    write_budget(&tx, id, 1.0).await.unwrap();
    tx.commit().await.unwrap();
    let mut work = tokio::task::JoinSet::new();
    for n in 0..32 {
        let db = if n % 2 == 0 { a.clone() } else { b.clone() };
        work.spawn(async move { db.reserve_api_key_budget(id, 0.1).await });
    }
    let mut receipts = Vec::new();
    while let Some(result) = work.join_next().await {
        match result.unwrap() {
            Ok(receipt) => receipts.push(receipt),
            Err(BudgetReservationError::BudgetExceeded) => (),
            Err(error) => panic!("Unexpected admission error: {error:?}"),
        }
    }
    assert_eq!(
        receipts.len(),
        10,
        "two independent connections share one cap"
    );
    assert_eq!(
        b.api_key_budget(id).await.unwrap().unwrap().used_amount,
        1_000_000_000
    );
    drop(a);
    drop(b);
    let restarted = database(&url).await;
    assert_eq!(
        restarted
            .api_key_budget(id)
            .await
            .unwrap()
            .unwrap()
            .used_amount,
        1_000_000_000
    );
    assert!(matches!(
        restarted.reserve_api_key_budget(id, 0.1).await,
        Err(BudgetReservationError::BudgetExceeded)
    ));
    restarted
        .settle_api_key_budget(receipts[0], 0.025)
        .await
        .unwrap();
    restarted
        .settle_api_key_budget(receipts[0], 0.025)
        .await
        .unwrap();
    assert!(
        restarted
            .settle_api_key_budget(receipts[0], 0.0)
            .await
            .is_err()
    );
    assert_eq!(
        restarted
            .api_key_budget(id)
            .await
            .unwrap()
            .unwrap()
            .used_amount,
        925_000_000
    );
    // Aborting a request cannot turn its durable hold into free capacity.
    let receipt = restarted.reserve_api_key_budget(id, 0.05).await.unwrap();
    let manager = BudgetManager::new();
    drop(
        BudgetReservation::persistent(
            restarted.clone(),
            receipt,
            id,
            0.05,
            manager.tracker().clone(),
        )
        .unwrap(),
    );
    assert_eq!(
        restarted
            .api_key_budget(id)
            .await
            .unwrap()
            .unwrap()
            .used_amount,
        975_000_000
    );
    // An invalid final amount leaves the original reservation recoverable.
    assert!(
        restarted
            .settle_api_key_budget(receipt, 0.06)
            .await
            .is_err()
    );
    restarted.settle_api_key_budget(receipt, 0.0).await.unwrap();
    assert_eq!(
        restarted
            .api_key_budget(id)
            .await
            .unwrap()
            .unwrap()
            .used_amount,
        925_000_000
    );
    // A failed SQL settlement rolls back its receipt fence and can be retried.
    restarted.db.execute_unprepared("CREATE TRIGGER fail_refund BEFORE UPDATE ON api_key_budgets BEGIN SELECT RAISE(ABORT, 'injected failure'); END").await.unwrap();
    assert!(
        restarted
            .settle_api_key_budget(receipts[1], 0.0)
            .await
            .is_err()
    );
    restarted
        .db
        .execute_unprepared("DROP TRIGGER fail_refund")
        .await
        .unwrap();
    restarted
        .settle_api_key_budget(receipts[1], 0.0)
        .await
        .unwrap();
    assert_eq!(
        restarted
            .api_key_budget(id)
            .await
            .unwrap()
            .unwrap()
            .used_amount,
        825_000_000
    );
    // Missing schema/backend errors must never become an unlimited budget.
    restarted
        .db
        .execute_unprepared("DROP TABLE api_key_budget_reservations")
        .await
        .unwrap();
    assert!(matches!(
        restarted.reserve_api_key_budget(id, 0.01).await,
        Err(BudgetReservationError::BackendUnavailable)
    ));
    assert_eq!(
        restarted
            .api_key_budget(id)
            .await
            .unwrap()
            .unwrap()
            .used_amount,
        825_000_000
    );
}

#[tokio::test]
async fn persistent_key_budget_key_mutations_are_atomic_and_cap_changes_preserve_spend() {
    let mut config = crate::config::Config::default();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    let storage = Arc::new(StorageLayer::new(&config.gateway.storage).await.unwrap());
    storage.migrate().await.unwrap();
    let keys = KeyManager::new(DatabaseKeyRepository::new(storage.clone()));
    let (key, _) = keys
        .generate_key(CreateKeyConfig {
            name: "budgeted".into(),
            max_budget: Some(1.0),
            ..Default::default()
        })
        .await
        .unwrap();
    let id = keys.get_key(key).await.unwrap().unwrap().budget_id.unwrap();
    let receipt = storage
        .database
        .reserve_api_key_budget(id, 0.4)
        .await
        .unwrap();
    storage
        .database
        .settle_api_key_budget(receipt, 0.3)
        .await
        .unwrap();
    keys.update_key(
        key,
        UpdateKeyConfig {
            max_budget: Some(0.2),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let budget = storage.database.api_key_budget(id).await.unwrap().unwrap();
    assert_eq!(
        (budget.max_amount, budget.used_amount),
        (200_000_000, 300_000_000)
    );
    assert!(matches!(
        storage.database.reserve_api_key_budget(id, 0.01).await,
        Err(BudgetReservationError::BudgetExceeded)
    ));
    assert!(
        keys.update_key(
            key,
            UpdateKeyConfig {
                name: Some("must roll back".into()),
                max_budget: Some(-1.0),
                ..Default::default()
            }
        )
        .await
        .is_err()
    );
    assert_eq!(keys.get_key(key).await.unwrap().unwrap().name, "budgeted");
    keys.update_key(
        key,
        UpdateKeyConfig {
            max_budget: Some(2.0),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        storage
            .database
            .api_key_budget(id)
            .await
            .unwrap()
            .unwrap()
            .used_amount,
        300_000_000
    );
    let (rotated, _) = keys.rotate_key(key).await.unwrap();
    assert_eq!(
        keys.get_key(rotated).await.unwrap().unwrap().budget_id,
        Some(id)
    );
    let count = keys.count_keys(None).await.unwrap();
    assert!(
        keys.generate_key(CreateKeyConfig {
            name: "invalid".into(),
            max_budget: Some(f64::NAN),
            ..Default::default()
        })
        .await
        .is_err()
    );
    assert_eq!(keys.count_keys(None).await.unwrap(), count);
}

use sea_orm::{ConnectionTrait, TransactionTrait};
