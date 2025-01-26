use crate::model::header::{
    ACCEPT_CHARSET, ACCEPT_ENCODING, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_REQUEST_HEADERS,
    CONNECTION, CONTENT_LENGTH, DATE, EXPECT, HOST, ORIGIN, TE, TRAILER, TRANSFER_ENCODING,
    UPGRADE, VIA,
};
use crate::model::{Body, HeaderMap, HeaderName, Method, Request, Response, StatusCode};
use crate::utils::invalid_input_error;
use std::io::{copy, Read, Result, Write};

pub fn encode_request<W: Write>(request: &mut Request<Body>, mut writer: W) -> Result<W> {
    if request
        .uri()
        .authority()
        .is_some_and(|a| a.as_str().contains('@'))
    {
        return Err(invalid_input_error(
            "Username and password are not allowed in HTTP URLs",
        ));
    }
    let host = request
        .uri()
        .host()
        .ok_or_else(|| invalid_input_error("No host provided"))?;

    if let Some(query) = request.uri().query() {
        write!(
            &mut writer,
            "{} {}?{} HTTP/1.1\r\n",
            request.method().as_str(),
            request.uri().path(),
            query
        )?;
    } else {
        write!(
            &mut writer,
            "{} {} HTTP/1.1\r\n",
            request.method().as_str(),
            request.uri().path(),
        )?;
    }

    // host
    if let Some(port) = request.uri().port() {
        write!(writer, "host: {host}:{}\r\n", port.as_str())?;
    } else {
        write!(writer, "host: {host}\r\n")?;
    }

    // headers
    encode_headers(request.headers(), &mut writer)?;

    // body with content-length if existing
    let must_include_body = does_request_must_include_body(request.method());
    encode_body(request.body_mut(), &mut writer, must_include_body)?;

    Ok(writer)
}

pub fn encode_response<W: Write>(response: &mut Response<Body>, mut writer: W) -> Result<W> {
    let status = response.status();
    write!(
        &mut writer,
        "HTTP/1.1 {} {}\r\n",
        status.as_u16(),
        status.canonical_reason().unwrap_or_default()
    )?;
    encode_headers(response.headers(), &mut writer)?;
    let must_include_body = does_response_must_include_body(response.status());
    encode_body(response.body_mut(), &mut writer, must_include_body)?;
    Ok(writer)
}

fn encode_headers(headers: &HeaderMap, writer: &mut impl Write) -> Result<()> {
    for (name, value) in headers {
        if !is_forbidden_name(name) {
            write!(writer, "{name}: ")?;
            writer.write_all(value.as_bytes())?;
            write!(writer, "\r\n")?;
        }
    }
    Ok(())
}

fn encode_body(body: &mut Body, writer: &mut impl Write, must_include_body: bool) -> Result<()> {
    if let Some(length) = body.len() {
        if must_include_body || length > 0 {
            write!(writer, "content-length: {length}\r\n\r\n")?;
            copy(body, writer)?;
        } else {
            write!(writer, "\r\n")?;
        }
    } else {
        write!(writer, "transfer-encoding: chunked\r\n\r\n")?;
        let mut buffer = vec![b'\0'; 4096];
        loop {
            let mut read = 0;
            while read < 1024 {
                // We try to avoid too small chunks
                let new_read = body.read(&mut buffer[read..])?;
                if new_read == 0 {
                    break; // EOF
                }
                read += new_read;
            }
            write!(writer, "{read:X}\r\n")?;
            writer.write_all(&buffer[..read])?;
            if read == 0 {
                break; // Done
            } else {
                write!(writer, "\r\n")?;
            }
        }
        if let Some(trailers) = body.trailers() {
            encode_headers(trailers, writer)?;
        }
        write!(writer, "\r\n")?;
    }
    Ok(())
}

/// Checks if it is a [forbidden header name](https://fetch.spec.whatwg.org/#forbidden-header-name)
///
/// We removed some of them not managed by this library (`Access-Control-Request-Headers`, `Access-Control-Request-Method`, `DNT`, `Cookie`, `Cookie2`, `Referer`, `Proxy-`, `Sec-`, `Via`...)
fn is_forbidden_name(header: &HeaderName) -> bool {
    header == ACCEPT_CHARSET
        || header == ACCEPT_ENCODING
        || header == ACCESS_CONTROL_REQUEST_HEADERS
        || header == ACCESS_CONTROL_ALLOW_METHODS
        || header == CONNECTION
        || header == CONTENT_LENGTH
        || header == DATE
        || header == EXPECT
        || header == HOST
        || header.as_str() == "keep-alive"
        || header == ORIGIN
        || header == TE
        || header == TRAILER
        || header == TRANSFER_ENCODING
        || header == UPGRADE
        || header == VIA
}

fn does_request_must_include_body(method: &Method) -> bool {
    *method == Method::POST || *method == Method::PUT
}

