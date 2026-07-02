use super::*;
// ==================== EventBroker Subscribe Tests ====================

#[test]
fn test_broker_subscribe() {
    let broker = EventBroker::<TestData>::new();
    let (handle, _rx) = broker.subscribe();

    assert!(handle.is_active());
    assert_eq!(broker.subscriber_count(), 1);
}

#[test]
fn test_broker_subscribe_multiple() {
    let broker = EventBroker::<TestData>::new();

    let (_h1, _r1) = broker.subscribe();
    let (_h2, _r2) = broker.subscribe();
    let (_h3, _r3) = broker.subscribe();

    assert_eq!(broker.subscriber_count(), 3);
}

#[test]
fn test_broker_subscribe_with_capacity() {
    let broker = EventBroker::<TestData>::new();
    let (handle, _rx) = broker.subscribe_with_capacity(1024);

    assert!(handle.is_active());
    assert_eq!(broker.subscriber_count(), 1);
}

#[test]
fn test_broker_has_subscribers() {
    let broker = EventBroker::<TestData>::new();
    assert!(!broker.has_subscribers());

    let (_handle, _rx) = broker.subscribe();
    assert!(broker.has_subscribers());
}

// ==================== EventBroker Unsubscribe Tests ====================

#[test]
fn test_broker_unsubscribe() {
    let broker = EventBroker::<TestData>::new();
    let (handle, _rx) = broker.subscribe();

    assert_eq!(broker.subscriber_count(), 1);

    let removed = broker.unsubscribe(&handle);
    assert!(removed);
    assert!(!handle.is_active());
    assert_eq!(broker.subscriber_count(), 0);
}

#[test]
fn test_broker_unsubscribe_by_id() {
    let broker = EventBroker::<TestData>::new();
    let (handle, _rx) = broker.subscribe();
    let id = handle.id.clone();

    assert_eq!(broker.subscriber_count(), 1);

    let removed = broker.unsubscribe_by_id(&id);
    assert!(removed);
    assert_eq!(broker.subscriber_count(), 0);
}

#[test]
fn test_broker_unsubscribe_unknown() {
    let broker = EventBroker::<TestData>::new();
    let handle = SubscriptionHandle::new();

    let removed = broker.unsubscribe(&handle);
    assert!(!removed);
}

#[test]
fn test_broker_unsubscribe_by_unknown_id() {
    let broker = EventBroker::<TestData>::new();

    let removed = broker.unsubscribe_by_id("unknown-id");
    assert!(!removed);
}

#[test]
fn test_broker_clear() {
    let broker = EventBroker::<TestData>::new();

    let (h1, _r1) = broker.subscribe();
    let (h2, _r2) = broker.subscribe();

    assert_eq!(broker.subscriber_count(), 2);

    broker.clear();

    assert_eq!(broker.subscriber_count(), 0);
    assert!(!h1.is_active());
    assert!(!h2.is_active());
}
