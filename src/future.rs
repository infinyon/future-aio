use crate::timer;
use anyhow::Result;
use std::time;

pub use futures_lite::future::*;

pub async fn timeout<F, T>(duration: time::Duration, future: F) -> Result<T>
where
    F: Future<Output = T>,
    T: Send + 'static,
{
    race(async move { Ok(future.await) }, async move {
        let _ = timer::sleep(duration).await;
        Err(anyhow::anyhow!("Future timed out after {:?}", duration))
    })
    .await
}
