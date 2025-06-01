use std::time;

use anyhow::Result;

use crate::timer;

pub use futures_lite::future::*;

/// If the future completes before the duration has elapsed, then the completed value is returned.
/// Otherwise, an error is returned and the future is canceled.
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
