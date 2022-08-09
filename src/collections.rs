use async_std::channel::{Receiver, Sender};
use async_std::sync::Mutex;
use std::collections::VecDeque;
use std::ops::Deref;
use tracing::error;

/// Blocking async double-ended queue with interior mutability.
pub struct BlockingDeque<T> {
    queue: Mutex<VecDeque<T>>,
    control: (Sender<()>, Receiver<()>),
}

impl<T> BlockingDeque<T> {
    /// Creates a new queue.
    ///
    /// The created queue has space to hold at most `cap` records at a time.
    ///
    /// # Panics
    ///
    /// Capacity must be a positive number. If `cap` is zero, this function will panic.
    pub fn new(cap: usize) -> Self {
        let queue = Mutex::new(VecDeque::new());
        let control = async_std::channel::bounded(cap);
        Self { queue, control }
    }

    /// Appends an element to the back of the deque.
    ///
    /// If the queue is full, this method waits until there is space for a value.
    ///
    /// # Examples
    ///
    /// ```
    /// use fluvio_future::collections::BlockingDeque;
    /// fluvio_future::task::run(async {
    ///     let queue = BlockingDeque::new(2);
    ///     assert!(queue.is_empty());
    ///     queue.push_back(1).await;
    ///     queue.push_back(1).await;
    ///     assert_eq!(queue.len(), 2);
    /// });
    /// ```
    pub async fn push_back(&self, value: T) {
        let _ = self.control.0.send(()).await; // drop the error because we own both sides
        self.queue.lock().await.push_back(value);
    }

    /// Removes the first element and returns it, or `None` if the deque is
    /// empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use fluvio_future::collections::BlockingDeque;
    /// fluvio_future::task::run(async {
    ///     let queue = BlockingDeque::new(1);
    ///     assert_eq!(queue.pop_front().await, None);
    ///     queue.push_back(1).await;
    ///     assert_eq!(queue.pop_front().await, Some(1));
    /// });
    /// ```
    pub async fn pop_front(&self) -> Option<T> {
        let front = self.queue.lock().await.pop_front()?;
        if let Err(err) = self.control.1.try_recv() {
            error!(
                "control channel is in corrupted state. expected an value, got: {}",
                err
            )
        }
        Some(front)
    }

    /// Returns `true` if the queue is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use fluvio_future::collections::BlockingDeque;
    /// fluvio_future::task::run(async {
    ///     let queue = BlockingDeque::new(1);
    ///     assert!(queue.is_empty());
    ///     queue.push_back(1).await;
    ///     assert!(!queue.is_empty());
    /// });
    /// ```
    pub fn is_empty(&self) -> bool {
        self.control.1.is_empty()
    }

    /// Returns the number of records in the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use fluvio_future::collections::BlockingDeque;
    /// fluvio_future::task::run(async {
    ///     let queue = BlockingDeque::new(2);
    ///     assert_eq!(queue.len(), 0);
    ///     queue.push_back(1).await;
    ///     queue.push_back(1).await;
    ///     assert_eq!(queue.len(), 2);
    /// });
    /// ```
    pub fn len(&self) -> usize {
        self.control.1.len()
    }

    /// Provides a shared reference to the internal queue for the `consumer`.
    ///
    /// # Examples
    ///
    /// ```
    /// use fluvio_future::collections::BlockingDeque;
    /// fluvio_future::task::run(async {
    ///     let queue = BlockingDeque::new(1);
    ///     queue.push_back(1).await;
    ///     
    ///     assert!(queue.inspect(|q| q.front().is_some()).await);
    /// });
    /// ```
    pub async fn inspect<R, F>(&self, consumer: F) -> R
    where
        F: Fn(&VecDeque<T>) -> R,
    {
        consumer(self.queue.lock().await.deref())
    }

    /// Provides an exclusive mutable reference to the last element in the queue for the `consumer`.
    ///
    /// # Examples
    ///
    /// ```
    /// use fluvio_future::collections::BlockingDeque;
    /// fluvio_future::task::run(async {
    ///     let queue = BlockingDeque::new(1);
    ///     queue.push_back(1).await;
    ///     queue.inspect_back_mut(|back| *(back.unwrap()) = 2).await;
    ///     
    ///     assert_eq!(queue.pop_front().await, Some(2));
    /// });
    /// ```
    pub async fn inspect_back_mut<R, F>(&self, consumer: F) -> R
    where
        F: Fn(Option<&mut T>) -> R,
    {
        consumer(self.queue.lock().await.back_mut())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_std::task::{sleep, spawn};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    #[fluvio_future::test]
    async fn test_order_in_bound() {
        //given
        let input = [0, 1, 2, 3, 4, 5];
        let queue = BlockingDeque::new(input.len());

        //when
        for i in input {
            queue.push_back(i).await;
            assert_eq!(queue.len(), i + 1);
        }

        let mut result = Vec::new();
        while let Some(v) = queue.pop_front().await {
            result.push(v);
        }
        //then
        assert_eq!(input.as_slice(), result.as_slice());
    }

    #[fluvio_future::test]
    async fn test_blocking_in_bound() {
        //given
        let queue = BlockingDeque::new(2);
        queue.push_back(1).await;
        queue.push_back(2).await;
        let shared = Arc::new(queue);
        let shared1 = shared.clone();
        spawn(async move {
            sleep(Duration::from_millis(300)).await;
            let _ = shared1.pop_front().await;
            let _ = shared1.pop_front().await;
        });

        //when
        let started = Instant::now();
        shared.push_back(3).await;
        shared.push_back(4).await;

        //then
        assert!(started.elapsed() >= Duration::from_millis(300));
    }

    #[fluvio_future::test]
    async fn test_inspect() {
        //given
        let input = [0, 1, 2, 3, 4, 5];
        let queue = BlockingDeque::new(10);

        //when
        assert!(!queue.inspect(|q| q.front().is_some()).await);
        assert!(queue.is_empty());
        for i in input {
            queue.push_back(i).await;
            assert!(queue.inspect(|q| q.front().unwrap() == &0).await);
        }

        for i in input {
            assert!(queue.inspect(|q| q.front().unwrap() == &i).await);
            assert_eq!(queue.pop_front().await.unwrap(), i);
        }

        //then
        assert!(queue.is_empty());
    }

    #[fluvio_future::test]
    async fn test_inspect_back_mut() {
        //given
        let queue = BlockingDeque::new(1);
        queue.push_back(1).await;

        //when
        queue.inspect_back_mut(|back| *(back.unwrap()) = 2).await;

        //then
        assert_eq!(queue.pop_front().await, Some(2));
        assert!(queue.is_empty());
    }
}