fn does_response_must_include_body(status: StatusCode) -> bool {
    !(status.is_informational()
        || status == StatusCode::NO_CONTENT
        || status == StatusCode::NOT_MODIFIED)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::header::{ACCEPT, CONTENT_LANGUAGE};
    use crate::model::{ChunkedTransferPayload, HeaderMap, HeaderValue};
    use std::str;

    #[test]
    fn user_password_not_allowed_in_request() {
        let mut buffer = Vec::new();
        assert!(encode_request(
            &mut Request::builder()
                .uri("http://foo@example.com/")
                .body(Body::empty())
                .unwrap(),
            &mut buffer
        )
        .is_err());
        assert!(encode_request(
            &mut Request::builder()
                .uri("http://foo:bar@example.com/")
                .body(Body::empty())
                .unwrap(),
            &mut buffer
        )
        .is_err());
    }

    #[test]
    fn encode_get_request() -> Result<()> {
        let mut request = Request::builder()
            .uri("http://example.com:81/foo/bar?query#fragment")
            .header(ACCEPT, "application/json")
            .body(Body::empty())
            .unwrap();
        let buffer = encode_request(&mut request, Vec::new())?;
        assert_eq!(
            str::from_utf8(&buffer).unwrap(),
            "GET /foo/bar?query HTTP/1.1\r\nhost: example.com:81\r\naccept: application/json\r\n\r\n"
        );
        Ok(())
    }

    #[test]
    fn encode_post_request() -> Result<()> {
        let mut request = Request::builder()
            .method(Method::POST)
            .uri("http://example.com/foo/bar?query#fragment")
            .header(ACCEPT, "application/json")
            .body(Body::from("testbodybody"))
            .unwrap();
        let buffer = encode_request(&mut request, Vec::new())?;
        assert_eq!(
            str::from_utf8(&buffer).unwrap(),
            "POST /foo/bar?query HTTP/1.1\r\nhost: example.com\r\naccept: application/json\r\ncontent-length: 12\r\n\r\ntestbodybody"
        );
        Ok(())
    }

    #[test]
    fn encode_post_request_without_body() -> Result<()> {
        let mut request = Request::builder()
            .method(Method::POST)
            .uri("http://example.com/foo/bar?query#fragment")
            .body(Body::empty())
            .unwrap();
        let buffer = encode_request(&mut request, Vec::new())?;
        assert_eq!(
            str::from_utf8(&buffer).unwrap(),
            "POST /foo/bar?query HTTP/1.1\r\nhost: example.com\r\ncontent-length: 0\r\n\r\n"
        );
        Ok(())
    }

    #[test]
    fn encode_post_request_with_chunked() -> Result<()> {
        let mut trailers = HeaderMap::new();
        trailers.append(CONTENT_LANGUAGE, HeaderValue::from_static("foo"));

        let mut request = Request::builder()
            .method(Method::POST)
            .uri("http://example.com/foo/bar?query#fragment")
            .body(Body::from_chunked_transfer_payload(SimpleTrailers {
                read: b"testbodybody".as_slice(),
                trailers,
            }))
            .unwrap();
        let buffer = encode_request(&mut request, Vec::new())?;
        assert_eq!(
            str::from_utf8(&buffer).unwrap(),
            "POST /foo/bar?query HTTP/1.1\r\nhost: example.com\r\ntransfer-encoding: chunked\r\n\r\nC\r\ntestbodybody\r\n0\r\ncontent-language: foo\r\n\r\n"
        );
        Ok(())
    }

    #[test]
    fn encode_response_ok() -> Result<()> {
        let mut response = Response::builder()
            .header(ACCEPT, "application/json")
            .body(Body::from("test test2"))
            .unwrap();
        let buffer = encode_response(&mut response, Vec::new())?;
        assert_eq!(
            str::from_utf8(&buffer).unwrap(),
            "HTTP/1.1 200 OK\r\naccept: application/json\r\ncontent-length: 10\r\n\r\ntest test2"
        );
        Ok(())
    }

    #[test]
    fn encode_response_not_found() -> Result<()> {
        let mut response = Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap();
        let buffer = encode_response(&mut response, Vec::new())?;
        assert_eq!(
            str::from_utf8(&buffer).unwrap(),
            "HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\n\r\n"
        );
        Ok(())
    }

    #[test]
    fn encode_response_custom_code() -> Result<()> {
        let mut response = Response::builder().status(499).body(Body::empty()).unwrap();
        let buffer = encode_response(&mut response, Vec::new())?;
        assert_eq!(
            str::from_utf8(&buffer).unwrap(),
            "HTTP/1.1 499 \r\ncontent-length: 0\r\n\r\n"
        );
        Ok(())
    }

    struct SimpleTrailers {
        read: &'static [u8],
        trailers: HeaderMap,
    }

    impl Read for SimpleTrailers {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
            self.read.read(buf)
        }
    }

    impl ChunkedTransferPayload for SimpleTrailers {
        fn trailers(&self) -> Option<&HeaderMap> {
            Some(&self.trailers)
        }
    }
}
