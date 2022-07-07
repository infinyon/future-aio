use async_trait::async_trait;
use futures_lite::FutureExt as lite_ext;
use futures_util::FutureExt;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::future::Future;
use std::time::Duration;
use tracing::warn;

use crate::timer::sleep;
pub use delay::ExponentialBackoff;
pub use delay::FibonacciBackoff;
pub use delay::FixedDelay;

/// An extension trait for `Future` that provides a convenient methods for retries.
#[async_trait]
pub trait RetryExt: Future {
    /// Transforms the current `Future` to a new one that is time-limited by the given timeout.
    /// Returns [TimeoutError] if timeout exceeds. Otherwise, it returns the original Future’s result.
    /// Example:
    /// ```
    /// use fluvio_future::timer::sleep;
    /// use fluvio_future::retry::RetryExt;
    /// use fluvio_future::retry::TimeoutError;
    /// use std::time::{Duration, Instant};
    ///
    /// fluvio_future::task::run(async {
    ///     let result = sleep(Duration::from_secs(10)).timeout(Duration::from_secs(1)).await;
    ///     assert_eq!(result, Err(TimeoutError));
    /// });
    /// ```
    async fn timeout(self, timeout: Duration) -> Result<Self::Output, TimeoutError>;
}

#[async_trait]
impl<F: Future + Send> RetryExt for F {
    async fn timeout(self, timeout: Duration) -> Result<Self::Output, TimeoutError> {
        self.map(Ok)
            .or(async move {
                let _ = sleep(timeout).await;
                Err(TimeoutError)
            })
            .await
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TimeoutError;

impl Display for TimeoutError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for TimeoutError {}

/// Awaits on `Future`. Of Ok(_) or retry condition passes, returns from the function, otherwise returns
/// error to the outer scope.
macro_rules! poll_err {
    ($function:ident, $condition:ident) => {{
        match $function().await {
            Ok(output) => return Ok(output),
            Err(err) if !$condition(&err) => return Err(err),
            Err(err) => err,
        }
    }};
}

/// Provides `Future` with specified retries strategy. See [retry_if] for details.
pub fn retry<I, O, F, E, A>(retries: I, factory: A) -> impl Future<Output = Result<O, E>>
where
    I: IntoIterator<Item = Duration>,
    A: FnMut() -> F,
    F: Future<Output = Result<O, E>>,
    E: Debug,
{
    retry_if(retries, factory, |_| true)
}

/// Provides retry functionality in async context. The `Future` that you want to retry needs to be
/// represented in `FnMut() -> Future` structure. Each retry creates a new instance of `Future` and
/// awaits it. Iterator `Iterator<Item=Duration>` controls the number of retries and delays between
/// them. If iterator returns None, retries stop. There are three common implementations of retry
/// strategies: [FixedDelay], [FibonacciBackoff], and [ExponentialBackoff].
///
/// Example:
/// ```
/// use std::io::{Error, ErrorKind};
/// use std::ops::AddAssign;
/// use std::time::Duration;
/// use fluvio_future::retry::FixedDelay;
/// use fluvio_future::retry::retry;
///
/// fluvio_future::task::run(async {
///     let mut attempts = 0u8;
///     let result = retry(FixedDelay::from_millis(100).take(2), || {
///         attempts.add_assign(1);
///         operation()
///     }).await;
///     assert!(matches!(result, Err(err) if err.kind() == ErrorKind::NotFound));
///     assert_eq!(attempts, 3); // first attempt + 2 retries
/// });
///
/// async fn operation() -> Result<(), Error> {
///     Err(Error::from(ErrorKind::NotFound))
/// }
/// ```
pub async fn retry_if<I, O, F, E, A, P>(retries: I, mut factory: A, condition: P) -> Result<O, E>
where
    I: IntoIterator<Item = Duration>,
    A: FnMut() -> F,
    F: Future<Output = Result<O, E>>,
    P: Fn(&E) -> bool,
    E: Debug,
{
    let mut err = poll_err!(factory, condition);
    for delay_duration in retries.into_iter() {
        sleep(delay_duration).await;
        warn!(?err, "retrying");
        err = poll_err!(factory, condition);
    }
    Err(err)
}

mod delay {
    use std::time::Duration;

    /// A retry strategy driven by a fixed interval between retries.
    /// ```
    /// use std::io::Error;
    /// use fluvio_future::retry::{FixedDelay, retry};
    /// use std::time::{Duration, Instant};
    ///
    /// fluvio_future::task::run(async {
    ///     let _: Result<(), Error> = retry(FixedDelay::from_millis(100).take(4), || async {Ok(())}).await; // 4 retries
    /// });
    /// ```
    #[derive(Default, Clone, Debug, Eq, PartialEq)]
    pub struct FixedDelay {
        delay: Duration,
    }

    impl FixedDelay {
        pub fn new(delay: Duration) -> Self {
            Self { delay }
        }

        pub fn from_millis(millis: u64) -> Self {
            Self::new(Duration::from_millis(millis))
        }

        pub fn from_secs(secs: u64) -> Self {
            Self::new(Duration::from_secs(secs))
        }
    }

    impl Iterator for FixedDelay {
        type Item = Duration;

        fn next(&mut self) -> Option<Duration> {
            Some(self.delay)
        }
    }

    /// A retry strategy driven by the fibonacci series of intervals between retires.
    /// ```
    /// use std::io::Error;
    /// use fluvio_future::retry::{FibonacciBackoff, retry};
    /// use std::time::{Duration, Instant};
    ///
    /// fluvio_future::task::run(async {
    ///     let _: Result<(), Error> = retry(FibonacciBackoff::from_millis(100).take(4), || async {Ok(())}).await; // 4 retries
    /// });
    /// ```
    #[derive(Default, Clone, Debug, Eq, PartialEq)]
    pub struct FibonacciBackoff {
        current: Duration,
        next: Duration,
        max_delay: Option<Duration>,
    }

    impl FibonacciBackoff {
        pub fn new(initial_delay: Duration) -> Self {
            Self {
                current: initial_delay,
                next: initial_delay,
                max_delay: None,
            }
        }

        pub fn from_millis(millis: u64) -> Self {
            Self::new(Duration::from_millis(millis))
        }

        pub fn from_secs(secs: u64) -> Self {
            Self::new(Duration::from_secs(secs))
        }

        pub fn max_delay(mut self, max_delay: Duration) -> Self {
            self.max_delay = Some(max_delay);
            self
        }
    }

    impl Iterator for FibonacciBackoff {
        type Item = Duration;

        fn next(&mut self) -> Option<Self::Item> {
            let duration = self.current;
            if let Some(ref max_delay) = self.max_delay {
                if duration > *max_delay {
                    return Some(*max_delay);
                }
            };
            if let Some(next_next) = self.current.checked_add(self.next) {
                self.current = self.next;
                self.next = next_next;
            } else {
                self.current = self.next;
                self.next = Duration::MAX;
            }
            Some(duration)
        }
    }

    /// A retry strategy driven by exponential back-off.
    /// ```
    /// use std::io::Error;
    /// use fluvio_future::retry::{ExponentialBackoff, retry};
    /// use std::time::{Duration, Instant};
    ///
    /// fluvio_future::task::run(async {
    ///     let _: Result<(), Error> = retry(ExponentialBackoff::from_millis(100).take(4), || async {Ok(())}).await; // 4 retries
    /// });
    /// ```
    #[derive(Default, Clone, Debug, Eq, PartialEq)]
    pub struct ExponentialBackoff {
        base_millis: u64,
        current_millis: u64,
        max_delay: Option<Duration>,
    }

    impl ExponentialBackoff {
        pub fn from_millis(millis: u64) -> Self {
            Self {
                base_millis: millis,
                current_millis: millis,
                max_delay: None,
            }
        }

        pub fn max_delay(mut self, max_delay: Duration) -> Self {
            self.max_delay = Some(max_delay);
            self
        }
    }

    impl Iterator for ExponentialBackoff {
        type Item = Duration;

        fn next(&mut self) -> Option<Self::Item> {
            let duration = Duration::from_millis(self.current_millis);
            if let Some(ref max_delay) = self.max_delay {
                if duration > *max_delay {
                    return Some(*max_delay);
                }
            };
            if let Some(next) = self.current_millis.checked_mul(self.base_millis) {
                self.current_millis = next;
            } else {
                self.current_millis = u64::MAX;
            }
            Some(duration)
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;

        #[test]
        fn test_fibonacci_series_starting_at_10() {
            let mut iter = FibonacciBackoff::from_millis(10);
            assert_eq!(iter.next(), Some(Duration::from_millis(10)));
            assert_eq!(iter.next(), Some(Duration::from_millis(10)));
            assert_eq!(iter.next(), Some(Duration::from_millis(20)));
            assert_eq!(iter.next(), Some(Duration::from_millis(30)));
            assert_eq!(iter.next(), Some(Duration::from_millis(50)));
            assert_eq!(iter.next(), Some(Duration::from_millis(80)));
        }

        #[test]
        fn test_fibonacci_saturates_at_maximum_value() {
            let mut iter = FibonacciBackoff::from_millis(u64::MAX);
            assert_eq!(iter.next(), Some(Duration::from_millis(u64::MAX)));
            assert_eq!(iter.next(), Some(Duration::from_millis(u64::MAX)));
        }

        #[test]
        fn test_fibonacci_stops_increasing_at_max_delay() {
            let mut iter = FibonacciBackoff::from_millis(10).max_delay(Duration::from_millis(50));
            assert_eq!(iter.next(), Some(Duration::from_millis(10)));
            assert_eq!(iter.next(), Some(Duration::from_millis(10)));
            assert_eq!(iter.next(), Some(Duration::from_millis(20)));
            assert_eq!(iter.next(), Some(Duration::from_millis(30)));
            assert_eq!(iter.next(), Some(Duration::from_millis(50)));
            assert_eq!(iter.next(), Some(Duration::from_millis(50)));
        }

        #[test]
        fn test_fibonacci_returns_max_when_max_less_than_base() {
            let mut iter = FibonacciBackoff::from_secs(20).max_delay(Duration::from_secs(10));

            assert_eq!(iter.next(), Some(Duration::from_secs(10)));
            assert_eq!(iter.next(), Some(Duration::from_secs(10)));
        }

        #[test]
        fn test_exponential_some_exponential_base_10() {
            let mut s = ExponentialBackoff::from_millis(10);

            assert_eq!(s.next(), Some(Duration::from_millis(10)));
            assert_eq!(s.next(), Some(Duration::from_millis(100)));
            assert_eq!(s.next(), Some(Duration::from_millis(1000)));
        }

        #[test]
        fn test_exponential_some_exponential_base_2() {
            let mut s = ExponentialBackoff::from_millis(2);

            assert_eq!(s.next(), Some(Duration::from_millis(2)));
            assert_eq!(s.next(), Some(Duration::from_millis(4)));
            assert_eq!(s.next(), Some(Duration::from_millis(8)));
        }

        #[test]
        fn test_exponential_saturates_at_maximum_value() {
            let mut s = ExponentialBackoff::from_millis(u64::MAX - 1);

            assert_eq!(s.next(), Some(Duration::from_millis(u64::MAX - 1)));
            assert_eq!(s.next(), Some(Duration::from_millis(u64::MAX)));
            assert_eq!(s.next(), Some(Duration::from_millis(u64::MAX)));
        }

        #[test]
        fn test_exponential_stops_increasing_at_max_delay() {
            let mut s = ExponentialBackoff::from_millis(2).max_delay(Duration::from_millis(4));

            assert_eq!(s.next(), Some(Duration::from_millis(2)));
            assert_eq!(s.next(), Some(Duration::from_millis(4)));
            assert_eq!(s.next(), Some(Duration::from_millis(4)));
        }

        #[test]
        fn test_exponential_max_when_max_less_than_base() {
            let mut s = ExponentialBackoff::from_millis(20).max_delay(Duration::from_millis(10));

            assert_eq!(s.next(), Some(Duration::from_millis(10)));
            assert_eq!(s.next(), Some(Duration::from_millis(10)));
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::ErrorKind;
    use std::ops::AddAssign;
    use std::time::Duration;
    use tracing::debug;

    #[fluvio_future::test]
    async fn test_fixed_retries_no_delay() {
        let mut executed_retries = 0u8;
        let operation = || {
            let i = executed_retries;
            executed_retries.add_assign(1);
            async move {
                debug!("called retry#{}", i);

                Result::<usize, std::io::Error>::Err(std::io::Error::from(ErrorKind::NotFound))
            }
        };
        let retry_result = retry(FixedDelay::default().take(2), operation).await;
        assert!(matches!(retry_result, Err(err) if err.kind() == ErrorKind::NotFound));
        assert_eq!(executed_retries, 3);
    }

    #[fluvio_future::test]
    async fn test_fixed_retries_timeout() {
        let mut executed_retries = 0u8;
        let operation = || {
            let i = executed_retries;
            executed_retries.add_assign(1);
            async move {
                debug!("called retry#{}", i);
                Result::<usize, std::io::Error>::Err(std::io::Error::from(ErrorKind::NotFound))
            }
        };
        let retry_result = retry(FixedDelay::from_millis(100).take(10), operation)
            .timeout(Duration::from_millis(300))
            .await;

        assert!(matches!(retry_result, Err(_)));
        assert!(executed_retries < 10);
    }

    #[fluvio_future::test]
    async fn test_fixed_retries_not_retryable() {
        let mut executed_retries = 0u8;
        let operation = || {
            let i = executed_retries;
            executed_retries.add_assign(1);
            async move {
                debug!("called retry#{}", i);
                Result::<usize, std::io::Error>::Err(std::io::Error::from(ErrorKind::NotFound))
            }
        };
        let retry_result =
            retry_if(FixedDelay::from_millis(100).take(10), operation, |_| false).await;

        assert!(matches!(retry_result, Err(err) if err.kind() == ErrorKind::NotFound));
        assert_eq!(executed_retries, 1);
    }

    #[fluvio_future::test]
    async fn test_conditional_retry() {
        let mut executed_retries = 0u8;
        let operation = || {
            executed_retries.add_assign(1);
            let i = executed_retries;
            async move {
                debug!("called retry#{}", i);
                if i < 2 {
                    Result::<usize, std::io::Error>::Err(std::io::Error::from(ErrorKind::NotFound))
                } else {
                    Result::<usize, std::io::Error>::Err(std::io::Error::from(
                        ErrorKind::AddrNotAvailable,
                    ))
                }
            }
        };
        let condition = |err: &std::io::Error| err.kind() == ErrorKind::NotFound;
        let retry_result = retry_if(FixedDelay::default().take(10), operation, condition).await;

        assert!(matches!(retry_result, Err(err) if err.kind() == ErrorKind::AddrNotAvailable));
        assert_eq!(executed_retries, 2);
    }
}
