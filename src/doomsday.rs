use async_std::sync::Mutex;
use async_std::task::JoinHandle;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tracing::error;

#[derive(Clone)]
pub struct DoomsdayTimer {
    time_to_explode: Arc<Mutex<Instant>>,
    duration: Duration,
    aggressive_mode: bool,
}

impl DoomsdayTimer {
    pub fn spawn_with_panic(duration: Duration) -> (Self, JoinHandle<()>) {
        let s = Self {
            time_to_explode: Arc::new(Mutex::new(Instant::now() + duration)),
            duration,
            aggressive_mode: false,
        };

        let cloned = s.clone();
        let jh = async_std::task::spawn(async move {
            cloned.main_loop().await;
        });
        (s, jh)
    }

    pub fn spawn_with_exit(duration: Duration) -> Self {
        let s = Self {
            time_to_explode: Arc::new(Mutex::new(Instant::now() + duration)),
            duration,
            aggressive_mode: false,
        };

        let cloned = s.clone();
        async_std::task::spawn(async move {
            cloned.main_loop().await;
        });
        s
    }

    pub async fn reset(&self) {
        let new_time_to_explode = Instant::now() + self.duration;
        *self.time_to_explode.lock().await = new_time_to_explode;
    }

    async fn main_loop(&self) {
        loop {
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

    pub fn explode(&self) {
        error!("Boom. DoomsdayTimer has exploded");
        if self.aggressive_mode {
            std::process::exit(1);
        } else {
            panic!("DoomsdayTimer with Duration {:?} exploded", self.duration);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::DoomsdayTimer;

    #[test]
    #[should_panic]
    fn test_explode() {
        let (_, jh) = DoomsdayTimer::spawn_with_panic(Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(2));
        async_std::task::block_on(jh)
    }
}
