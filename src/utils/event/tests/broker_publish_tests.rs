use super::*;
// ==================== EventBroker Publish Tests ====================

#[tokio::test]
async fn test_broker_publish_no_subscribers() {
    let broker = EventBroker::<TestData>::new();
    let event = Event::created(TestData::new(1, "test"));

    let delivered = broker.publish(event).await;

    assert_eq!(delivered, 0);
    assert_eq!(broker.events_published(), 1);
}

#[tokio::test]
async fn test_broker_publish_single_subscriber() {
    let broker = EventBroker::<TestData>::new();
    let (_handle, mut rx) = broker.subscribe();

    let data = TestData::new(1, "hello");
    let event = Event::created(data.clone());
    let event_id = event.id.clone();

    let delivered = broker.publish(event).await;
    assert_eq!(delivered, 1);

    let received = rx.recv().await.unwrap();
    assert_eq!(received.id, event_id);
    assert_eq!(received.data, data);
}

#[tokio::test]
async fn test_broker_publish_multiple_subscribers() {
    let broker = EventBroker::<TestData>::new();

    let (_h1, mut r1) = broker.subscribe();
    let (_h2, mut r2) = broker.subscribe();
    let (_h3, mut r3) = broker.subscribe();

    let data = TestData::new(1, "broadcast");
    let event = Event::created(data.clone());

    let delivered = broker.publish(event).await;
    assert_eq!(delivered, 3);

    let e1 = r1.recv().await.unwrap();
    let e2 = r2.recv().await.unwrap();
    let e3 = r3.recv().await.unwrap();

    assert_eq!(e1.data, data);
    assert_eq!(e2.data, data);
    assert_eq!(e3.data, data);
}

#[tokio::test]
async fn test_broker_publish_multiple_events() {
    let broker = EventBroker::<TestData>::new();
    let (_handle, mut rx) = broker.subscribe();

    for i in 0..5 {
        let event = Event::created(TestData::new(i, &format!("event-{}", i)));
        broker.publish(event).await;
    }

    assert_eq!(broker.events_published(), 5);

    for i in 0..5 {
        let received = rx.recv().await.unwrap();
        assert_eq!(received.data.id, i);
    }
}

#[tokio::test]
async fn test_broker_publish_blocking() {
    let broker = EventBroker::<TestData>::new();
    let (_handle, mut rx) = broker.subscribe();

    let data = TestData::new(1, "blocking");
    let event = Event::created(data.clone());

    let delivered = broker.publish_blocking(event).await;
    assert_eq!(delivered, 1);

    let received = rx.recv().await.unwrap();
    assert_eq!(received.data, data);
}

#[tokio::test]
async fn test_broker_publish_after_unsubscribe() {
    let broker = EventBroker::<TestData>::new();
    let (handle, _rx) = broker.subscribe();

    broker.unsubscribe(&handle);

    let event = Event::created(TestData::new(1, "test"));
    let delivered = broker.publish(event).await;

    assert_eq!(delivered, 0);
}

// ==================== Non-blocking Delivery Tests ====================

#[tokio::test]
async fn test_broker_non_blocking_slow_consumer() {
    // Create broker with very small capacity
    let broker = EventBroker::<TestData>::with_capacity(2);
    let (_handle, _rx) = broker.subscribe(); // Don't read from rx

    // Publish more events than capacity
    for i in 0..10 {
        let event = Event::created(TestData::new(i, "overflow"));
        broker.publish(event).await;
    }

    // Should have dropped some events
    assert!(broker.events_dropped() > 0);
    assert_eq!(broker.events_published(), 10);
}

#[tokio::test]
async fn test_broker_fast_consumer_no_drops() {
    let broker = EventBroker::<TestData>::with_capacity(10);
    let (_handle, mut rx) = broker.subscribe();

    // Spawn consumer
    let consumer = tokio::spawn(async move {
        let mut count = 0;
        while let Ok(Some(_)) = timeout(Duration::from_millis(100), rx.recv()).await {
            count += 1;
        }
        count
    });

    // Publish events
    for i in 0..5 {
        let event = Event::created(TestData::new(i, "fast"));
        broker.publish(event).await;
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    broker.clear();
    let received = consumer.await.unwrap();

    assert_eq!(received, 5);
    assert_eq!(broker.events_dropped(), 0);
}

// ==================== Closed Channel Tests ====================

#[tokio::test]
async fn test_broker_removes_closed_channels() {
    let broker = EventBroker::<TestData>::new();
    let (_handle, rx) = broker.subscribe();

    assert_eq!(broker.subscriber_count(), 1);

    // Drop the receiver to close the channel
    drop(rx);

    // Publish should detect closed channel and remove subscriber
    let event = Event::created(TestData::new(1, "test"));
    let delivered = broker.publish(event).await;

    assert_eq!(delivered, 0);
    assert_eq!(broker.subscriber_count(), 0);
}

// ==================== Stats Tests ====================

#[tokio::test]
async fn test_broker_stats() {
    let broker = EventBroker::<TestData>::with_capacity(100);

    let (_h1, _r1) = broker.subscribe();
    let (_h2, _r2) = broker.subscribe();

    for i in 0..5 {
        let event = Event::created(TestData::new(i, "stats"));
        broker.publish(event).await;
    }

    let stats = broker.stats();

    assert_eq!(stats.subscriber_count, 2);
    assert_eq!(stats.events_published, 5);
    assert_eq!(stats.events_dropped, 0);
    assert_eq!(stats.channel_capacity, 100);
}
