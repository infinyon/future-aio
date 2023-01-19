use std::sync::atomic::{AtomicBool, Ordering};

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use async_std::sync::Mutex;
pub use async_std::task::JoinHandle;
use tracing::error;

#[derive(Clone)]
/// DoomsdayTimer will panic (`spawn_with_panic()`) or exit (`spawn_with_exit()`) if it is not
/// `reset()` at least every `duration`
pub struct DoomsdayTimer {
    time_to_explode: Arc<Mutex<Instant>>,
    duration: Duration,
    defused: Arc<AtomicBool>,
    aggressive_mode: bool,
}

impl DoomsdayTimer {
    /// Spawn a new doomsday timer that will `panic()` if it explodes.
    /// `awaiting` on the jh will panic if the `DoomsdayTimer` panicked
    pub fn spawn_with_panic(duration: Duration) -> (Self, JoinHandle<()>) {
        let s = Self {
            time_to_explode: Arc::new(Mutex::new(Instant::now() + duration)),
            duration,
            defused: Default::default(),
            aggressive_mode: false,
        };

        let cloned = s.clone();
        let jh = async_std::task::spawn(async move {
            cloned.main_loop().await;
        });
        (s, jh)
    }

    /// Spawn a new doomsday timer that will `exit(1)` if it explodes.
    /// This will aggressively ensure the process does not survive
    pub fn spawn_with_exit(duration: Duration) -> Self {
        let s = Self {
            time_to_explode: Arc::new(Mutex::new(Instant::now() + duration)),
            duration,
            defused: Default::default(),
            aggressive_mode: false,
        };

        let cloned = s.clone();
        async_std::task::spawn(async move {
            cloned.main_loop().await;
        });
        s
    }

    /// Reset the timer to it's full duration
    pub async fn reset(&self) {
        let new_time_to_explode = Instant::now() + self.duration;
        *self.time_to_explode.lock().await = new_time_to_explode;
    }

    async fn main_loop(&self) {
        loop {
            if self.defused.load(Ordering::Relaxed) {
                return;
            }
            let now = Instant::now();
            let time_to_explode = *self.time_to_explode.lock().await;
            if now > time_to_explode {
                self.explode();
            } else {
                let time_to_sleep = time_to_explode - now;
                async_std::task::sleep(time_to_sleep).await;
            }
        }
    }

    /// Force the timer to explode
    pub fn explode(&self) {
        error!("Boom. DoomsdayTimer has exploded");
        if self.aggressive_mode {
            std::process::exit(1);
        } else {
            panic!("DoomsdayTimer with Duration {:?} exploded", self.duration);
        }
    }

    /// Defuse the timer. Cannot be undone and will no longer `explode`
    pub fn defuse(&self) {
        self.defused.store(true, Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::DoomsdayTimer;
    use crate::test_async;
    use std::io::Error;

    #[test_async(should_panic)]
    async fn test_explode() -> Result<(), Error> {
        let (_, jh) = DoomsdayTimer::spawn_with_panic(Duration::from_millis(1));
        async_std::task::sleep(Duration::from_millis(2)).await;
        jh.await;
        Ok(())
    }

    #[test_async]
    async fn test_do_not_explode() -> Result<(), Error> {
        let (bomb, jh) = DoomsdayTimer::spawn_with_panic(Duration::from_millis(10));
        async_std::task::sleep(Duration::from_millis(5)).await;
        bomb.reset().await;
        async_std::task::sleep(Duration::from_millis(5)).await;
        bomb.reset().await;
        async_std::task::sleep(Duration::from_millis(5)).await;
        bomb.defuse();
        async_std::task::block_on(jh);
        Ok(())
    }
}
