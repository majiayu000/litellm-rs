//! Blocking compatibility for router APIs whose I/O runs on owned runtimes.

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};

struct ThreadNotify {
    thread: Thread,
    notified: AtomicBool,
}

impl Wake for ThreadNotify {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.notified.store(true, Ordering::Release);
        self.thread.unpark();
    }
}

/// Preserve the blocking API without entering a caller's futures executor.
///
/// Redis work is driven by the admission/circuit runtimes, so this waiter only
/// polls their results. Each invocation owns its notification flag: a nested
/// waiter may consume the thread's park token without losing an outer wake.
pub(super) fn wait<F: Future>(future: F) -> F::Output {
    let notify = Arc::new(ThreadNotify {
        thread: thread::current(),
        notified: AtomicBool::new(false),
    });
    let waker = Waker::from(notify.clone());
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);

    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => {
                while !notify.notified.swap(false, Ordering::Acquire) {
                    thread::park();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::poll_fn;

    #[test]
    fn nested_wait_preserves_an_outer_notification() {
        let mut outer_polled = false;
        wait(poll_fn(|outer| {
            if outer_polled {
                return Poll::Ready(());
            }
            outer_polled = true;
            let mut inner_polled = false;
            wait(poll_fn(|inner| {
                if inner_polled {
                    return Poll::Ready(());
                }
                inner_polled = true;
                outer.waker().wake_by_ref();
                inner.waker().wake_by_ref();
                // Both waits share a thread park token. Consuming it here
                // must leave each wait's own notification intact.
                thread::park();
                Poll::Pending
            }));
            Poll::Pending
        }));
    }

    #[test]
    fn wait_observes_a_wake_from_another_thread() {
        let (sender, receiver) = std::sync::mpsc::channel::<Waker>();
        let ready = Arc::new(AtomicBool::new(false));
        let worker_ready = ready.clone();
        let worker = thread::spawn(move || {
            let waker = receiver.recv().unwrap();
            worker_ready.store(true, Ordering::Release);
            waker.wake();
        });
        let mut sender = Some(sender);
        let value = wait(poll_fn(|context| {
            if ready.load(Ordering::Acquire) {
                return Poll::Ready(7);
            }
            if let Some(sender) = sender.take() {
                sender.send(context.waker().clone()).unwrap();
            }
            Poll::Pending
        }));
        assert_eq!(value, 7);
        worker.join().unwrap();
    }
}
