//! Tests for file storage implementations

use super::default_data_path;
use super::local::LocalStorage;
use super::types::FileOwnerScope;
use std::path::PathBuf;
use tempfile::TempDir;
use uuid::Uuid;

struct DataDirEnvRestore(Option<String>);

impl DataDirEnvRestore {
    fn capture() -> Self {
        Self(std::env::var("LITELLM_DATA_DIR").ok())
    }
}

impl Drop for DataDirEnvRestore {
    fn drop(&mut self) {
        unsafe {
            match &self.0 {
                Some(value) => std::env::set_var("LITELLM_DATA_DIR", value),
                None => std::env::remove_var("LITELLM_DATA_DIR"),
            }
        }
    }
}

#[tokio::test]
async fn test_local_storage() {
    let temp_dir = TempDir::new().unwrap();
    let storage = LocalStorage::new(temp_dir.path().to_str().unwrap())
        .await
        .unwrap();

    // Test store
    let content = b"Hello, World!";
    let file_id = storage.store("test.txt", content).await.unwrap();
    assert!(!file_id.is_empty());

    // Test exists
    assert!(storage.exists(&file_id).await.unwrap());

    // Test get
    let retrieved = storage.get(&file_id).await.unwrap();
    assert_eq!(retrieved, content);

    // Test metadata
    let metadata = storage.metadata(&file_id).await.unwrap();
    assert_eq!(metadata.filename, "test.txt");
    assert_eq!(metadata.size, content.len() as u64);
    assert_eq!(metadata.purpose, None);

    // Test delete
    storage.delete(&file_id).await.unwrap();
    assert!(!storage.exists(&file_id).await.unwrap());
}

#[tokio::test]
async fn test_local_storage_persists_purpose_metadata() {
    let temp_dir = TempDir::new().unwrap();
    let storage = LocalStorage::new(temp_dir.path().to_str().unwrap())
        .await
        .unwrap();

    let file_id = storage
        .store_with_purpose("batch.jsonl", b"{\"custom_id\":\"1\"}\n", Some("batch"))
        .await
        .unwrap();

    let metadata = storage.metadata(&file_id).await.unwrap();
    assert_eq!(metadata.filename, "batch.jsonl");
    assert_eq!(metadata.purpose.as_deref(), Some("batch"));
}

#[tokio::test]
async fn test_local_storage_list_returns_stored_files() {
    let temp_dir = TempDir::new().unwrap();
    let storage = LocalStorage::new(temp_dir.path().to_str().unwrap())
        .await
        .unwrap();

    let first_id = storage
        .store("first.jsonl", b"{\"index\":1}\n")
        .await
        .unwrap();
    let second_id = storage
        .store("second.jsonl", b"{\"index\":2}\n")
        .await
        .unwrap();

    let mut listed = storage.list(None, None).await.unwrap();
    listed.sort();

    let mut expected = vec![first_id.clone(), second_id.clone()];
    expected.sort();
    assert_eq!(listed, expected);

    assert_eq!(
        storage.list(Some(&first_id), None).await.unwrap(),
        vec![first_id]
    );

    let limited = storage.list(None, Some(1)).await.unwrap();
    assert_eq!(limited.len(), 1);
    assert!(expected.contains(&limited[0]));

    for file_id in listed {
        let metadata = storage.metadata(&file_id).await.unwrap();
        assert_eq!(metadata.id, file_id);
        assert!(metadata.filename.ends_with(".jsonl"));
    }
}

#[tokio::test]
async fn gh1130_owned_local_store_survives_reopen_and_legacy_stays_unowned() {
    let temp_dir = TempDir::new().unwrap();
    let owner = FileOwnerScope::ApiKey(Uuid::new_v4());
    let owned_id;
    let legacy_id;
    {
        let storage = LocalStorage::new(temp_dir.path().to_str().unwrap())
            .await
            .unwrap();
        owned_id = storage
            .store_owned_with_purpose("owned.jsonl", b"{}\n", Some("batch"), owner.clone())
            .await
            .unwrap();
        legacy_id = storage.store("legacy.jsonl", b"{}\n").await.unwrap();
    }

    let reopened = LocalStorage::new(temp_dir.path().to_str().unwrap())
        .await
        .unwrap();
    assert_eq!(
        reopened
            .metadata_with_owner(&owned_id)
            .await
            .unwrap()
            .owner(),
        Some(&owner)
    );
    assert_eq!(
        reopened
            .metadata_with_owner(&legacy_id)
            .await
            .unwrap()
            .owner(),
        None
    );
    assert!(reopened.list(None, Some(0)).await.unwrap().is_empty());
}

#[test]
fn test_default_data_path_is_absolute() {
    let _guard = crate::storage::ENV_LOCK.lock().unwrap();
    let _restore = DataDirEnvRestore::capture();
    unsafe {
        std::env::remove_var("LITELLM_DATA_DIR");
    }

    // Ensure the default path is absolute, not relative like the old "./data"
    let path = default_data_path();
    assert!(
        path.is_absolute(),
        "default_data_path() should return an absolute path, got: {}",
        path.display()
    );
}

#[test]
fn test_default_data_path_ends_with_data() {
    let _guard = crate::storage::ENV_LOCK.lock().unwrap();
    let _restore = DataDirEnvRestore::capture();
    unsafe {
        std::env::remove_var("LITELLM_DATA_DIR");
    }

    let path = default_data_path();
    assert!(
        path.ends_with("litellm-rs/data"),
        "default_data_path() should end with litellm-rs/data, got: {}",
        path.display()
    );
}

