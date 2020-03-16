
#[cfg(feature = "asyncstd")]
mod inner_sync {
    pub use async_std::sync::Arc;
    pub use async_std::sync::Barrier;
    pub use async_std::sync::BarrierWaitResult;
    pub use async_std::sync::Mutex;
    pub use async_std::sync::MutexGuard;
    pub use async_std::sync::RwLock;
    pub use async_std::sync::RwLockReadGuard;
    pub use async_std::sync::RwLockWriteGuard;
    pub use async_std::sync::Weak;
}
#[cfg(feature = "asyncstd")]
pub use inner_sync::*;

pub mod mpsc {

    #[cfg(feature = "asyncstd")]
    pub use async_std::sync::Receiver;
    pub use async_std::sync::Sender;
    pub use async_std::sync::channel;


    #[cfg(feature = "tokio2")]
    pub use tokio::sync::broadcast::*;
}


pub use inner::Channel;

mod inner {

    use super::mpsc::Receiver;
    use super::mpsc::Sender;
    use super::mpsc::channel;

    /// abstraction for multi sender receiver channel
    #[derive(Debug)]
    pub struct Channel<T> {
        receiver: Receiver<T>,
        sender: Sender<T>
    }

    impl <T>Channel<T> {

        pub fn new(capacity: usize) -> Self {

            let (sender,receiver) = channel(capacity);
            Self {
                receiver,
                sender
            }
        }

        /// create new clone of sender
        pub fn sender(&self) -> Sender<T> {
            self.sender.clone()
        }

        #[cfg(feature = "asyncstd")]
        pub fn receiver(&self) -> Receiver<T> {
            self.receiver.clone()
        }

        #[cfg(feature = "tokio2")]
        pub fn receiver(&self) -> Receiver<T> {
            self.sender.subscribe()
        }
    }

}
