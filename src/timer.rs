pub use inner::*;

mod inner {

    use std::time::{Duration, Instant};

    use async_io::Timer;
    use futures_lite::future::Future;

    /// wait for until `duration` has elapsed.
    ///
    /// this effectively give back control to async execution engine until duration is finished
    ///
    /// # Examples
    ///
    /// ```
    /// use fluvio_future::timer::sleep;
    /// use std::time::{Duration, Instant};
    ///
    /// fluvio_future::task::run(async {
    ///     sleep(Duration::from_secs(1)).await;
    /// });
    /// ```
    pub fn sleep(duration: Duration) -> impl Future<Output = Instant> {
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
        assert!(elapsed < Duration::from_millis(20));
        assert_eq!(sleep_count, 10);

        Ok(())
    }
}