#[test]
fn test_default_data_path_env_override() {
    let _guard = crate::storage::ENV_LOCK.lock().unwrap();
    let _restore = DataDirEnvRestore::capture();
    // SAFETY: ENV_LOCK serializes tests in this crate that mutate
    // LITELLM_DATA_DIR. set_var/remove_var are unsafe because environment
    // mutation is process-global.
    unsafe {
        std::env::set_var("LITELLM_DATA_DIR", "/custom/storage/path");
    }
    let path = default_data_path();
    assert_eq!(
        path,
        PathBuf::from("/custom/storage/path/data"),
        "LITELLM_DATA_DIR should define the shared state directory"
    );
}

#[test]
fn test_content_type_detection() {
    assert_eq!(LocalStorage::detect_content_type("test.txt"), "text/plain");
    assert_eq!(
        LocalStorage::detect_content_type("data.json"),
        "application/json"
    );
    assert_eq!(LocalStorage::detect_content_type("image.png"), "image/png");
    assert_eq!(
        LocalStorage::detect_content_type("unknown"),
        "application/octet-stream"
    );
}

#[cfg(feature = "s3")]
mod s3_fixture {
    use super::super::s3::S3Storage;
    use crate::utils::error::gateway_error::GatewayError;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use url::Url;

    async fn fixture(responses: Vec<(u16, String)>) -> (S3Storage, Arc<Mutex<Vec<String>>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        tokio::spawn(async move {
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                loop {
                    let mut chunk = [0; 2048];
                    let size = stream.read(&mut chunk).await.unwrap();
                    if size == 0 {
                        break;
                    }
                    request.extend_from_slice(&chunk[..size]);
                    if request.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }
                recorded
                    .lock()
                    .unwrap()
                    .push(String::from_utf8(request).unwrap());
                let reason = if status == 200 {
                    "OK"
                } else {
                    "Internal Server Error"
                };
                let response = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Type: application/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            }
        });
        (S3Storage::test_endpoint(endpoint), requests)
    }

    fn page(keys: impl IntoIterator<Item = String>, truncated: bool, token: &str) -> String {
        let contents = keys
            .into_iter()
            .map(|key| format!("<Contents><Key>{key}</Key></Contents>"))
            .collect::<String>();
        format!(
            r#"<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/"><IsTruncated>{truncated}</IsTruncated>{token}{contents}</ListBucketResult>"#
        )
    }

    fn query(request: &str) -> Vec<(String, String)> {
        let target = request
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap();
        Url::parse(&format!("http://fixture{target}"))
            .unwrap()
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect()
    }

    #[tokio::test]
    async fn gh1130_list_objects_paginates_past_1000_with_stable_prefix_and_limit() {
        let first = page(
            (0..1000).map(|index| format!("tenant/{index:04}")),
            true,
            "<NextContinuationToken>page-2</NextContinuationToken>",
        );
        let second = page(["tenant/1000".into()], false, "");
        let (storage, requests) = fixture(vec![(200, first), (200, second)]).await;

        let keys = storage.list(Some("tenant/"), Some(1001)).await.unwrap();
        assert_eq!(keys.len(), 1001);
        assert_eq!(keys.first().unwrap(), "tenant/0000");
        assert_eq!(keys.last().unwrap(), "tenant/1000");

        let requests = requests.lock().unwrap();
        let first_query = query(&requests[0]);
        let second_query = query(&requests[1]);
        for query in [&first_query, &second_query] {
            assert!(query.contains(&("list-type".into(), "2".into())));
            assert!(query.contains(&("prefix".into(), "tenant/".into())));
        }
        assert!(first_query.contains(&("max-keys".into(), "1000".into())));
        assert!(second_query.contains(&("max-keys".into(), "1".into())));
        assert!(second_query.contains(&("continuation-token".into(), "page-2".into())));
    }

    #[tokio::test]
    async fn gh1130_list_objects_surfaces_later_page_error() {
        let first = page(
            ["tenant/0000".into()],
            true,
            "<NextContinuationToken>page-2</NextContinuationToken>",
        );
        let error = "<Error><Code>InternalError</Code><Message>later page</Message></Error>".into();
        let (storage, requests) = fixture(vec![(200, first), (500, error)]).await;

        assert!(storage.list(Some("tenant/"), None).await.is_err());
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(query(&requests[1]).contains(&("continuation-token".into(), "page-2".into())));
    }

    #[tokio::test]
    async fn gh1130_head_maps_only_404_to_canonical_not_found() {
        for (status, code, canonical_not_found) in [
            (404, "NoSuchKey", true),
            (403, "AccessDenied", false),
            (500, "InternalError", false),
        ] {
            let body = format!("<Error><Code>{code}</Code><Message>fixture</Message></Error>");
            let (storage, requests) = fixture(vec![(status, body)]).await;
            let error = storage
                .metadata_with_owner("tenant/object.jsonl")
                .await
                .unwrap_err();

            assert_eq!(
                matches!(&error, GatewayError::NotFound(_)),
                canonical_not_found,
                "status {status} mapped to {error:?}"
            );
            assert!(
                canonical_not_found || matches!(&error, GatewayError::Internal(_)),
                "status {status} was not an explicit internal failure: {error:?}"
            );
            let requests = requests.lock().unwrap();
            assert_eq!(requests.len(), 1, "status {status} retried unexpectedly");
            assert!(
                requests[0].starts_with("HEAD /test-bucket/tenant/object.jsonl HTTP/1.1\r\n"),
                "unexpected HEAD request: {}",
                requests[0].lines().next().unwrap_or_default()
            );
        }
    }
}
