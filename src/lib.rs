#[cfg(unix)]
pub mod file_slice;

#[cfg(feature = "fs")]
pub mod fs;

#[cfg(feature = "io")]
pub mod io;

#[cfg(feature = "task")]
pub mod task;

#[cfg(feature = "timer")]
pub mod timer;

#[cfg(any(test, feature = "fixture"))]
mod test_util;

#[cfg(any(test, feature = "fixture"))]
pub use fluvio_test_derive::test_async;

#[cfg(all(unix, feature = "zero_copy"))]
pub mod zero_copy;

#[cfg(feature = "net")]
pub mod net;

#[cfg(feature = "tls")]
#[cfg(unix)]
pub mod tls;

#[cfg(feature = "native2_tls")]
#[cfg(unix)]
pub mod native_tls;

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
