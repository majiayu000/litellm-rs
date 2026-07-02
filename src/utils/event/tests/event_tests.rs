use super::*;
// ==================== Event Tests ====================

#[test]
fn test_event_new() {
    let data = TestData::new(1, "test");
    let event = Event::new(EventType::Created, data.clone());

    assert!(!event.id.is_empty());
    assert_eq!(event.event_type, EventType::Created);
    assert_eq!(event.data, data);
    assert!(event.timestamp > 0);
    assert!(event.source.is_none());
    assert!(event.correlation_id.is_none());
}

#[test]
fn test_event_created() {
    let data = TestData::new(1, "created");
    let event = Event::created(data.clone());

    assert_eq!(event.event_type, EventType::Created);
    assert_eq!(event.data, data);
}

#[test]
fn test_event_updated() {
    let data = TestData::new(2, "updated");
    let event = Event::updated(data.clone());

    assert_eq!(event.event_type, EventType::Updated);
    assert_eq!(event.data, data);
}

#[test]
fn test_event_deleted() {
    let data = TestData::new(3, "deleted");
    let event = Event::deleted(data.clone());

    assert_eq!(event.event_type, EventType::Deleted);
    assert_eq!(event.data, data);
}

#[test]
fn test_event_custom() {
    let data = TestData::new(4, "custom");
    let event = Event::custom(42, data.clone());

    assert_eq!(event.event_type, EventType::Custom(42));
    assert_eq!(event.data, data);
}

#[test]
fn test_event_with_source() {
    let data = TestData::new(1, "test");
    let event = Event::created(data).with_source("test-component");

    assert_eq!(event.source, Some("test-component".to_string()));
}

#[test]
fn test_event_with_correlation_id() {
    let data = TestData::new(1, "test");
    let event = Event::created(data).with_correlation_id("corr-123");

    assert_eq!(event.correlation_id, Some("corr-123".to_string()));
}

#[test]
fn test_event_builder_chain() {
    let data = TestData::new(1, "test");
    let event = Event::created(data)
        .with_source("component-a")
        .with_correlation_id("trace-456");

    assert_eq!(event.source, Some("component-a".to_string()));
    assert_eq!(event.correlation_id, Some("trace-456".to_string()));
}

#[test]
fn test_event_is_type() {
    let event = Event::created(TestData::new(1, "test"));

    assert!(event.is_type(EventType::Created));
    assert!(!event.is_type(EventType::Updated));
    assert!(!event.is_type(EventType::Deleted));
}

#[test]
fn test_event_unique_ids() {
    let data = TestData::new(1, "test");
    let event1 = Event::created(data.clone());
    let event2 = Event::created(data);

    assert_ne!(event1.id, event2.id);
}

#[test]
fn test_event_clone() {
    let data = TestData::new(1, "test");
    let event = Event::created(data).with_source("src");
    let cloned = event.clone();

    assert_eq!(event.id, cloned.id);
    assert_eq!(event.event_type, cloned.event_type);
    assert_eq!(event.data, cloned.data);
    assert_eq!(event.source, cloned.source);
}
