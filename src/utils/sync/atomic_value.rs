//! Atomic value container using arc-swap
//!
//! Provides a concurrent-safe single value container with atomic swap semantics.
//! Ideal for configuration values or shared state that needs to be updated atomically.

use arc_swap::ArcSwap;
use std::sync::Arc;

/// A concurrent-safe single value container using arc-swap.
///
/// This container provides atomic load and store operations for a single value.
/// It's ideal for configuration values or shared state that needs to be read
/// frequently and updated occasionally.
///
/// # Type Parameters
///
/// * `T` - The value type
///
/// # Example
///
/// ```rust
/// use litellm_rs::utils::sync::AtomicValue;
///
/// let value: AtomicValue<String> = AtomicValue::new("initial".to_string());
/// assert_eq!(value.load().as_ref(), "initial");
///
/// value.store("updated".to_string());
/// assert_eq!(value.load().as_ref(), "updated");
/// ```
#[derive(Debug)]
pub struct AtomicValue<T> {
    inner: ArcSwap<T>,
}

impl<T> AtomicValue<T> {
    /// Creates a new `AtomicValue` with the specified initial value.
    ///
    /// # Arguments
    ///
    /// * `value` - The initial value
    ///
    /// # Example
    ///
    /// ```rust
    /// use litellm_rs::utils::sync::AtomicValue;
    ///
    /// let value: AtomicValue<i32> = AtomicValue::new(42);
    /// ```
    pub fn new(value: T) -> Self {
        Self {
            inner: ArcSwap::from_pointee(value),
        }
    }

    /// Loads the current value.
    ///
    /// Returns an `Arc` pointing to the current value. This is a very fast
    /// operation and does not block other readers or writers.
    ///
    /// # Returns
    ///
    /// An `Arc<T>` containing the current value.
    ///
    /// # Example
    ///
    /// ```rust
    /// use litellm_rs::utils::sync::AtomicValue;
    ///
    /// let value: AtomicValue<i32> = AtomicValue::new(42);
    /// let current = value.load();
    /// assert_eq!(*current, 42);
    /// ```
    pub fn load(&self) -> Arc<T> {
        self.inner.load_full()
    }

    /// Stores a new value, replacing the current one.
    ///
    /// This operation is atomic and will not block readers.
    ///
    /// # Arguments
    ///
    /// * `value` - The new value to store
    ///
    /// # Example
    ///
    /// ```rust
    /// use litellm_rs::utils::sync::AtomicValue;
    ///
    /// let value: AtomicValue<i32> = AtomicValue::new(42);
    /// value.store(100);
    /// assert_eq!(*value.load(), 100);
    /// ```
    pub fn store(&self, value: T) {
        self.inner.store(Arc::new(value));
    }

    /// Swaps the current value with a new one, returning the old value.
    ///
    /// # Arguments
    ///
    /// * `value` - The new value to store
    ///
    /// # Returns
    ///
    /// An `Arc<T>` containing the previous value.
    ///
    /// # Example
    ///
    /// ```rust
    /// use litellm_rs::utils::sync::AtomicValue;
    ///
    /// let value: AtomicValue<i32> = AtomicValue::new(42);
    /// let old = value.swap(100);
    /// assert_eq!(*old, 42);
    /// assert_eq!(*value.load(), 100);
    /// ```
    pub fn swap(&self, value: T) -> Arc<T> {
        self.inner.swap(Arc::new(value))
    }

    /// Stores a new value from an Arc.
    ///
    /// This is useful when you already have an Arc and want to avoid
    /// an extra allocation.
    ///
    /// # Arguments
    ///
    /// * `value` - An Arc containing the new value
    ///
    /// # Example
    ///
    /// ```rust
    /// use litellm_rs::utils::sync::AtomicValue;
    /// use std::sync::Arc;
    ///
    /// let value: AtomicValue<i32> = AtomicValue::new(42);
    /// value.store_arc(Arc::new(100));
    /// assert_eq!(*value.load(), 100);
    /// ```
    pub fn store_arc(&self, value: Arc<T>) {
        self.inner.store(value);
    }

