mod decoder;
mod encoder;

pub use decoder::{decode_request_body, decode_request_headers, decode_response};
pub use encoder::{encode_request, encode_response};

/// Capacity for buffers.
///
/// Should be significantly greater than BufWriter capacity to avoid flush in the `copy` method.
pub(super) const BUFFER_CAPACITY: usize = 16 * 1024;
