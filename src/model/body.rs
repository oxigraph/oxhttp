use crate::model::Headers;
use std::cmp::min;
use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::io::{Cursor, Error, ErrorKind, Read, Result};

/// A request or response [body](https://httpwg.org/http-core/draft-ietf-httpbis-messaging-latest.html#message.body).
///
/// It implements the [`Read`] API.
pub struct Body(BodyAlt);

enum BodyAlt {
    SimpleOwned(Cursor<Vec<u8>>),
    SimpleBorrowed(&'static [u8]),
    Sized {
        content: Box<dyn Read>,
        total_len: u64,
        consumed_len: u64,
    },
    Chunked(Box<dyn ChunkedTransferPayload>),
}

impl Body {
    /// Creates a new body from a [`Read`] implementation.
    ///
    /// If the body is sent as an HTTP request or response it will be streamed using [chunked transfer encoding](https://httpwg.org/http-core/draft-ietf-httpbis-messaging-latest.html#chunked.encoding).
    #[inline]
    pub fn from_read(read: impl Read + 'static) -> Self {
        Self::from_chunked_transfer_payload(SimpleChunkedTransferEncoding(read))
    }

    #[inline]
    pub(crate) fn from_read_and_len(read: impl Read + 'static, len: u64) -> Self {
        Self(BodyAlt::Sized {
            total_len: len,
            consumed_len: 0,
            content: Box::new(read.take(len)),
        })
    }

    /// Creates a [chunked transfer encoding](https://httpwg.org/http-core/draft-ietf-httpbis-messaging-latest.html#chunked.encoding) body with optional [trailers](https://httpwg.org/http-core/draft-ietf-httpbis-semantics-latest.html#trailer.fields).
    #[inline]
    pub fn from_chunked_transfer_payload(payload: impl ChunkedTransferPayload + 'static) -> Self {
        Self(BodyAlt::Chunked(Box::new(payload)))
    }

    /// The number of bytes in the body (if known).
    #[allow(clippy::len_without_is_empty)]
    #[inline]
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
    #[inline]
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
    #[inline]
    pub fn to_vec(mut self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.read_to_end(&mut buf)?;
        Ok(buf)
    }

    /// Reads the full body into a string.
    ///
    /// WARNING: Beware of the body size!
    ///
    /// ```
    /// use oxhttp::model::Body;
    /// use std::io::Cursor;
    ///
    /// let mut body = Body::from_read(b"foo".as_ref());
    /// assert_eq!(&body.to_string()?, "foo");
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn to_string(mut self) -> Result<String> {
        let mut buf = String::new();
        self.read_to_string(&mut buf)?;
        Ok(buf)
    }
}

impl Read for Body {
    #[inline]
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

impl Default for Body {
    #[inline]
    fn default() -> Self {
        b"".as_ref().into()
    }
}

impl From<Vec<u8>> for Body {
    #[inline]
    fn from(data: Vec<u8>) -> Self {
        Self(BodyAlt::SimpleOwned(Cursor::new(data)))
    }
}

impl From<String> for Body {
    #[inline]
    fn from(data: String) -> Self {
        data.into_bytes().into()
    }
}

impl From<&'static [u8]> for Body {
    #[inline]
    fn from(data: &'static [u8]) -> Self {
        Self(BodyAlt::SimpleBorrowed(data))
    }
}

impl From<&'static str> for Body {
    #[inline]
    fn from(data: &'static str) -> Self {
        data.as_bytes().into()
    }
}

impl fmt::Debug for Body {
    #[inline]
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

/// Trait to give to [`Body::from_chunked_transfer_payload`] a body to serialize
/// as [chunked transfer encoding](https://httpwg.org/http-core/draft-ietf-httpbis-messaging-latest.html#chunked.encoding).
///
/// It allows to provide [trailers](https://httpwg.org/http-core/draft-ietf-httpbis-semantics-latest.html#trailer.fields) to serialize.
pub trait ChunkedTransferPayload: Read {
    /// The [trailers](https://httpwg.org/http-core/draft-ietf-httpbis-semantics-latest.html#trailer.fields) to serialize.
    fn trailers(&self) -> Option<&Headers>;
}

struct SimpleChunkedTransferEncoding<R: Read>(R);

impl<R: Read> Read for SimpleChunkedTransferEncoding<R> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.0.read(buf)
    }
}

impl<R: Read> ChunkedTransferPayload for SimpleChunkedTransferEncoding<R> {
    #[inline]
    fn trailers(&self) -> Option<&Headers> {
        None
    }
}
