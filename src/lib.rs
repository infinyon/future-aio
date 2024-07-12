#[cfg(unix)]
pub mod file_slice;

#[cfg(feature = "fs")]
#[cfg(not(target_arch = "wasm32"))]
pub mod fs;

#[cfg(feature = "io")]
#[cfg(not(target_arch = "wasm32"))]
pub mod io;

#[cfg(feature = "task")]
pub mod task;

#[cfg(feature = "timer")]
pub mod timer;

#[cfg(feature = "retry")]
pub mod retry;

#[cfg(any(test, feature = "fixture"))]
mod test_util;

#[cfg(any(test, feature = "fixture"))]
pub use fluvio_future_derive::test_async;

#[cfg(any(test, feature = "fixture"))]
pub use fluvio_future_derive::test;

#[cfg(all(unix, feature = "zero_copy"))]
pub mod zero_copy;

#[cfg(feature = "net")]
pub mod net;

#[cfg(all(any(unix, windows), feature = "rust_tls"))]
pub mod rust_tls;

#[cfg(all(any(unix, windows), feature = "rust_tls", not(feature = "native_tls")))]
pub use rust_tls as tls;

#[cfg(all(any(unix, windows), feature = "native_tls"))]
pub mod native_tls;

#[cfg(all(any(unix, windows), feature = "native_tls", not(feature = "rust_tls")))]
pub use crate::native_tls as tls;

#[cfg(feature = "openssl_tls")]
#[cfg(not(target_arch = "wasm32"))]
pub mod openssl;

#[cfg(feature = "sync")]
pub mod sync;

#[cfg(feature = "future")]
pub mod future;

#[cfg(feature = "subscriber")]
pub mod subscriber {
    use tracing_subscriber::EnvFilter;

    pub fn init_logger() {
        init_tracer(None);
    }

    pub fn init_tracer(level: Option<tracing::Level>) {
        let _ = tracing_subscriber::fmt()
            .with_max_level(level.unwrap_or(tracing::Level::DEBUG))
            .with_env_filter(EnvFilter::from_default_env())
            .with_writer(std::io::stderr)
            .try_init();
    }
}

#[cfg(feature = "doomsday")]
#[cfg(not(target_arch = "wasm32"))]
pub mod doomsday;

#[cfg(feature = "attributes")]
pub use fluvio_future_derive::main_async;

/// re-export tracing
pub mod tracing {

    pub use tracing::*;
}
