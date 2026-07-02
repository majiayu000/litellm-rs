use super::*;
// ==================== SubscriptionHandle Tests ====================

#[test]
fn test_subscription_handle_new() {
    let handle = SubscriptionHandle::new();

    assert!(!handle.id.is_empty());
    assert!(handle.is_active());
}

#[test]
fn test_subscription_handle_cancel() {
    let handle = SubscriptionHandle::new();
    assert!(handle.is_active());

    handle.cancel();
    assert!(!handle.is_active());
}

#[test]
fn test_subscription_handle_default() {
    let handle = SubscriptionHandle::default();
    assert!(handle.is_active());
}

#[test]
fn test_subscription_handle_unique_ids() {
    let handle1 = SubscriptionHandle::new();
    let handle2 = SubscriptionHandle::new();

    assert_ne!(handle1.id, handle2.id);
}
