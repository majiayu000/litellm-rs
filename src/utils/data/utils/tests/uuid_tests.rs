use crate::utils::data::utils::DataUtils;
use uuid::Uuid;

// ==================== UUID Tests ====================

#[test]
fn test_uuid_generation() {
    let uuid1 = DataUtils::generate_uuid();
    let uuid2 = DataUtils::generate_uuid();
    assert_ne!(uuid1, uuid2);
    assert!(Uuid::parse_str(&uuid1).is_ok());

    let short_id = DataUtils::generate_short_id();
    assert_eq!(short_id.len(), 8);
}

#[test]
fn test_uuid_format() {
    let uuid = DataUtils::generate_uuid();
    // UUID format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
    assert_eq!(uuid.len(), 36);
    assert_eq!(uuid.chars().nth(8), Some('-'));
    assert_eq!(uuid.chars().nth(13), Some('-'));
    assert_eq!(uuid.chars().nth(18), Some('-'));
    assert_eq!(uuid.chars().nth(23), Some('-'));
}

#[test]
fn test_short_id_uniqueness() {
    let mut ids = std::collections::HashSet::new();
    for _ in 0..100 {
        let id = DataUtils::generate_short_id();
        assert!(ids.insert(id), "Short IDs should be unique");
    }
}

#[test]
fn test_short_id_alphanumeric() {
    let id = DataUtils::generate_short_id();
    assert!(id.chars().all(|c| c.is_alphanumeric() || c == '-'));
}