    /// Swaps the current value with a new Arc, returning the old value.
    ///
    /// # Arguments
    ///
    /// * `value` - An Arc containing the new value
    ///
    /// # Returns
    ///
    /// An `Arc<T>` containing the previous value.
    ///
    /// # Example
    ///
    /// ```rust
    /// use litellm_rs::utils::sync::AtomicValue;
    /// use std::sync::Arc;
    ///
    /// let value: AtomicValue<i32> = AtomicValue::new(42);
    /// let old = value.swap_arc(Arc::new(100));
    /// assert_eq!(*old, 42);
    /// ```
    pub fn swap_arc(&self, value: Arc<T>) -> Arc<T> {
        self.inner.swap(value)
    }
}

impl<T: Clone> AtomicValue<T> {
    /// Updates the value using a closure, with atomic compare-and-swap retry.
    ///
    /// This operation uses `ArcSwap::rcu()` to perform a read-copy-update loop,
    /// retrying until the compare-and-swap succeeds. The closure may be called
    /// more than once if concurrent updates occur, so it must be side-effect-free
    /// (i.e., `Fn` rather than `FnOnce`).
    ///
    /// # Arguments
    ///
    /// * `f` - A closure that takes a reference to the current value and returns the new value
    ///
    /// # Returns
    ///
    /// The new value after the update.
    ///
    /// # Example
    ///
    /// ```rust
    /// use litellm_rs::utils::sync::AtomicValue;
    ///
    /// let value: AtomicValue<i32> = AtomicValue::new(42);
    /// let new_value = value.update(|v| v + 1);
    /// assert_eq!(new_value, 43);
    /// ```
    pub fn update<F>(&self, f: F) -> T
    where
        F: Fn(&T) -> T,
    {
        use std::cell::Cell;
        // `rcu` retries the closure until a compare-and-swap succeeds, so the
        // closure may be called more than once.  The last invocation is always
        // the one whose result was actually stored, so we capture that Arc and
        // return a clone of it.
        let last_new: Cell<Option<Arc<T>>> = Cell::new(None);
        self.inner.rcu(|current| {
            let new_arc = Arc::new(f(current.as_ref()));
            last_new.set(Some(Arc::clone(&new_arc)));
            new_arc
        });
        match last_new.into_inner() {
            Some(arc) => (*arc).clone(),
            None => unreachable!("rcu always invokes the closure at least once"),
        }
    }

    /// Gets a clone of the current value.
    ///
    /// This is a convenience method that loads the value and clones it.
    ///
    /// # Returns
    ///
    /// A clone of the current value.
    ///
    /// # Example
    ///
    /// ```rust
    /// use litellm_rs::utils::sync::AtomicValue;
    ///
    /// let value: AtomicValue<String> = AtomicValue::new("hello".to_string());
    /// let cloned: String = value.get();
    /// assert_eq!(cloned, "hello");
    /// ```
    pub fn get(&self) -> T {
        (*self.load()).clone()
    }
}

impl<T: Default> Default for AtomicValue<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> Clone for AtomicValue<T> {
    /// Creates a new `AtomicValue` pointing to the same underlying data.
    ///
    /// Note: This creates a new `AtomicValue` that shares the same `Arc`,
    /// but updates to one will not affect the other after the clone.
    fn clone(&self) -> Self {
        Self {
            inner: ArcSwap::new(self.inner.load_full()),
        }
    }
}

