use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "tls")] {
        mod rust_tls;
        pub use rust_tls::*;
    } else if #[cfg(feature  = "native2_tls")] {
        mod native_tls;
        pub use native_tls::*;
    } 
}

