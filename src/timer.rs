pub use inner::*;


#[cfg(feature = "asyncstd")]
mod inner {

    use std::time::Duration;

    use futures_timer::Delay;

    pub async fn sleep(duration: Duration)  {
        let delay = Delay::new(duration);
        delay.await;
    }
}

#[cfg(feature = "tokio2")]
mod inner {

    use std::time::Duration;
    use tokio::time::delay_for;
    

    pub async fn sleep(duration: Duration)  {
        delay_for(duration).await;
    }
}



