//! Event subscription functionality for the pricing service

use super::service::PricingService;
use super::types::PricingUpdateEvent;
use tokio::sync::broadcast;

impl PricingService {
    /// Subscribe to pricing update events
    pub fn subscribe_to_updates(&self) -> broadcast::Receiver<PricingUpdateEvent> {
        self.event_sender.subscribe()
    }
}
