use super::*;
// ==================== Concurrent Tests ====================

#[tokio::test]
async fn test_broker_concurrent_subscribe() {
    let broker = Arc::new(EventBroker::<TestData>::new());
    let mut handles = vec![];

    for _ in 0..10 {
        let broker_clone = broker.clone();
        let handle = tokio::spawn(async move {
            let (_h, _r) = broker_clone.subscribe();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    assert_eq!(broker.subscriber_count(), 10);
}

#[tokio::test]
async fn test_broker_concurrent_publish() {
    let broker = Arc::new(EventBroker::<TestData>::new());
    let (_handle, mut rx) = broker.subscribe();

    let received_count = Arc::new(AtomicU32::new(0));
    let received_count_clone = received_count.clone();

    // Spawn consumer
    let consumer = tokio::spawn(async move {
        while let Ok(Some(_)) = timeout(Duration::from_millis(500), rx.recv()).await {
            received_count_clone.fetch_add(1, Ordering::Relaxed);
        }
    });

    // Spawn multiple publishers
    let mut publisher_handles = vec![];
    for i in 0..5 {
        let broker_clone = broker.clone();
        let handle = tokio::spawn(async move {
            for j in 0..10 {
                let event = Event::created(TestData::new(i * 10 + j, "concurrent"));
                broker_clone.publish(event).await;
            }
        });
        publisher_handles.push(handle);
    }

    // Wait for publishers
    for handle in publisher_handles {
        handle.await.unwrap();
    }

    broker.clear();
    let _ = consumer.await;

    assert_eq!(broker.events_published(), 50);
    assert!(received_count.load(Ordering::Relaxed) > 0);
}

#[tokio::test]
async fn test_broker_concurrent_subscribe_unsubscribe() {
    let broker = Arc::new(EventBroker::<TestData>::new());
    let barrier = Arc::new(Barrier::new(20));

    let mut handles = vec![];

    // 10 subscribers
    for _ in 0..10 {
        let broker_clone = broker.clone();
        let barrier_clone = barrier.clone();
        let handle = tokio::spawn(async move {
            barrier_clone.wait().await;
            let (h, _r) = broker_clone.subscribe();
            tokio::time::sleep(Duration::from_millis(50)).await;
            broker_clone.unsubscribe(&h);
        });
        handles.push(handle);
    }

    // 10 publishers
    for i in 0..10 {
        let broker_clone = broker.clone();
        let barrier_clone = barrier.clone();
        let handle = tokio::spawn(async move {
            barrier_clone.wait().await;
            for j in 0..5 {
                let event = Event::created(TestData::new(i * 5 + j, "chaos"));
                broker_clone.publish(event).await;
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    // All subscribers should have unsubscribed
    assert_eq!(broker.subscriber_count(), 0);
    assert_eq!(broker.events_published(), 50);
}

#[tokio::test]
async fn test_broker_concurrent_clear() {
    let broker = Arc::new(EventBroker::<TestData>::new());

    // Add subscribers
    for _ in 0..5 {
        broker.subscribe();
    }

    let broker_clone = broker.clone();
    let clear_handle = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        broker_clone.clear();
    });

    let broker_clone2 = broker.clone();
    let publish_handle = tokio::spawn(async move {
        for i in 0..20 {
            let event = Event::created(TestData::new(i, "during-clear"));
            broker_clone2.publish(event).await;
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    });

    clear_handle.await.unwrap();
    publish_handle.await.unwrap();

    // After clear, no subscribers
    assert_eq!(broker.subscriber_count(), 0);
}
