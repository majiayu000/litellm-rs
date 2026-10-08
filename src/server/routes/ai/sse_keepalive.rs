//! Downstream SSE keepalives expose closed readers during an idle provider wait.

use bytes::Bytes;
use futures::Stream;
use std::time::Duration;
use tokio::sync::mpsc;

/// Comments carry no provider output or usage and do not restart the producer's
/// upstream idle timeout. The body owns the receiver, so a transport error
/// closes it and wakes the producer's existing cancellation/settlement path.
pub(super) fn channel_body(
    rx: mpsc::Receiver<Bytes>,
) -> impl Stream<Item = Result<Bytes, actix_web::Error>> {
    let period = Duration::from_secs(1);
    let mut heartbeat = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    futures::stream::unfold((rx, heartbeat), |(mut rx, mut heartbeat)| async move {
        let bytes = tokio::select! {
            biased;
            bytes = rx.recv() => bytes?,
            _ = heartbeat.tick() => Bytes::from_static(b": keep-alive\n\n"),
        };
        Some((Ok::<_, actix_web::Error>(bytes), (rx, heartbeat)))
    })
}
