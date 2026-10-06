use super::*;
use crate::auth::{AuthMethod, AuthSystem};
use crate::config::models::{auth::AuthConfig, storage::StorageConfig};
use crate::core::models::team::{Team, TeamStatus};
use crate::core::models::user::types::UserStatus;
use crate::core::teams::TeamRepository;
use crate::core::types::context::RequestContext;
use crate::storage::database::entities::user as user_entity;
use sea_orm::{ConnectionTrait, EntityTrait, Statement};

async fn fixture() -> (ApiKeyHandler, SeaOrmTeamRepository) {
    let mut config = StorageConfig::default();
    config.database.enabled = false;
    config.redis.enabled = false;
    let storage = Arc::new(StorageLayer::new(&config).await.unwrap());
    let repository = SeaOrmTeamRepository::new(storage.database.clone());
    let handler = ApiKeyHandler::new(storage, None).await.unwrap();
    (handler, repository)
}

async fn active_user(handler: &ApiKeyHandler) -> User {
    let mut user = User::new(
        format!("team-key-{}", Uuid::new_v4()),
        format!("team-key-{}@example.com", Uuid::new_v4()),
        "unused-password-hash".into(),
    );
    user.status = UserStatus::Active;
    handler.storage.db().create_user(&user).await.unwrap()
}

async fn auth_system(handler: &ApiKeyHandler) -> AuthSystem {
    let config = AuthConfig {
        jwt_secret: "AaaAaaAaaAaaAaaAaaAaaAaaAaaAaa1!".into(),
        ..Default::default()
    };
    AuthSystem::new(&config, handler.storage.clone())
        .await
        .unwrap()
}

async fn assert_rejected(handler: &ApiKeyHandler, key: &str, reason: &str) {
    assert!(handler.verify_key(key).await.unwrap().is_none());
    let detailed = handler.verify_key_detailed(key).await.unwrap();
    assert!(!detailed.is_valid);
    assert_eq!(detailed.invalid_reason.as_deref(), Some(reason));
}

#[tokio::test]
async fn deleted_team_invalidates_keys_in_existing_and_fresh_verifiers() {
    let (handler, repository) = fixture().await;
    let team = repository
        .create(Team::new("deleted-team".into(), None))
        .await
        .unwrap();
    let (api_key, raw_key) = handler
        .create_key(
            None,
            Some(team.id()),
            "team-only".into(),
            vec!["api.chat".into()],
        )
        .await
        .unwrap();
    let auth = auth_system(&handler).await;
    assert!(handler.verify_key(&raw_key).await.unwrap().is_some());
    assert!(
        handler
            .verify_key_detailed(&raw_key)
            .await
            .unwrap()
            .is_valid
    );
    let accepted = auth
        .authenticate(AuthMethod::ApiKey(raw_key.clone()), RequestContext::new())
        .await
        .unwrap();
    assert!(accepted.success);
    assert_eq!(accepted.context.team_id(), Some(team.id()));

    // Exercise the same transactional deletion path as team management. The
    // API key itself stays active: rejection must come from team lifecycle.
    repository.delete(team.id()).await.unwrap();
    assert!(repository.get(team.id()).await.unwrap().is_none());
    assert!(
        handler
            .storage
            .db()
            .find_api_key_by_hash(&api_key.key_hash)
            .await
            .unwrap()
            .unwrap()
            .is_active
    );
    assert_rejected(&handler, &raw_key, MISSING_TEAM_REASON).await;
    let fresh = ApiKeyHandler::new(handler.storage.clone(), None)
        .await
        .unwrap();
    assert_rejected(&fresh, &raw_key, MISSING_TEAM_REASON).await;
    assert!(!fresh.last_used_cache.contains_key(&api_key.metadata.id));

    for verifier in [auth, auth_system(&handler).await] {
        let rejected = verifier
            .authenticate(AuthMethod::ApiKey(raw_key.clone()), RequestContext::new())
            .await
            .unwrap();
        assert!(!rejected.success);
        assert!(rejected.context.team_id().is_none());
        assert!(rejected.api_key.is_none());
    }
}

#[tokio::test]
async fn missing_team_rejects_both_team_only_and_user_team_keys() {
    let (handler, _) = fixture().await;
    let user = active_user(&handler).await;
    for owner in [None, Some(user.id())] {
        let (api_key, raw_key) = handler
            .create_key(
                owner,
                Some(Uuid::new_v4()),
                "missing-team".into(),
                vec!["api.chat".into()],
            )
            .await
            .unwrap();
        assert_rejected(&handler, &raw_key, MISSING_TEAM_REASON).await;
        assert!(!handler.last_used_cache.contains_key(&api_key.metadata.id));
    }
}

#[tokio::test]
async fn every_nonactive_team_status_rejects_team_and_user_team_keys() {
    let (handler, repository) = fixture().await;
    let user = active_user(&handler).await;
    for status in [
        TeamStatus::Inactive,
        TeamStatus::Suspended,
        TeamStatus::Deleted,
    ] {
        let mut team = repository
            .create(Team::new(format!("status-{}", Uuid::new_v4()), None))
            .await
            .unwrap();
        let mut keys = Vec::new();
        for owner in [None, Some(user.id())] {
            let (api_key, raw_key) = handler
                .create_key(
                    owner,
                    Some(team.id()),
                    "inactive-team".into(),
                    vec!["api.chat".into()],
                )
                .await
                .unwrap();
            assert!(handler.verify_key(&raw_key).await.unwrap().is_some());
            keys.push((api_key, raw_key));
        }
        team.status = status;
        repository.update(team).await.unwrap();
        let fresh = ApiKeyHandler::new(handler.storage.clone(), None)
            .await
            .unwrap();
        for (api_key, raw_key) in keys {
            assert_rejected(&handler, &raw_key, INACTIVE_TEAM_REASON).await;
            assert_rejected(&fresh, &raw_key, INACTIVE_TEAM_REASON).await;
            assert!(!fresh.last_used_cache.contains_key(&api_key.metadata.id));
        }
    }
}

