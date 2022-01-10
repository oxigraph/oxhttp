#![doc = include_str!("../README.md")]
#![deny(
    future_incompatible,
    nonstandard_style,
    rust_2018_idioms,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unused_qualifications
)]

#[cfg(all(target_feature = "native-tls", target_feature = "rustls"))]
compile_error!(
    "Both `native-tls` and `rustls` options of oxhttp can't be enabled at the same time"
);

mod client;
mod io;
pub mod model;
mod server;
mod utils;

pub use client::Client;
pub use server::Server;
