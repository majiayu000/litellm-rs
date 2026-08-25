fn vector_test_config(
    db_type: &str,
    url: String,
    allow_degraded: bool,
) -> crate::config::models::file_storage::VectorDbConfig {
    crate::config::models::file_storage::VectorDbConfig {
        db_type: db_type.to_string(),
        url,
        api_key: "test".to_string(),
        index_name: "test".to_string(),
        allow_degraded,
    }
}

async fn failing_qdrant_config(
    allow_degraded: bool,
) -> (
    crate::config::models::file_storage::VectorDbConfig,
    tokio::task::JoinHandle<()>,
) {
    use tokio::io::AsyncWriteExt;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock Qdrant listener should bind");
    let address = listener
        .local_addr()
        .expect("mock Qdrant listener should have an address");
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener
            .accept()
            .await
            .expect("mock Qdrant listener should accept one request");
        socket
            .write_all(
                b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .await
            .expect("mock Qdrant response should be written");
    });
    (
        vector_test_config("qdrant", format!("http://{address}"), allow_degraded),
        task,
    )
}

fn storage_with_vector(
    vector_db: Option<crate::config::models::file_storage::VectorDbConfig>,
) -> StorageConfig {
    StorageConfig {
        database: sqlite_db_config(),
        redis: RedisConfig::default(),
        files: FileStorageConfig::default(),
        vector_db,
    }
}

#[tokio::test]
async fn vector_db_runtime_failure_without_allow_degraded_fails_startup() {
    let (vector, server_task) = failing_qdrant_config(false).await;
    let config = storage_with_vector(Some(vector));
    let error = StorageLayer::new(&config)
        .await
        .expect_err("reachable validation followed by a failed Qdrant init must fail startup");
    server_task.await.expect("mock Qdrant task should finish");
    assert!(error.to_string().contains("503 Service Unavailable"));
}

#[tokio::test]
async fn vector_db_runtime_failure_with_allow_degraded_continues_without_vector() {
    let (vector, server_task) = failing_qdrant_config(true).await;
    let config = storage_with_vector(Some(vector));
    let storage = StorageLayer::new(&config)
        .await
        .expect("allow_degraded=true must tolerate a failed Qdrant init");
    server_task.await.expect("mock Qdrant task should finish");
    assert!(storage.vector.is_none());
    assert_eq!(storage.vector_status, DependencyStatus::Degraded);
}

#[tokio::test]
async fn invalid_vector_db_is_not_hidden_by_allow_degraded() {
    let config = storage_with_vector(Some(vector_test_config(
        "weaviate",
        "http://127.0.0.1:1".to_string(),
        true,
    )));
    let error = StorageLayer::new(&config)
        .await
        .expect_err("allow_degraded must not hide invalid vector configuration");
    assert!(error.to_string().contains("not implemented yet"));
}

#[tokio::test]
async fn vector_db_disabled_is_status_disabled() {
    let storage = StorageLayer::new(&storage_with_vector(None))
        .await
        .expect("storage layer must init without vector DB");
    assert_eq!(storage.vector_status, DependencyStatus::Disabled);
}
