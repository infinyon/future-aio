mod logger;

pub use logger::init_logger;

#[cfg(feature = "fixture")]
pub mod fixture;
