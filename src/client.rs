//! Simple HTTP client

use crate::io::{decode_response, encode_request};
use crate::model::{HeaderName, HeaderValue, InvalidHeader, Request, Response, Url};
use crate::utils::invalid_input_error;
#[cfg(feature = "native-tls")]
use native_tls::TlsConnector;
use std::convert::TryFrom;
use std::io::{BufReader, BufWriter, Error, ErrorKind, Result};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// A simple HTTP client.
///
/// It aims at following the basic concepts of the [Web Fetch standard](https://fetch.spec.whatwg.org/) without the bits specific to web browsers (context, CORS...).
///
/// HTTPS is supported behind the disabled by default `native-tls` feature.
///
/// Missing: HSTS support, authentication, redirects and keep alive.
///
/// ```
/// use oxhttp::Client;
/// use oxhttp::model::{Request, Method, Status, HeaderName};
/// use std::io::Read;
///
/// let client = Client::new();
/// let response = client.request(Request::builder(Method::GET, "http://example.com".parse()?).build())?;
/// assert_eq!(response.status(), Status::OK);
/// assert_eq!(response.header(&HeaderName::CONTENT_TYPE).unwrap().as_ref(), b"text/html; charset=UTF-8");
/// let body = response.into_body().to_string()?;
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[allow(missing_copy_implementations)]
#[derive(Default)]
pub struct Client {
    timeout: Option<Duration>,
    user_agent: Option<HeaderValue>,
}

impl Client {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the global timout value (applies to both read, write and connection).
    #[inline]
    pub fn set_global_timeout(&mut self, timeout: Duration) {
        self.timeout = Some(timeout);
    }

    /// Sets the default value for the [`User-Agent`](https://httpwg.org/http-core/draft-ietf-httpbis-semantics-latest.html#field.user-agent) header.
    #[inline]
    pub fn set_user_agent(
        &mut self,
        user_agent: impl Into<String>,
    ) -> std::result::Result<(), InvalidHeader> {
        self.user_agent = Some(HeaderValue::try_from(user_agent.into())?);
        Ok(())
    }

    pub fn request(&self, mut request: Request) -> Result<Response> {
        // Additional headers
        set_header_fallback(&mut request, HeaderName::USER_AGENT, &self.user_agent);
        request
            .headers_mut()
            .set(HeaderName::CONNECTION, HeaderValue::new_unchecked("close"));

        let host = request
            .url()
            .host_str()
            .ok_or_else(|| invalid_input_error("No host provided"))?;

        match request.url().scheme() {
            "http" => {
                let port = get_and_validate_port(request.url(), 80)?;
                let mut stream = self.connect((host, port))?;
                encode_request(request, BufWriter::new(&mut stream))?;
                decode_response(BufReader::new(stream))
            }
            "https" => {
                #[cfg(feature = "native-tls")]
                {
                    let port = get_and_validate_port(request.url(), 443)?;
                    let connector =
                        TlsConnector::new().map_err(|e| Error::new(ErrorKind::Other, e))?;
                    let stream = self.connect((host, port))?;
                    let mut stream = connector
                        .connect(host, stream)
                        .map_err(|e| Error::new(ErrorKind::Other, e))?;
                    encode_request(request, BufWriter::new(&mut stream))?;
                    decode_response(BufReader::new(stream))
                }
                #[cfg(not(feature = "native-tls"))]
                Err(invalid_input_error(format!("HTTPS is not supported by the client. You should enable the `native-tls` feature of the `oxhttp` crate")))
            }
            _ => Err(invalid_input_error(format!(
                "Not supported URL scheme: {}",
                request.url().scheme()
            ))),
        }
    }

    fn connect(&self, addr: impl ToSocketAddrs) -> Result<TcpStream> {
        let stream = if let Some(timeout) = self.timeout {
            addr.to_socket_addrs()?.fold(
                Err(Error::new(
                    ErrorKind::InvalidInput,
                    "Not able to resolve the provide addresses",
                )),
                |e, addr| match e {
                    Ok(stream) => Ok(stream),
                    Err(_) => TcpStream::connect_timeout(&addr, timeout),
                },
            )
        } else {
            TcpStream::connect(addr)
        }?;
        stream.set_read_timeout(self.timeout)?;
        stream.set_write_timeout(self.timeout)?;
        Ok(stream)
    }
}