impl<T> From<T> for AtomicValue<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T> From<Arc<T>> for AtomicValue<T> {
    fn from(value: Arc<T>) -> Self {
        Self {
            inner: ArcSwap::new(value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_new_and_load() {
        let value: AtomicValue<i32> = AtomicValue::new(42);
        assert_eq!(*value.load(), 42);
    }

    #[test]
    fn test_store() {
        let value: AtomicValue<i32> = AtomicValue::new(42);
        value.store(100);
        assert_eq!(*value.load(), 100);
    }

    #[test]
    fn test_swap() {
        let value: AtomicValue<i32> = AtomicValue::new(42);
        let old = value.swap(100);
        assert_eq!(*old, 42);
        assert_eq!(*value.load(), 100);
    }

    #[test]
    fn test_store_arc() {
        let value: AtomicValue<i32> = AtomicValue::new(42);
        value.store_arc(Arc::new(100));
        assert_eq!(*value.load(), 100);
    }

    #[test]
    fn test_swap_arc() {
        let value: AtomicValue<i32> = AtomicValue::new(42);
        let old = value.swap_arc(Arc::new(100));
        assert_eq!(*old, 42);
        assert_eq!(*value.load(), 100);
    }

    #[test]
    fn test_update() {
        let value: AtomicValue<i32> = AtomicValue::new(42);
        let new_value = value.update(|v| v + 1);
        assert_eq!(new_value, 43);
        assert_eq!(*value.load(), 43);
    }

    #[test]
    fn test_get() {
        let value: AtomicValue<String> = AtomicValue::new("hello".to_string());
        let cloned: String = value.get();
        assert_eq!(cloned, "hello");
    }

    #[test]
    fn test_default() {
        let value: AtomicValue<i32> = AtomicValue::default();
        assert_eq!(*value.load(), 0);

        let value: AtomicValue<String> = AtomicValue::default();
        assert_eq!(*value.load(), "");
    }

    #[test]
    fn test_clone() {
        let value1: AtomicValue<i32> = AtomicValue::new(42);
        let value2 = value1.clone();

        // Both start with the same value
        assert_eq!(*value1.load(), 42);
        assert_eq!(*value2.load(), 42);

        // Updates to one don't affect the other
        value1.store(100);
        assert_eq!(*value1.load(), 100);
        assert_eq!(*value2.load(), 42);
    }

    #[test]
    fn test_from_value() {
        let value: AtomicValue<i32> = AtomicValue::from(42);
        assert_eq!(*value.load(), 42);
    }

    #[test]
    fn test_from_arc() {
        let arc = Arc::new(42);
        let value: AtomicValue<i32> = AtomicValue::from(arc);
        assert_eq!(*value.load(), 42);
    }

    #[test]
    fn test_with_string() {
        let value: AtomicValue<String> = AtomicValue::new("initial".to_string());
        assert_eq!(value.load().as_ref(), "initial");

        value.store("updated".to_string());
        assert_eq!(value.load().as_ref(), "updated");
    }

    #[test]
    fn test_with_struct() {
        #[derive(Debug, Clone, PartialEq)]
        struct Config {
            host: String,
            port: u16,
        }

        let config = Config {
            host: "localhost".to_string(),
            port: 8080,
        };

        let value: AtomicValue<Config> = AtomicValue::new(config);
        assert_eq!(value.load().host, "localhost");
        assert_eq!(value.load().port, 8080);

        value.store(Config {
            host: "0.0.0.0".to_string(),
            port: 9090,
        });
        assert_eq!(value.load().host, "0.0.0.0");
        assert_eq!(value.load().port, 9090);
    }

    #[test]
    fn test_concurrent_reads() {
        let value: Arc<AtomicValue<i32>> = Arc::new(AtomicValue::new(42));
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let value = Arc::clone(&value);
                thread::spawn(move || {
                    for _ in 0..1000 {
                        let _ = value.load();
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(*value.load(), 42);
    }

    #[test]
    fn test_concurrent_writes() {
        let value: Arc<AtomicValue<i32>> = Arc::new(AtomicValue::new(0));
        let handles: Vec<_> = (0..10)
            .map(|i| {
                let value = Arc::clone(&value);
                thread::spawn(move || {
                    for _ in 0..100 {
                        value.store(i);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // Value should be one of the written values (0-9)
        let final_value = *value.load();
        assert!((0..10).contains(&final_value));
    }

    #[test]
    fn test_concurrent_updates() {
        let value: Arc<AtomicValue<i32>> = Arc::new(AtomicValue::new(0));
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let value = Arc::clone(&value);
                thread::spawn(move || {
                    for _ in 0..100 {
                        value.update(|v| v + 1);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // update() uses rcu() for atomic compare-and-swap retry, so all 1000
        // increments must be visible — no updates are lost under concurrency.
        let final_value = *value.load();
        assert_eq!(final_value, 1000);
    }
}
