use super::*;
// ==================== Edge Cases ====================

#[tokio::test]
async fn test_broker_empty_string_data() {
    let broker = EventBroker::<String>::new();
    let (_handle, mut rx) = broker.subscribe();

    let event = Event::created(String::new());
    broker.publish(event).await;

    let received = rx.recv().await.unwrap();
    assert_eq!(received.data, "");
}

#[tokio::test]
async fn test_broker_large_data() {
    let broker = EventBroker::<Vec<u8>>::new();
    let (_handle, mut rx) = broker.subscribe();

    let large_data: Vec<u8> = vec![0u8; 1024 * 1024]; // 1MB
    let event = Event::created(large_data.clone());

    broker.publish(event).await;

    let received = rx.recv().await.unwrap();
    assert_eq!(received.data.len(), 1024 * 1024);
}

#[tokio::test]
async fn test_broker_zero_capacity() {
    // Zero capacity should still work (becomes 1 internally in tokio)
    let broker = EventBroker::<TestData>::with_capacity(0);
    let (_handle, mut rx) = broker.subscribe();

    let event = Event::created(TestData::new(1, "zero"));
    broker.publish(event).await;

    // Should still be able to receive
    let result = timeout(Duration::from_millis(100), rx.recv()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_broker_rapid_subscribe_unsubscribe() {
    let broker = EventBroker::<TestData>::new();

    for _ in 0..100 {
        let (handle, _rx) = broker.subscribe();
        broker.unsubscribe(&handle);
    }

    assert_eq!(broker.subscriber_count(), 0);
}

#[tokio::test]
async fn test_broker_publish_different_event_types() {
    let broker = EventBroker::<TestData>::new();
    let (_handle, mut rx) = broker.subscribe();

    broker
        .publish(Event::created(TestData::new(1, "created")))
        .await;
    broker
        .publish(Event::updated(TestData::new(2, "updated")))
        .await;
    broker
        .publish(Event::deleted(TestData::new(3, "deleted")))
        .await;
    broker
        .publish(Event::custom(99, TestData::new(4, "custom")))
        .await;

    let e1 = rx.recv().await.unwrap();
    let e2 = rx.recv().await.unwrap();
    let e3 = rx.recv().await.unwrap();
    let e4 = rx.recv().await.unwrap();

    assert_eq!(e1.event_type, EventType::Created);
    assert_eq!(e2.event_type, EventType::Updated);
    assert_eq!(e3.event_type, EventType::Deleted);
    assert_eq!(e4.event_type, EventType::Custom(99));
}
