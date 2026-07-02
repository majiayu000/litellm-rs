use super::*;
// ==================== Subscriber Trait Tests ====================

struct CountingSubscriber {
    count: AtomicU32,
}

impl CountingSubscriber {
    fn new() -> Self {
        Self {
            count: AtomicU32::new(0),
        }
    }

    fn count(&self) -> u32 {
        self.count.load(Ordering::Relaxed)
    }
}

#[async_trait::async_trait]
impl Subscriber<TestData> for CountingSubscriber {
    async fn on_event(&self, _event: Event<TestData>) {
        self.count.fetch_add(1, Ordering::Relaxed);
    }
}

struct FilteringSubscriber {
    filter_id: u64,
    count: AtomicU32,
}

impl FilteringSubscriber {
    fn new(filter_id: u64) -> Self {
        Self {
            filter_id,
            count: AtomicU32::new(0),
        }
    }

    fn count(&self) -> u32 {
        self.count.load(Ordering::Relaxed)
    }
}

#[async_trait::async_trait]
impl Subscriber<TestData> for FilteringSubscriber {
    async fn on_event(&self, _event: Event<TestData>) {
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    fn should_receive(&self, event: &Event<TestData>) -> bool {
        event.data.id == self.filter_id
    }
}

#[tokio::test]
async fn test_subscriber_trait_counting() {
    let subscriber = Arc::new(CountingSubscriber::new());

    let event1 = Event::created(TestData::new(1, "a"));
    let event2 = Event::created(TestData::new(2, "b"));

    subscriber.on_event(event1).await;
    subscriber.on_event(event2).await;

    assert_eq!(subscriber.count(), 2);
}

#[tokio::test]
async fn test_subscriber_trait_filtering() {
    let subscriber = FilteringSubscriber::new(42);

    let event1 = Event::created(TestData::new(1, "no"));
    let event2 = Event::created(TestData::new(42, "yes"));
    let event3 = Event::created(TestData::new(100, "no"));

    assert!(!subscriber.should_receive(&event1));
    assert!(subscriber.should_receive(&event2));
    assert!(!subscriber.should_receive(&event3));
    assert_eq!(subscriber.count(), 0);
}