// Bad ports https://fetch.spec.whatwg.org/#bad-port
// Should be sorted
const BAD_PORTS: [u16; 80] = [
    1, 7, 9, 11, 13, 15, 17, 19, 20, 21, 22, 23, 25, 37, 42, 43, 53, 69, 77, 79, 87, 95, 101, 102,
    103, 104, 109, 110, 111, 113, 115, 117, 119, 123, 135, 137, 139, 143, 161, 179, 389, 427, 465,
    512, 513, 514, 515, 526, 530, 531, 532, 540, 548, 554, 556, 563, 587, 601, 636, 989, 990, 993,
    995, 1719, 1720, 1723, 2049, 3659, 4045, 5060, 5061, 6000, 6566, 6665, 6666, 6667, 6668, 6669,
    6697, 10080,
];

fn get_and_validate_port(url: &Url, default_port: u16) -> Result<u16> {
    url.port().map_or(Ok(default_port), |port| {
        if BAD_PORTS.binary_search(&port).is_ok() {
            Err(invalid_input_error(format!(
                "The port {} is not allowed for HTTP(S) because it is dedicated to an other use",
                port
            )))
        } else {
            Ok(port)
        }
    })
}

fn set_header_fallback(
    request: &mut Request,
    header_name: HeaderName,
    header_value: &Option<HeaderValue>,
) {
    if let Some(header_value) = header_value {
        if !request.headers().contains(&header_name) {
            request.headers_mut().set(header_name, header_value.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Method, Status};

    #[test]
    fn test_http_get_ok() -> Result<()> {
        let client = Client::new();
        let response = client.request(
            Request::builder(Method::GET, "http://example.com".parse().unwrap()).build(),
        )?;
        assert_eq!(response.status(), Status::OK);
        assert_eq!(
            response.header(&HeaderName::CONTENT_TYPE).unwrap().as_ref(),
            b"text/html; charset=UTF-8"
        );
        Ok(())
    }

    #[test]
    fn test_http_get_ok_with_user_agent_and_timeout() -> Result<()> {
        let mut client = Client::new();
        client.set_user_agent("OxHTTP/1.0").unwrap();
        client.set_global_timeout(Duration::from_secs(5));
        let response = client.request(
            Request::builder(Method::GET, "http://example.com".parse().unwrap()).build(),
        )?;
        assert_eq!(response.status(), Status::OK);
        assert_eq!(
            response.header(&HeaderName::CONTENT_TYPE).unwrap().as_ref(),
            b"text/html; charset=UTF-8"
        );
        Ok(())
    }

    #[test]
    fn test_http_get_ok_explicit_port() -> Result<()> {
        let client = Client::new();
        let response = client.request(
            Request::builder(Method::GET, "http://example.com:80".parse().unwrap()).build(),
        )?;
        assert_eq!(response.status(), Status::OK);
        assert_eq!(
            response.header(&HeaderName::CONTENT_TYPE).unwrap().as_ref(),
            b"text/html; charset=UTF-8"
        );
        Ok(())
    }

    #[test]
    fn test_http_wrong_port() {
        let client = Client::new();
        assert!(client
            .request(
                Request::builder(Method::GET, "http://example.com:22".parse().unwrap()).build(),
            )
            .is_err());
    }

    #[cfg(feature = "native-tls")]
    #[test]
    fn test_https_get_ok() -> Result<()> {
        let client = Client::new();
        let response = client.request(
            Request::builder(Method::GET, "https://example.com".parse().unwrap()).build(),
        )?;
        assert_eq!(response.status(), Status::OK);
        assert_eq!(
            response.header(&HeaderName::CONTENT_TYPE).unwrap().as_ref(),
            b"text/html; charset=UTF-8"
        );
        Ok(())
    }

    #[cfg(not(feature = "native-tls"))]
    #[test]
    fn test_https_get_ok() {
        let client = Client::new();
        assert!(client
            .request(Request::builder(Method::GET, "https://example.com".parse().unwrap()).build())
            .is_err());
    }

    #[test]
    fn test_http_get_not_found() -> Result<()> {
        let client = Client::new();
        let response = client.request(
            Request::builder(
                Method::GET,
                "http://example.com/not_existing".parse().unwrap(),
            )
            .build(),
        )?;
        assert_eq!(response.status(), Status::NOT_FOUND);
        Ok(())
    }

    #[test]
    fn test_file_get_error() {
        let client = Client::new();
        assert!(client
            .request(
                Request::builder(
                    Method::GET,
                    "file://example.com/not_existing".parse().unwrap(),
                )
                .build(),
            )
            .is_err());
    }
}
