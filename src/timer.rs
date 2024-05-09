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
        Sleeper::after(duration)
    }

    pub enum SleeperFired {
        Fired,
        NotFired,
        // used when timer is based on interval
        // or anything that could be fired multiple times
        None,
    }

    #[pin_project]
    pub struct Sleeper {
        #[pin]
        timer: Timer,
        fired: SleeperFired,
    }

    impl Sleeper {
        pub fn after(duration: Duration) -> Self {
            Self {
                timer: after(duration),
                fired: SleeperFired::NotFired,
            }
        }
    }

    impl Future for Sleeper {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let this = self.project();
            if let SleeperFired::Fired = *this.fired {
                Poll::Ready(())
            } else if this.timer.poll(cx).is_ready() {
                if let SleeperFired::NotFired = *this.fired {
                    *this.fired = SleeperFired::Fired;
                }
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

        assert!(elapsed < Duration::from_millis(1000)); // make this generous to handle slow CI
        assert!(elapsed > Duration::from_millis(10));
        assert_eq!(sleep_count, 10);
    }
}