#[tokio::test]
async fn active_team_does_not_override_an_inactive_user_owner() {
    let (handler, repository) = fixture().await;
    let mut user = active_user(&handler).await;
    let team = repository
        .create(Team::new("active-team".into(), None))
        .await
        .unwrap();
    let (_, raw_key) = handler
        .create_key(
            Some(user.id()),
            Some(team.id()),
            "user-team".into(),
            vec!["api.chat".into()],
        )
        .await
        .unwrap();
    let (_, owner) = handler.verify_key(&raw_key).await.unwrap().unwrap();
    assert_eq!(owner.unwrap().id(), user.id());
    assert!(
        handler
            .verify_key_detailed(&raw_key)
            .await
            .unwrap()
            .is_valid
    );
    user.status = UserStatus::Inactive;
    user_entity::Entity::update(user_entity::Model::from_domain_user(&user))
        .exec(handler.storage.db().connection())
        .await
        .unwrap();
    assert_rejected(&handler, &raw_key, INACTIVE_OWNER_REASON).await;
}

#[tokio::test]
async fn team_lookup_failures_propagate_without_affecting_keys_without_a_team() {
    let (handler, repository) = fixture().await;
    let user = active_user(&handler).await;
    let team = repository
        .create(Team::new("backend-error".into(), None))
        .await
        .unwrap();
    let (team_key, raw_team_key) = handler
        .create_key(
            Some(user.id()),
            Some(team.id()),
            "team-key".into(),
            vec!["api.chat".into()],
        )
        .await
        .unwrap();
    let (_, raw_user_key) = handler
        .create_key(
            Some(user.id()),
            None,
            "user-only".into(),
            vec!["api.chat".into()],
        )
        .await
        .unwrap();
    let auth = auth_system(&handler).await;
    // Leave the valid key, owner, and legacy team mirror accessible. A failed
    // authoritative team query must be an infrastructure error, not a fallback.
    handler
        .storage
        .db()
        .connection()
        .execute_unprepared("DROP TABLE teams")
        .await
        .unwrap();
    assert!(handler.verify_key(&raw_team_key).await.is_err());
    assert!(handler.verify_key_detailed(&raw_team_key).await.is_err());
    assert!(!handler.last_used_cache.contains_key(&team_key.metadata.id));
    assert!(
        auth.authenticate(AuthMethod::ApiKey(raw_team_key), RequestContext::new())
            .await
            .is_err()
    );
    assert!(handler.verify_key(&raw_user_key).await.unwrap().is_some());
    assert!(
        handler
            .verify_key_detailed(&raw_user_key)
            .await
            .unwrap()
            .is_valid
    );
}

#[tokio::test]
async fn legacy_only_team_verification_does_not_create_canonical_records() {
    let (handler, repository) = fixture().await;
    let team = repository
        .create(Team::new("legacy-only-auth".into(), None))
        .await
        .unwrap();
    let db = handler.storage.db().connection();
    // Keep only the legacy mirror, as in an installation awaiting migration.
    // This fixture has exactly one team and an isolated in-memory database.
    db.execute_unprepared("DELETE FROM teams").await.unwrap();
    let (_, raw_key) = handler
        .create_key(
            None,
            Some(team.id()),
            "legacy-team-key".into(),
            vec!["api.chat".into()],
        )
        .await
        .unwrap();

    assert!(handler.verify_key(&raw_key).await.unwrap().is_some());
    assert!(
        handler
            .verify_key_detailed(&raw_key)
            .await
            .unwrap()
            .is_valid
    );
    let row = db
        .query_one(Statement::from_string(
            db.get_database_backend(),
            "SELECT COUNT(*) AS count FROM teams",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        row.try_get::<i64>("", "count").unwrap(),
        0,
        "authentication must not migrate a legacy snapshot into a new principal"
    );

    repository.delete(team.id()).await.unwrap();
    assert_rejected(&handler, &raw_key, MISSING_TEAM_REASON).await;
}

#[tokio::test]
async fn inactive_canonical_team_overrides_an_active_legacy_mirror() {
    let (handler, repository) = fixture().await;
    let mut team = repository
        .create(Team::new("canonical-auth-status".into(), None))
        .await
        .unwrap();
    let (_, raw_key) = handler
        .create_key(
            None,
            Some(team.id()),
            "canonical-team-key".into(),
            vec!["api.chat".into()],
        )
        .await
        .unwrap();
    team.status = TeamStatus::Inactive;
    repository.update(team.clone()).await.unwrap();

    let mut legacy = handler
        .storage
        .database
        .get_team(&team.id().to_string())
        .await
        .unwrap()
        .unwrap();
    legacy.is_active = true;
    handler.storage.database.update_team(&legacy).await.unwrap();

    assert_rejected(&handler, &raw_key, INACTIVE_TEAM_REASON).await;
    let fresh = ApiKeyHandler::new(handler.storage.clone(), None)
        .await
        .unwrap();
    assert_rejected(&fresh, &raw_key, INACTIVE_TEAM_REASON).await;
}
