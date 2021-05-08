/// run async expression and assert based on result value
#[macro_export]
macro_rules! assert_async_block {
    ($ft_exp:expr) => {{
        let ft = $ft_exp;

        #[cfg(not(target_arch = "wasm32"))]
        match fluvio_future::task::run_block_on(ft) {
            Ok(_) => log::debug!("finished run"),
            Err(err) => assert!(false, "error {:?}", err),
        }
        #[cfg(target_arch = "wasm32")]
        fluvio_future::task::run_block_on(ft);
    }};
}

#[cfg(test)]
mod test {

    use std::io::Error;
    use std::pin::Pin;
    use std::task::Context;
    use std::task::Poll;

    use futures_lite::future::poll_fn;
    use futures_lite::Future;
    use log::debug;

    use crate::test_async;

    // actual test run

    #[test_async]
    async fn async_derive_test() -> Result<(), Error> {
        debug!("I am live");
        Ok(())
    }

    #[test_async(ignore)]
    async fn async_derive_test_ignore() -> Result<(), Error> {
        debug!("I am live");
        Ok(())
    }

    #[fluvio_future::test]
    async fn simple_test() {
        assert_eq!(1, "x".len());
    }

    #[fluvio_future::test(ignore)]
    async fn simple_test_ignore() {
        assert_eq!(1, "x".len());
    }

    #[test]
    fn test_1_sync_example() {
        async fn test_1() -> Result<(), Error> {
            debug!("works");
            Ok(())
        }

        let ft = async { test_1().await };

        assert_async_block!(ft);
    }

    struct TestFuture {}

    impl Future for TestFuture {
        type Output = u16;

        fn poll(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
            Poll::Ready(2)
        }
    }

    #[test_async]
    async fn test_future() -> Result<(), Error> {
        let t = TestFuture {};
        let v: u16 = t.await;
        assert_eq!(v, 2);
        Ok(())
    }

    fn test_poll(_cx: &mut Context) -> Poll<u16> {
        Poll::Ready(4)
    }

    #[test_async]
    async fn test_future_with_poll() -> Result<(), Error> {
        assert_eq!(poll_fn(test_poll).await, 4);
        Ok(())
    }
}
