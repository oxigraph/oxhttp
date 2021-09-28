use crate::model::{Body, HeaderName, Request, Response};
use crate::utils::invalid_input_error;
use std::io::{copy, Read, Result, Write};

pub fn encode_request(request: Request, mut writer: impl Write) -> Result<()> {
    if !request.url().username().is_empty() || request.url().password().is_some() {
        return Err(invalid_input_error(
            "Username and password are not allowed in HTTP URLs",
        ));
    }
    let host = request
        .url()
        .host_str()
        .ok_or_else(|| invalid_input_error("No host provided"))?;

    if let Some(query) = request.url().query() {
        write!(
            &mut writer,
            "{} {}?{} HTTP/1.1\r\n",
            request.method(),
            request.url().path(),
            query
        )?;
    } else {
        write!(
            &mut writer,
            "{} {} HTTP/1.1\r\n",
            request.method(),
            request.url().path(),
        )?;
    }

    // host
    if let Some(port) = request.url().port() {
        write!(writer, "host: {}:{}\r\n", host, port)?;
    } else {
        write!(writer, "host: {}\r\n", host)?;
    }

    // headers
    for (name, value) in request.headers() {
        if !is_forbidden_name(name) {
            write!(writer, "{}: ", name)?;
            writer.write_all(value)?;
            write!(writer, "\r\n")?;
        }
    }

    // body with content-length if existing
    encode_body(request.into_body(), &mut writer)?;

    writer.flush()
}

pub fn encode_response(response: Response, mut writer: impl Write) -> Result<()> {
    write!(&mut writer, "HTTP/1.1 {}\r\n", response.status())?;

    // headers
    for (name, value) in response.headers() {
        if !is_forbidden_name(name) {
            write!(writer, "{}: ", name)?;
            writer.write_all(value)?;
            write!(writer, "\r\n")?;
        }
    }

    // body with content-length if existing
    encode_body(response.into_body(), &mut writer)?;

    writer.flush()
}

fn encode_body(mut body: Body, writer: &mut impl Write) -> Result<()> {
    if let Some(length) = body.len() {
        if length > 0 {
            write!(writer, "content-length: {}\r\n\r\n", length)?;
            copy(&mut body, writer)?;
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
            write!(writer, "{:X}\r\n", read)?;
            writer.write_all(&buffer[..read])?;
            write!(writer, "\r\n")?;
            if read == 0 {
                break; // Done
            }
        }
        if let Some(trailers) = body.trailers() {
            for (name, value) in trailers {
                if !is_forbidden_name(name) {
                    write!(writer, "{}: ", name)?;
                    writer.write_all(value)?;
                    write!(writer, "\r\n")?;
                }
            }
        }
        write!(writer, "\r\n")?;
    }
    Ok(())
}

