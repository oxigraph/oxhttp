//! OxHTTP is a very simple synchronous implementation of [HTTP 1.1](https://httpwg.org/http-core/) [`Client`] and [`Server`].
//!
//! Opposite to most of the existing Rust HTTP library OxHTTP does not depend on an async runtime.
//!
//! The client documentation is provided by the [`Client`] struct and the server documentation by the [`Server`] struct.
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

mod client;
mod io;
pub mod model;
mod server;
mod utils;

pub use client::Client;
pub use server::Server;
