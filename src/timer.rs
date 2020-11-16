pub use inner::*;

mod inner {

    use std::pin::Pin;
    use std::task::{Context, Poll};
    use std::time::Duration;

    use async_io::Timer;
    use futures_lite::future::Future;

    use pin_project::pin_project;

    /// same as `after` but return () to make it compatible as previous
    pub fn sleep(duration: Duration) -> Sleeper {
        Sleeper(after(duration))
    }

    #[pin_project]
    pub struct Sleeper(#[pin] Timer);

    impl Future for Sleeper {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let this = self.project();
            if let Poll::Ready(_) = this.0.poll(cx) {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        }
    }

    /// wait for until `duration` has elapsed.
    ///
    /// this effectively give back control to async execution engine until duration is finished
    ///
    /// # Examples
    ///
    /// ```
    /// use fluvio_future::timer::after;
    /// use std::time::{Duration, Instant};
    ///
    /// fluvio_future::task::run(async {
    ///     after(Duration::from_secs(1)).await;
    /// });
    /// ```
    pub fn after(duration: Duration) -> Timer {
        Timer::after(duration)
    }
}

#[cfg(test)]
mod test {

    use std::time::Duration;
    use std::time::Instant;

    use log::debug;
    use tokio::select;

    use fluvio_future::test_async;
    use fluvio_future::timer::sleep;

    /// test timer loop
    #[test_async]
    async fn test_sleep() -> Result<(), ()> {
        let mut sleep_count: u16 = 0;
        let time_now = Instant::now();

        let mut sleep_ft = sleep(Duration::from_millis(10));

        for _ in 0u16..10u16 {
            select! {
                _ = &mut sleep_ft => {
                    // fire everytime but won't make cause more delay than initial 10 ms
                    sleep_count += 1;
                    debug!("timer fired");
                }
            }
        }

        let elapsed = time_now.elapsed();

        debug!("total time elaspsed: {:#?}", elapsed);
        assert!(elapsed < Duration::from_millis(30));
        assert!(elapsed > Duration::from_millis(10));
        assert_eq!(sleep_count, 10);

        Ok(())
    }
}
