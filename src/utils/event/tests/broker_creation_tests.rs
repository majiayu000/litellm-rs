use super::*;
// ==================== EventBroker Creation Tests ====================

#[test]
fn test_broker_new() {
    let broker = EventBroker::<TestData>::new();
    assert_eq!(broker.subscriber_count(), 0);
    assert_eq!(broker.events_published(), 0);
    assert_eq!(broker.events_dropped(), 0);
}

#[test]
fn test_broker_with_capacity() {
    let broker = EventBroker::<TestData>::with_capacity(100);
    assert_eq!(broker.subscriber_count(), 0);
}

#[test]
fn test_broker_with_config() {
    let config = EventBrokerConfig {
        channel_capacity: 512,
        log_dropped_events: false,
    };
    let broker = EventBroker::<TestData>::with_config(config);
    assert_eq!(broker.subscriber_count(), 0);
}

#[test]
fn test_broker_default() {
    let broker = EventBroker::<TestData>::default();
    assert_eq!(broker.subscriber_count(), 0);
}

#[test]
fn test_broker_debug() {
    let broker = EventBroker::<TestData>::new();
    let debug_str = format!("{:?}", broker);
    assert!(debug_str.contains("EventBroker"));
    assert!(debug_str.contains("subscriber_count"));
}
