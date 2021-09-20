use crate::model::{Body, HeaderName, HeaderValue, Headers, InvalidHeader, Status};
use std::convert::TryInto;

/// A HTTP response.
///
/// ```
/// use oxhttp::model::{HeaderName, Body, Response, Status};
///
/// let response = Response::new(Status::OK)
///     .with_header(HeaderName::CONTENT_TYPE, "application/json")?
///     .with_body("{\"foo\": \"bar\"}");
///
/// assert_eq!(response.status(), Status::OK);
/// assert_eq!(response.header(&HeaderName::CONTENT_TYPE).unwrap().as_ref(), b"application/json");
/// assert_eq!(&response.into_body().to_vec()?, b"{\"foo\": \"bar\"}");
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Debug)]
pub struct Response {
    status: Status,
    headers: Headers,
    body: Body,
}

impl Response {
    pub fn new(status: Status) -> Self {
        Self {
            status,
            headers: Headers::new(),
            body: Body::default(),
        }
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn headers(&self) -> &Headers {
        &self.headers
    }

    pub fn headers_mut(&mut self) -> &mut Headers {
        &mut self.headers
    }

    pub fn header(&self, name: &HeaderName) -> Option<&HeaderValue> {
        self.headers.get(name)
    }

    pub fn with_header(
        mut self,
        name: HeaderName,
        value: impl TryInto<HeaderValue, Error = InvalidHeader>,
    ) -> Result<Self, InvalidHeader> {
        self.headers_mut().append(name, value.try_into()?);
        Ok(self)
    }

    pub fn body(&self) -> &Body {
        &self.body
    }

    pub fn with_body(mut self, body: impl Into<Body>) -> Self {
        self.body = body.into();
        self
    }

    pub fn into_body(self) -> Body {
        self.body
    }
}
