#[cfg(unix)]
pub mod file_slice;

#[cfg(all(feature = "fs", not(target_arch = "wasm32")))]
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
pub use fluvio_test_derive::test_async;

#[cfg(any(test, feature = "fixture"))]
pub use fluvio_test_derive::test;

#[cfg(all(unix, feature = "zero_copy"))]
pub mod zero_copy;

#[cfg(feature = "net")]
pub mod net;

#[cfg(all(unix, feature = "rust_tls"))]
pub mod rust_tls;

#[cfg(all(unix, feature = "rust_tls", not(feature = "native2_tls")))]
pub use rust_tls as tls;

#[cfg(all(any(unix, windows), feature = "native2_tls"))]
pub mod native_tls;

#[cfg(all(any(unix, windows), feature = "native2_tls", not(feature = "rust_tls")))]
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
            .try_init();
    }
}

/// re-export tracing
pub mod tracing {

    pub use tracing::*;
}
