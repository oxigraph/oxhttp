use crate::model::{Body, Headers, Status};

/// A HTTP response.
///
/// ```
/// use oxhttp::model::{HeaderName, Body, Response, Status};
///
/// let mut response = Response::new(Status::OK);
/// response.headers_mut().append(HeaderName::CONTENT_TYPE, "application/json".parse()?);
/// let response = response.with_body("{\"foo\": \"bar\"}");
///
/// assert_eq!(response.status(), Status::OK);
/// assert_eq!(&response.into_body().to_vec()?, b"{\"foo\": \"bar\"}");
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Debug)]
pub struct Response<'a> {
    status: Status,
    headers: Headers,
    body: Body<'a>,
}

impl<'a> Response<'a> {
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

    pub fn body(&self) -> &Body<'a> {
        &self.body
    }

    pub fn with_body(mut self, body: impl Into<Body<'a>>) -> Self {
        self.body = body.into();
        self
    }

    pub fn into_body(self) -> Body<'a> {
        self.body
    }
}
