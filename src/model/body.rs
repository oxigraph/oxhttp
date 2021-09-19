use crate::model::Headers;
use std::cmp::min;
use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::io::{Cursor, Error, ErrorKind, Read, Result};

/// A request or response [body](https://httpwg.org/http-core/draft-ietf-httpbis-messaging-latest.html#message.body).
///
/// It implements the [`Read`] API.
pub struct Body<'a>(BodyAlt<'a>);

enum BodyAlt<'a> {
    SimpleOwned(Cursor<Vec<u8>>),
    SimpleBorrowed(&'a [u8]),
    Sized {
        content: Box<dyn Read + 'a>,
        total_len: u64,
        consumed_len: u64,
    },
    Chunked(Box<dyn ChunkedTransferPayload + 'a>),
}

impl<'a> Body<'a> {
    /// Creates a new body from a [`Read`] implementation.
    ///
    /// If the body is sent as an HTTP request or response it will be streamed using [chunked transfer encoding](https://httpwg.org/http-core/draft-ietf-httpbis-messaging-latest.html#chunked.encoding).
    pub fn from_read(read: impl Read + 'a) -> Self {
        Self::from_chunked_transfer_payload(SimpleChunkedTransferEncoding(read))
    }

    pub(crate) fn from_read_and_len(read: impl Read + 'a, len: u64) -> Self {
        Self(BodyAlt::Sized {
            total_len: len,
            consumed_len: 0,
            content: Box::new(read.take(len)),
        })
    }

    /// Creates a [chunked transfer encoding](https://httpwg.org/http-core/draft-ietf-httpbis-messaging-latest.html#chunked.encoding) body with optional trailers.
    pub fn from_chunked_transfer_payload(payload: impl ChunkedTransferPayload + 'a) -> Self {
        Self(BodyAlt::Chunked(Box::new(payload)))
    }

    /// The number of bytes in the body (if known).
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> Option<u64> {
        match &self.0 {
            BodyAlt::SimpleOwned(d) => Some(d.get_ref().len().try_into().unwrap()),
            BodyAlt::SimpleBorrowed(d) => Some(d.len().try_into().unwrap()),
            BodyAlt::Sized { total_len, .. } => Some(*total_len),
            BodyAlt::Chunked(_) => None,
        }
    }

    /// Returns the chunked transfer encoding trailers if they exists and are already received.
    /// You should fully consume the body before attempting to fetch them.
    pub fn trailers(&self) -> Option<&Headers> {
        match &self.0 {
            BodyAlt::SimpleOwned(_) | BodyAlt::SimpleBorrowed(_) | BodyAlt::Sized { .. } => None,
            BodyAlt::Chunked(c) => c.trailers(),
        }
    }

    /// Reads the full body into a vector.
    ///
    /// WARNING: Beware of the body size!
    ///
    /// ```
    /// use oxhttp::model::Body;
    /// use std::io::Cursor;
    ///
    /// let mut body = Body::from_read(b"foo".as_ref());
    /// assert_eq!(&body.to_vec()?, b"foo");
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn to_vec(mut self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.read_to_end(&mut buf)?;
        Ok(buf)
    }
}

impl<'a> Read for Body<'a> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        match &mut self.0 {
            BodyAlt::SimpleOwned(c) => c.read(buf),
            BodyAlt::SimpleBorrowed(c) => c.read(buf),
            BodyAlt::Sized {
                content,
                total_len,
                consumed_len,
            } => {
                let filtered_buf_size =
                    min(*total_len - *consumed_len, buf.len().try_into().unwrap())
                        .try_into()
                        .unwrap();
                if filtered_buf_size == 0 {
                    return Ok(0); // No need to read anything
                }
                let additional = content.read(&mut buf[..filtered_buf_size])?;
                *consumed_len += u64::try_from(additional).unwrap();
                if additional == 0 && consumed_len != total_len {
                    // We check we do not miss some bytes
                    return Err(Error::new(ErrorKind::ConnectionAborted, format!("The body was expected to contain {} bytes but we have been able to only read {}", total_len, consumed_len)));
                }
                Ok(additional)
            }
            BodyAlt::Chunked(inner) => inner.read(buf),
        }
    }
}

impl<'a> Default for Body<'a> {
    fn default() -> Self {
        b"".as_ref().into()
    }
}

impl From<Vec<u8>> for Body<'static> {
    fn from(data: Vec<u8>) -> Self {
        Self(BodyAlt::SimpleOwned(Cursor::new(data)))
    }
}

impl From<String> for Body<'static> {
    fn from(data: String) -> Self {
        data.into_bytes().into()
    }
}

impl<'a> From<&'a [u8]> for Body<'a> {
    fn from(data: &'a [u8]) -> Self {
        Self(BodyAlt::SimpleBorrowed(data))
    }
}

impl<'a> From<&'a str> for Body<'a> {
    fn from(data: &'a str) -> Self {
        data.as_bytes().into()
    }
}

impl<'a> fmt::Debug for Body<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            BodyAlt::SimpleOwned(d) => f
                .debug_struct("Body")
                .field("len", &d.get_ref().len())
                .finish(),
            BodyAlt::SimpleBorrowed(d) => f.debug_struct("Body").field("len", &d.len()).finish(),
            BodyAlt::Sized { total_len, .. } => {
                f.debug_struct("Body").field("len", total_len).finish()
            }
            BodyAlt::Chunked(_) => f.debug_struct("Body").finish(),
        }
    }
}

pub trait ChunkedTransferPayload: Read {
    fn trailers(&self) -> Option<&Headers>;
}

struct SimpleChunkedTransferEncoding<R: Read>(R);

impl<R: Read> Read for SimpleChunkedTransferEncoding<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.0.read(buf)
    }
}

impl<R: Read> ChunkedTransferPayload for SimpleChunkedTransferEncoding<R> {
    fn trailers(&self) -> Option<&Headers> {
        None
    }
}
