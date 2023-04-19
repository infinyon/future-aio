use std::fmt::Display;
use std::sync::atomic::{AtomicBool, Ordering};

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use log::info;
use tracing::{debug, error};

use crate::sync::Mutex;
#[deprecated(
    since = "0.5.1",
    note = "please use `fluvio_future::task::JoinHandle` instead"
)]
pub use crate::task::JoinHandle;

#[derive(Clone)]
/// DoomsdayTimer will configurably panic or exit if it is not
/// `reset()` at least every `duration`
pub struct DoomsdayTimer {
    time_to_explode: Arc<Mutex<Instant>>,
    duration: Duration,
    defused: Arc<AtomicBool>,
    aggressive_mode: bool,
}

impl Display for DoomsdayTimer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fail_mode = if self.aggressive_mode {
            "Exits"
        } else {
            "Panics"
        };
        write!(
            f,
            "DoomsdayTimer(Duration: {:?}, {})",
            self.duration, fail_mode,
        )
    }
}

impl DoomsdayTimer {
    /// Spawn a new doomsday timer.
    /// If `exit_on_explode` is true, it will terminate process with `exit(1)` if it explodes.
    /// Otherwise it will call `panic()`. Note that `awaiting` on the jh will panic if the `DoomsdayTimer` panicked
    pub fn spawn(duration: Duration, exit_on_explode: bool) -> (Self, JoinHandle<()>) {
        let s = Self {
            time_to_explode: Arc::new(Mutex::new(Instant::now() + duration)),
            duration,
            defused: Default::default(),
            aggressive_mode: exit_on_explode,
        };

        let cloned = s.clone();
        let jh = crate::task::spawn(async move {
            cloned.main_loop().await;
        });
        (s, jh)
    }

    /// Reset the timer to it's full duration
    pub async fn reset(&self) {
        let new_time_to_explode = Instant::now() + self.duration;
        *self.time_to_explode.lock().await = new_time_to_explode;
        debug!("{} has been reset", self);
    }

    async fn main_loop(&self) {
        loop {
            if self.defused.load(Ordering::Relaxed) {
                debug!("{} has been defused, terminating main loop", self);
                return;
            }
            let now = Instant::now();
            let time_to_explode = *self.time_to_explode.lock().await;
            if now > time_to_explode {
                error!("{} exploded due to timeout", self);
                self.explode_inner();
            } else {
                let time_to_sleep = time_to_explode - now;
                crate::timer::sleep(time_to_sleep).await;
            }
        }
    }

    /// Force the timer to explode
    pub fn explode(&self) {
        error!("{} was exploded manually", self);
        self.explode_inner();
    }

    fn explode_inner(&self) {
        if self.aggressive_mode {
            error!("{} exiting", self);
            std::process::exit(1);
        } else {
            error!("{} panicking", self);
            panic!("DoomsdayTimer with Duration {:?} exploded", self.duration);
        }
    }
    /// Defuse the timer. Cannot be undone and will no longer `explode`
    pub fn defuse(&self) {
        self.defused.store(true, Ordering::Relaxed);
        info!("{} has been defused", self);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::DoomsdayTimer;
    use crate::task::run_block_on;
    use crate::test_async;
    use std::io::Error;

    #[test_async(should_panic)]
    async fn test_explode() -> Result<(), Error> {
        let (_, jh) = DoomsdayTimer::spawn(Duration::from_millis(1), false);
        crate::timer::sleep(Duration::from_millis(2)).await;
        jh.await;
        Ok(())
    }

    #[test_async]
    async fn test_do_not_explode() -> Result<(), Error> {
        let (bomb, jh) = DoomsdayTimer::spawn(Duration::from_millis(10), false);
        crate::timer::sleep(Duration::from_millis(5)).await;
        bomb.reset().await;
        crate::timer::sleep(Duration::from_millis(5)).await;
        bomb.reset().await;
        crate::timer::sleep(Duration::from_millis(5)).await;
        bomb.defuse();
        run_block_on(jh);
        Ok(())
    }
}
