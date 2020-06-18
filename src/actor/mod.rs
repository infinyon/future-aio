#[cfg(feature = "unstable")]
pub use dispatcher::*;

#[cfg(feature = "unstable")]
mod dispatcher {
    use crate::task::spawn;

    use async_trait::async_trait;

    /// Simple async dispatcher who perform i/o dispatch in separate async task
    /// 
    /// Once dispatcher goes into spawn, it can't be reached directly.
    /// Dispatcher should own async construct like channel or other signal mechanism to 
    /// receive events
    #[async_trait]
    pub trait AsyncDispatcher: Sized + Send + 'static {
        
        fn run(self) {
            let ft = async move {
                self.dispatch_loop().await;
            };

            spawn(ft);
        }

        /// perform dispatch loop
        /// this is where dispatcher should perform select
        async fn dispatch_loop(mut self);
    }



    #[cfg(test)]
    mod test {

        use std::time::Duration;
        
        use async_lock::Lock;
        use async_trait::async_trait;

        use crate::test_async;
        use crate::timer::sleep;

        use super::*;

        struct SimpleDispatcher {
            count: Lock<u16>,
            delay:  Duration
        }

        #[async_trait]
        impl AsyncDispatcher for SimpleDispatcher {

            async fn dispatch_loop(mut self) {

                use crate::timer::sleep;

                let guard = self.count.lock().await;
                let count = *guard;
                drop(guard);
                for _ in 0..count {
                    let mut guard = self.count.lock().await;
                    *guard = *guard - 1;
                    sleep(self.delay).await;
                }

            }

        }


        #[test_async]
        async fn test_dispatcher() -> Result<(),()> {

            let count = Lock::new(5);
            let dispatcher = SimpleDispatcher { count: count.clone(), delay: Duration::from_micros(10) };
            dispatcher.run();
            // wait for dispatcher to finish, count should be zero
            sleep(Duration::from_millis(5)).await;
            let guard = count.lock().await;
            assert_eq!(*guard,0);
            Ok(())
        }
    }

}


