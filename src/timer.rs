pub use inner::*;

mod inner {

    use std::time::Duration;

    use async_io::Timer;

    pub async fn sleep(duration: Duration)  {
        Timer::after(duration).await;
    }
}


