use tokio::sync::{Mutex, MutexGuard};

struct BatchState<T> {
    pending: Vec<T>,
    in_flight: Vec<T>,
}

impl<T> Default for BatchState<T> {
    fn default() -> Self {
        Self {
            pending: Vec::new(),
            in_flight: Vec::new(),
        }
    }
}

pub(super) struct DurableBatch<T> {
    state: Mutex<BatchState<T>>,
    flush_lock: Mutex<()>,
    capacity: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct BatchFull {
    capacity: usize,
}

impl std::fmt::Display for BatchFull {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "export batch capacity {} reached; event was not buffered",
            self.capacity
        )
    }
}

impl<T> DurableBatch<T> {
    pub(super) fn new(capacity: usize) -> Self {
        Self {
            state: Mutex::new(BatchState::default()),
            flush_lock: Mutex::new(()),
            capacity: capacity.max(1),
        }
    }
}

impl<T: Clone> DurableBatch<T> {
    pub(super) async fn push(&self, value: T) -> Result<usize, BatchFull> {
        let mut state = self.state.lock().await;
        if state.pending.len().saturating_add(state.in_flight.len()) >= self.capacity {
            return Err(BatchFull {
                capacity: self.capacity,
            });
        }
        state.pending.push(value);
        Ok(state.pending.len())
    }

    pub(super) async fn serialize_flush(&self) -> MutexGuard<'_, ()> {
        self.flush_lock.lock().await
    }

    pub(super) async fn batch_for_export(&self) -> Vec<T> {
        let mut state = self.state.lock().await;
        if !state.pending.is_empty() {
            let pending = std::mem::take(&mut state.pending);
            state.in_flight.extend(pending);
        }
        state.in_flight.clone()
    }

    pub(super) async fn acknowledge(&self) {
        self.state.lock().await.in_flight.clear();
    }

    #[cfg(test)]
    pub(super) async fn snapshot(&self) -> Vec<T> {
        let state = self.state.lock().await;
        state
            .in_flight
            .iter()
            .chain(&state.pending)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Notify;

    #[tokio::test]
    async fn canceled_export_preserves_in_flight_batch_and_retry_order() {
        let batch = Arc::new(DurableBatch::new(2));
        batch.push(1).await.unwrap();
        let entered = Arc::new(Notify::new());
        let task = tokio::spawn({
            let batch = Arc::clone(&batch);
            let entered = Arc::clone(&entered);
            async move {
                let _flush = batch.serialize_flush().await;
                assert_eq!(batch.batch_for_export().await, [1]);
                entered.notify_one();
                std::future::pending::<()>().await;
            }
        });
        entered.notified().await;
        batch.push(2).await.unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert_eq!(batch.snapshot().await, [1, 2]);

        let _flush = batch.serialize_flush().await;
        assert_eq!(batch.batch_for_export().await, [1, 2]);
        batch.acknowledge().await;
        assert!(batch.batch_for_export().await.is_empty());
    }

    #[tokio::test]
    async fn failed_export_retention_is_bounded() {
        let batch = DurableBatch::new(2);
        batch.push(1).await.unwrap();
        assert_eq!(batch.batch_for_export().await, [1]);
        batch.push(2).await.unwrap();

        let error = batch.push(3).await.unwrap_err();
        assert_eq!(error, BatchFull { capacity: 2 });
        assert_eq!(batch.snapshot().await, [1, 2]);
        assert_eq!(batch.batch_for_export().await, [1, 2]);
    }
}
