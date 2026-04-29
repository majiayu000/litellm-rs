use super::team_repository::SeaOrmTeamRepository;
use super::types::SeaOrmDatabase;
use crate::config::models::storage::DatabaseConfig;
use crate::core::models::team::{Team, TeamStatus};
use crate::core::teams::repository::TeamRepository;
use std::sync::Arc;

async fn create_repository() -> SeaOrmTeamRepository {
    let db = Arc::new(
        SeaOrmDatabase::new(&DatabaseConfig {
            enabled: false,
            ..DatabaseConfig::default()
        })
        .await
        .expect("failed to create in-memory database"),
    );
    db.migrate().await.expect("failed to run migrations");
    SeaOrmTeamRepository::new(db)
}

#[tokio::test]
async fn test_list_and_count_exclude_deleted_teams() {
    let repo = create_repository().await;

    let active_a = Team::new("active-a".to_string(), None);
    let active_b = Team::new("active-b".to_string(), None);
    let mut deleted = Team::new("deleted-a".to_string(), None);
    deleted.status = TeamStatus::Deleted;

    repo.create(active_a).await.unwrap();
    repo.create(deleted).await.unwrap();
    repo.create(active_b).await.unwrap();

    let (teams, total) = repo.list(0, 10).await.unwrap();
    assert_eq!(total, 2);
    assert_eq!(teams.len(), 2);

    let names: Vec<String> = teams.into_iter().map(|t| t.name).collect();
    assert!(names.contains(&"active-a".to_string()));
    assert!(names.contains(&"active-b".to_string()));
    assert!(!names.contains(&"deleted-a".to_string()));

    let count = repo.count().await.unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn test_pagination_applies_after_deleted_filtering() {
    let repo = create_repository().await;

    let mut team_b_deleted = Team::new("team-b".to_string(), None);
    team_b_deleted.status = TeamStatus::Deleted;
    let mut team_e_deleted = Team::new("team-e".to_string(), None);
    team_e_deleted.status = TeamStatus::Deleted;

    repo.create(Team::new("team-a".to_string(), None))
        .await
        .unwrap();
    repo.create(team_b_deleted).await.unwrap();
    repo.create(Team::new("team-c".to_string(), None))
        .await
        .unwrap();
    repo.create(Team::new("team-d".to_string(), None))
        .await
        .unwrap();
    repo.create(team_e_deleted).await.unwrap();
    repo.create(Team::new("team-f".to_string(), None))
        .await
        .unwrap();

    let (teams, total) = repo.list(1, 2).await.unwrap();
    assert_eq!(total, 4);
    assert_eq!(teams.len(), 2);
    assert_eq!(teams[0].name, "team-c");
    assert_eq!(teams[1].name, "team-d");
}
