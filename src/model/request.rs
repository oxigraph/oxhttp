use crate::model::{Body, HeaderName, HeaderValue, Headers, InvalidHeader, Method, Url};
use std::convert::TryInto;

/// A HTTP request.
///
/// ```
/// use oxhttp::model::{Request, Method, HeaderName, Body};
///
/// let request = Request::new(Method::POST, "http://example.com:80/foo".parse()?)
///     .with_header(HeaderName::CONTENT_TYPE, "application/json")?
///     .with_body("{\"foo\": \"bar\"}");
///
/// assert_eq!(*request.method(), Method::POST);
/// assert_eq!(request.url().as_str(), "http://example.com/foo");
/// assert_eq!(&request.into_body().to_vec()?, b"{\"foo\": \"bar\"}");
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Debug)]
pub struct Request {
    method: Method,
    url: Url,
    headers: Headers,
    body: Body,
}

impl Request {
    pub fn new(method: Method, url: Url) -> Self {
        Self {
            method,
            url,
            headers: Headers::new(),
            body: Body::default(),
        }
    }

    pub fn method(&self) -> &Method {
        &self.method
    }

    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn headers(&self) -> &Headers {
        &self.headers
    }

    pub fn headers_mut(&mut self) -> &mut Headers {
        &mut self.headers
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