/// Checks if it is a [forbidden header name](https://fetch.spec.whatwg.org/#forbidden-header-name)
///
/// We removed some of them not managed by this library (`Access-Control-Request-Headers`, `Access-Control-Request-Method`, `DNT`, `Cookie`, `Cookie2`, `Referer`, `Proxy-`, `Sec-`, `Via`...)
fn is_forbidden_name(header: &HeaderName) -> bool {
    header.as_ref() == "accept-charset"
        || *header == HeaderName::ACCEPT_ENCODING
        || header.as_ref() == "access-control-request-headers"
        || header.as_ref() == "access-control-request-method"
        || *header == HeaderName::CONNECTION
        || *header == HeaderName::CONTENT_LENGTH
        || *header == HeaderName::DATE
        || *header == HeaderName::EXPECT
        || *header == HeaderName::HOST
        || header.as_ref() == "keep-alive"
        || header.as_ref() == "origin"
        || *header == HeaderName::TE
        || *header == HeaderName::TRAILER
        || *header == HeaderName::TRANSFER_ENCODING
        || *header == HeaderName::UPGRADE
        || *header == HeaderName::VIA
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ChunkedTransferPayload, Headers, Method, Status};
    use std::io::Cursor;
    use std::str;

    #[test]
    fn user_password_not_allowed_in_request() {
        let mut buffer = Vec::new();
        assert!(encode_request(
            Request::builder(Method::GET, "http://foo@example.com/".parse().unwrap()).build(),
            &mut buffer
        )
        .is_err());
        assert!(encode_request(
            Request::builder(Method::GET, "http://foo:bar@example.com/".parse().unwrap()).build(),
            &mut buffer
        )
        .is_err());
    }

    #[test]
    fn encode_get_request() -> Result<()> {
        let request = Request::builder(
            Method::GET,
            "http://example.com:81/foo/bar?query#fragment"
                .parse()
                .unwrap(),
        )
        .with_header(HeaderName::ACCEPT, "application/json")
        .unwrap()
        .build();
        let mut buffer = Vec::new();
        encode_request(request, &mut buffer)?;
        assert_eq!(
            str::from_utf8(&buffer).unwrap(),
            "GET /foo/bar?query HTTP/1.1\r\nhost: example.com:81\r\naccept: application/json\r\n\r\n"
        );
        Ok(())
    }

    #[test]
    fn encode_post_request() -> Result<()> {
        let request = Request::builder(
            Method::POST,
            "http://example.com/foo/bar?query#fragment".parse().unwrap(),
        )
        .with_header(HeaderName::ACCEPT, "application/json")
        .unwrap()
        .with_body(b"testbodybody".as_ref());
        let mut buffer = Vec::new();
        encode_request(request, &mut buffer)?;
        assert_eq!(
            str::from_utf8(&buffer).unwrap(),
            "POST /foo/bar?query HTTP/1.1\r\nhost: example.com\r\naccept: application/json\r\ncontent-length: 12\r\n\r\ntestbodybody"
        );
        Ok(())
    }

    #[test]
    fn encode_post_request_with_chunked() -> Result<()> {
        let mut trailers = Headers::new();
        trailers.append(HeaderName::CONTENT_LANGUAGE, "foo".parse().unwrap());

        let request = Request::builder(
            Method::POST,
            "http://example.com/foo/bar?query#fragment".parse().unwrap(),
        )
        .with_body(Body::from_chunked_transfer_payload(SimpleTrailers {
            read: Cursor::new("testbodybody"),
            trailers,
        }));
        let mut buffer = Vec::new();
        encode_request(request, &mut buffer)?;
        assert_eq!(
            str::from_utf8(&buffer).unwrap(),
            "POST /foo/bar?query HTTP/1.1\r\nhost: example.com\r\ntransfer-encoding: chunked\r\n\r\nC\r\ntestbodybody\r\n0\r\n\r\ncontent-language: foo\r\n\r\n"
        );
        Ok(())
    }

    #[test]
    fn encode_response_ok() -> Result<()> {
        let response = Response::builder(Status::OK)
            .with_header(HeaderName::ACCEPT, "application/json")
            .unwrap()
            .with_body("test test2");
        let mut buffer = Vec::new();
        encode_response(response, &mut buffer)?;
        assert_eq!(
            str::from_utf8(&buffer).unwrap(),
            "HTTP/1.1 200 OK\r\naccept: application/json\r\ncontent-length: 10\r\n\r\ntest test2"
        );
        Ok(())
    }

    #[test]
    fn encode_response_not_found() -> Result<()> {
        let response = Response::builder(Status::NOT_FOUND).build();
        let mut buffer = Vec::new();
        encode_response(response, &mut buffer)?;
        assert_eq!(
            str::from_utf8(&buffer).unwrap(),
            "HTTP/1.1 404 Not Found\r\n\r\n"
        );
        Ok(())
    }

    struct SimpleTrailers {
        read: Cursor<&'static str>,
        trailers: Headers,
    }

    impl Read for SimpleTrailers {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
            self.read.read(buf)
        }
    }

    impl ChunkedTransferPayload for SimpleTrailers {
        fn trailers(&self) -> Option<&Headers> {
            Some(&self.trailers)
        }
    }
}
