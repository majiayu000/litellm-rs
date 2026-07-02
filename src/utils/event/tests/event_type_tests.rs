use super::*;
// ==================== EventType Tests ====================

#[test]
fn test_event_type_created() {
    let event_type = EventType::Created;
    assert!(event_type.is_created());
    assert!(!event_type.is_updated());
    assert!(!event_type.is_deleted());
    assert!(!event_type.is_custom());
    assert_eq!(event_type.custom_code(), None);
}

#[test]
fn test_event_type_updated() {
    let event_type = EventType::Updated;
    assert!(!event_type.is_created());
    assert!(event_type.is_updated());
    assert!(!event_type.is_deleted());
    assert!(!event_type.is_custom());
}

#[test]
fn test_event_type_deleted() {
    let event_type = EventType::Deleted;
    assert!(!event_type.is_created());
    assert!(!event_type.is_updated());
    assert!(event_type.is_deleted());
    assert!(!event_type.is_custom());
}

#[test]
fn test_event_type_custom() {
    let event_type = EventType::Custom(42);
    assert!(!event_type.is_created());
    assert!(!event_type.is_updated());
    assert!(!event_type.is_deleted());
    assert!(event_type.is_custom());
    assert_eq!(event_type.custom_code(), Some(42));
}

#[test]
fn test_event_type_display() {
    assert_eq!(format!("{}", EventType::Created), "Created");
    assert_eq!(format!("{}", EventType::Updated), "Updated");
    assert_eq!(format!("{}", EventType::Deleted), "Deleted");
    assert_eq!(format!("{}", EventType::Custom(100)), "Custom(100)");
}

#[test]
fn test_event_type_equality() {
    assert_eq!(EventType::Created, EventType::Created);
    assert_ne!(EventType::Created, EventType::Updated);
    assert_eq!(EventType::Custom(1), EventType::Custom(1));
    assert_ne!(EventType::Custom(1), EventType::Custom(2));
}

#[test]
fn test_event_type_clone() {
    let original = EventType::Custom(99);
    let cloned = original;
    assert_eq!(original, cloned);
}
