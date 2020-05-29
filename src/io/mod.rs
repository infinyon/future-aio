

#[cfg(feature = "asyncstd")]
pub use inner::*;

#[cfg(feature = "asyncstd")]
mod inner {
    pub use async_std::io::*;
    pub use async_std::prelude::*;
    pub use futures::io::AsyncWrite;
    pub use futures::io::AsyncReadExt;
    pub use futures::io::AsyncWriteExt;
    pub use futures::io::AsyncSeekExt;
    pub use futures::io::AsyncBufReadExt;
}


#[cfg(feature = "asyncstd")]   
mod async_prelude {
    pub use async_std::io::prelude::SeekExt as AsyncSeekExt;
}

#[cfg(feature = "tokio2")]
pub use tokio::io::*;

