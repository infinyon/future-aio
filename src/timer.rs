pub use inner::*;

#[cfg(not(target_arch = "wasm32"))]
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
            if this.0.poll(cx).is_ready() {
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
#[cfg(target_arch = "wasm32")]
mod inner {
    use std::time::Duration;
    // TODO: when https://github.com/tomaka/wasm-timer/pull/13 is merged, move this back to
    // wasm-timer
    use fluvio_wasm_timer::Delay;
    pub fn sleep(duration: Duration) -> Delay {
        Delay::new(duration)
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod test {

    use std::time::Duration;
    use std::time::Instant;

    use log::debug;
    use tokio::select;

    use fluvio_future::timer::sleep;

    /// test timer loop
    #[fluvio_future::test]
    async fn test_sleep() {
        let mut times_fired: u16 = 0;
        let mut times_not_fired: u16 = 0;
        let time_now = Instant::now();

        let mut sleep_ft = sleep(Duration::from_millis(10));

        for _ in 0u16..10u16 {
            select! {
                _ = &mut sleep_ft => {
                    // fire everytime but won't make cause more delay than initial 10 ms
                    times_fired += 1;
                    debug!("timer fired");
                }

                _ = sleep(Duration::from_millis(40)) => {
                    times_not_fired += 1;
                }
            }
        }

        let elapsed = time_now.elapsed();

        debug!("total time elaspsed: {:#?}", elapsed);

        assert!(elapsed < Duration::from_millis(1000)); // make this generous to handle slow CI
        assert!(elapsed > Duration::from_millis(10));
        assert_eq!(times_fired, 1);
        assert_eq!(times_not_fired, 9);
    }
}
