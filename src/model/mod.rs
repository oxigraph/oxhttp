//! The HTTP model encoded in Rust type system.
//!
//! This reexport the [`http`](https://docs.rs/http) crate except for [`Body`].
//!
//! The main entry points are [`Request`] and [`Response`].
mod body;

pub use body::{Body, ChunkedTransferPayload};
pub use http::*;
