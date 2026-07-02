use super::*;
// ==================== Config Tests ====================

#[test]
fn test_broker_config_default() {
    let config = EventBrokerConfig::default();
    assert_eq!(config.channel_capacity, 256);
    assert!(config.log_dropped_events);
}

#[test]
fn test_broker_config_with_capacity() {
    let config = EventBrokerConfig::with_capacity(1024);
    assert_eq!(config.channel_capacity, 1024);
    assert!(config.log_dropped_events);
}

#[test]
fn test_broker_config_clone() {
    let config = EventBrokerConfig {
        channel_capacity: 512,
        log_dropped_events: false,
    };
    let cloned = config.clone();

    assert_eq!(cloned.channel_capacity, 512);
    assert!(!cloned.log_dropped_events);
}
