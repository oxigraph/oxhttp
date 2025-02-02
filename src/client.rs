#![allow(unreachable_code, clippy::needless_return)]

use crate::io::{decode_response, encode_request, BUFFER_CAPACITY};
use crate::model::header::{
    InvalidHeaderValue, ACCEPT_ENCODING, CONNECTION, LOCATION, RANGE, USER_AGENT,
};
use crate::model::uri::Scheme;
use crate::model::{Body, HeaderValue, Method, Request, Response, StatusCode, Uri};
use crate::utils::{invalid_data_error, invalid_input_error};
#[cfg(feature = "native-tls")]
use native_tls::TlsConnector;
#[cfg(all(
    any(feature = "rustls-aws-lc-webpki", feature = "rustls-ring-webpki"),
    not(feature = "native-tls"),
    not(feature = "rustls-aws-lc-native"),
    not(feature = "rustls-ring-native"),
))]
use rustls::RootCertStore;
#[cfg(all(
    any(
        feature = "rustls-aws-lc-webpki",
        feature = "rustls-ring-webpki",
        feature = "rustls-aws-lc-native",
        feature = "rustls-ring-native"
    ),
    not(feature = "native-tls")
))]
use rustls::{ClientConfig, ClientConnection, StreamOwned};
#[cfg(all(
    any(
        feature = "rustls-aws-lc-webpki",
        feature = "rustls-ring-webpki",
        feature = "rustls-aws-lc-native",
        feature = "rustls-ring-native"
    ),
    not(feature = "native-tls")
))]
use rustls_pki_types::ServerName;
#[cfg(all(
    any(feature = "rustls-aws-lc-native", feature = "rustls-ring-native"),
    not(feature = "native-tls")
))]
use rustls_platform_verifier::ConfigVerifierExt;
use std::io::{BufReader, BufWriter, Error, ErrorKind, Result};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
#[cfg(all(
    any(
        feature = "rustls-aws-lc-webpki",
        feature = "rustls-ring-webpki",
        feature = "rustls-aws-lc-native",
        feature = "rustls-ring-native"
    ),
    not(feature = "native-tls")
))]
use std::sync::Arc;
#[cfg(any(
    feature = "rustls-aws-lc-webpki",
    feature = "rustls-ring-webpki",
    feature = "rustls-aws-lc-native",
    feature = "rustls-ring-native",
    feature = "native-tls"
))]
use std::sync::OnceLock;
use std::time::Duration;
use url::Url;
#[cfg(all(
    any(feature = "rustls-aws-lc-webpki", feature = "rustls-ring-webpki"),
    not(feature = "native-tls"),
    not(feature = "rustls-aws-lc-native"),
    not(feature = "rustls-ring-native"),
))]
use webpki_roots::TLS_SERVER_ROOTS;

/// An HTTP client.
///
/// It aims at following the basic concepts of the [Web Fetch standard](https://fetch.spec.whatwg.org/) without the bits specific to web browsers (context, CORS...).
///
/// HTTPS is supported behind the disabled by default features.
/// To enable it you need to enable one of the following features:
///
/// * `native-tls` to use the current system native implementation.
/// * `rustls-ring-webpki` to use [Rustls](https://github.com/rustls/rustls) with
///   the [Ring](https://github.com/briansmith/ring) cryptographic library and
///   the [Common CA Database](https://www.ccadb.org/).
/// * `rustls-ring-native` to use [Rustls](https://github.com/rustls/rustls) with
///   the [Ring](https://github.com/briansmith/ring) cryptographic library and the host certificates.
/// * `rustls-aws-lc-webpki` to use [Rustls](https://github.com/rustls/rustls) with
///   the [AWS Libcrypto for Rust](https://github.com/aws/aws-lc-rs) and
///   the [Common CA Database](https://www.ccadb.org/).
/// * `rustls-aws-lc-native` to use [Rustls](https://github.com/rustls/rustls) with
///   the [AWS Libcrypto for Rust](https://github.com/aws/aws-lc-rs) and the host certificates.
///
/// If the `flate2` feature is enabled, the client will automatically decode `gzip` and `deflate` content-encodings.
///
/// The client does not follow redirections by default. Use [`Client::with_redirection_limit`] to set a limit to the number of consecutive redirections the server should follow.
///
/// Missing: HSTS support, authentication and keep alive.
///
/// ```
/// use http::header::CONTENT_TYPE;
/// use oxhttp::model::{Body, HeaderName, Method, Request, StatusCode};
/// use oxhttp::Client;
/// use std::io::Read;
///
/// let client = Client::new();
/// let response = client.request(
///     Request::builder()
///         .uri("http://example.com")
///         .body(Body::empty())?,
/// )?;
/// assert_eq!(response.status(), StatusCode::OK);
/// assert_eq!(response.headers().get(CONTENT_TYPE).unwrap(), "text/html");
/// let body = response.into_body().to_string()?;
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Default)]
pub struct Client {
    timeout: Option<Duration>,
    user_agent: Option<HeaderValue>,
    redirection_limit: usize,
}

impl Client {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the global timeout value (applies to both read, write and connection).
    #[inline]
    pub fn with_global_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Sets the default value for the [`User-Agent`](https://httpwg.org/http-core/draft-ietf-httpbis-semantics-latest.html#field.user-agent) header.
    #[inline]
    pub fn with_user_agent(
        mut self,
        user_agent: impl Into<String>,
    ) -> std::result::Result<Self, InvalidHeaderValue> {
        self.user_agent = Some(HeaderValue::try_from(user_agent.into())?);
        Ok(self)
    }

    /// Sets the number of time a redirection should be followed.
    /// By default the redirections are not followed (limit = 0).
    #[inline]
    pub fn with_redirection_limit(mut self, limit: usize) -> Self {
        self.redirection_limit = limit;
        self
    }

    pub fn request(&self, request: Request<impl Into<Body>>) -> Result<Response<Body>> {
        let mut request = request.map(Into::into);
        // Loops the number of allowed redirections + 1
        for _ in 0..(self.redirection_limit + 1) {
            let previous_method = request.method().clone();
            let response = self.single_request(&mut request)?;
            let Some(location) = response.headers().get(LOCATION) else {
                return Ok(response);
            };
            let mut request_builder = Request::builder();
            request_builder = request_builder.method(match response.status() {
                StatusCode::MOVED_PERMANENTLY | StatusCode::FOUND | StatusCode::SEE_OTHER => {
                    if previous_method == Method::HEAD {
                        Method::HEAD
                    } else {
                        Method::GET
                    }
                }
                StatusCode::TEMPORARY_REDIRECT | StatusCode::PERMANENT_REDIRECT
                    if previous_method.is_safe() =>
                {
                    previous_method
                }
                _ => return Ok(response),
            });
            let location = location.to_str().map_err(invalid_data_error)?;
            request_builder = request_builder.uri(join_urls(request.uri(), location)?);
            for (header_name, header_value) in request.headers() {
                request_builder = request_builder.header(header_name, header_value);
            }
            request = request_builder.body(Body::empty()).map_err(|e| {
                invalid_input_error(format!(
                    "Failure when trying to build the redirected request: {e}"
                ))
            })?;
        }
        Err(Error::new(
            ErrorKind::Other,
            format!(
                "The server requested too many redirects ({}). The latest redirection target is {}",
                self.redirection_limit + 1,
                request.uri()
            ),
        ))
    }

    fn single_request(&self, request: &mut Request<Body>) -> Result<Response<Body>> {
        // Additional headers
        {
            let headers = request.headers_mut();
            headers.insert(CONNECTION, HeaderValue::from_static("close"));
            if let Some(user_agent) = &self.user_agent {
                headers
                    .entry(USER_AGENT)
                    .or_insert_with(|| user_agent.clone());
            }
            if cfg!(feature = "flate2") && !headers.contains_key(RANGE) {
                headers
                    .entry(ACCEPT_ENCODING)
                    .or_insert_with(|| HeaderValue::from_static("gzip,deflate"));
            }
        }

        #[cfg(any(
            feature = "rustls-aws-lc-webpki",
            feature = "rustls-ring-webpki",
            feature = "rustls-aws-lc-native",
            feature = "rustls-ring-native",
            feature = "native-tls"
        ))]
        let host = request
            .uri()
            .host()
            .ok_or_else(|| invalid_input_error("No host provided"))?;

        let scheme = request.uri().scheme().ok_or_else(|| {
            invalid_input_error(format!("A URI scheme must be set, found {}", request.uri()))
        })?;

        if *scheme == Scheme::HTTP {
            let addresses = get_and_validate_socket_addresses(request.uri(), 80)?;
            let stream = self.connect(&addresses)?;
            let stream =
                encode_request(request, BufWriter::with_capacity(BUFFER_CAPACITY, stream))?
                    .into_inner()
                    .map_err(|e| e.into_error())?;
            return decode_response(BufReader::with_capacity(BUFFER_CAPACITY, stream));
        }

        #[cfg(feature = "native-tls")]
        if *scheme == Scheme::HTTPS {
            static TLS_CONNECTOR: OnceLock<TlsConnector> = OnceLock::new();

            let addresses = get_and_validate_socket_addresses(request.uri(), 443)?;
            let stream = self.connect(&addresses)?;
            let stream = TLS_CONNECTOR
                .get_or_init(|| match TlsConnector::new() {
                    Ok(connector) => connector,
                    Err(e) => panic!("Error while loading TLS configuration: {}", e), // TODO: use get_or_try_init
                })
                .connect(host, stream)
                .map_err(|e| Error::new(ErrorKind::Other, e))?;
            let stream =
                encode_request(request, BufWriter::with_capacity(BUFFER_CAPACITY, stream))?
                    .into_inner()
                    .map_err(|e| e.into_error())?;
            return decode_response(BufReader::with_capacity(BUFFER_CAPACITY, stream));
        }
        #[cfg(all(
            any(
                feature = "rustls-aws-lc-webpki",
                feature = "rustls-ring-webpki",
                feature = "rustls-aws-lc-native",
                feature = "rustls-ring-native"
            ),
            not(feature = "native-tls")
        ))]
        if *scheme == Scheme::HTTPS {
            static RUSTLS_CONFIG: OnceLock<Arc<ClientConfig>> = OnceLock::new();

            let rustls_config = RUSTLS_CONFIG.get_or_init(|| {
                #[cfg(any(feature = "rustls-aws-lc-native", feature = "rustls-ring-native"))]
                {
                    Arc::new(ClientConfig::with_platform_verifier())
                }
                #[cfg(all(
                    any(feature = "rustls-aws-lc-webpki", feature = "rustls-ring-webpki"),
                    not(feature = "rustls-aws-lc-native"),
                    not(feature = "rustls-ring-native")
                ))]
                {
                    Arc::new(
                        ClientConfig::builder()
                            .with_root_certificates(RootCertStore {
                                roots: TLS_SERVER_ROOTS.to_vec(),
                            })
                            .with_no_client_auth(),
                    )
                }
            });
            let addresses = get_and_validate_socket_addresses(request.uri(), 443)?;
            let dns_name = ServerName::try_from(host)
                .map_err(invalid_input_error)?
                .to_owned();
            let connection = ClientConnection::new(Arc::clone(rustls_config), dns_name)
                .map_err(|e| Error::new(ErrorKind::Other, e))?;
            let stream = StreamOwned::new(connection, self.connect(&addresses)?);
            let stream =
                encode_request(request, BufWriter::with_capacity(BUFFER_CAPACITY, stream))?
                    .into_inner()
                    .map_err(|e| e.into_error())?;
            return decode_response(BufReader::with_capacity(BUFFER_CAPACITY, stream));
        }

        #[cfg(not(any(
            feature = "rustls-aws-lc-webpki",
            feature = "rustls-ring-webpki",
            feature = "rustls-aws-lc-native",
            feature = "rustls-ring-native",
            feature = "native-tls"
        )))]
        if *scheme == Scheme::HTTPS {
            return Err(invalid_input_error("HTTPS is not supported by the client. You should enable the `native-tls` or `rustls` feature of the `oxhttp` crate"));
        }

        Err(invalid_input_error(format!(
            "Not supported URL scheme: {scheme}"
        )))
    }

    fn connect(&self, addresses: &[SocketAddr]) -> Result<TcpStream> {
        let stream = if let Some(timeout) = self.timeout {
            Self::connect_timeout(addresses, timeout)
        } else {
            TcpStream::connect(addresses)
        }?;
        stream.set_read_timeout(self.timeout)?;
        stream.set_write_timeout(self.timeout)?;
        stream.set_nodelay(true)?;
        Ok(stream)
    }

    fn connect_timeout(addresses: &[SocketAddr], timeout: Duration) -> Result<TcpStream> {
        let mut error = Error::new(
            ErrorKind::InvalidInput,
            "Not able to resolve the provide addresses",
        );
        for address in addresses {
            match TcpStream::connect_timeout(address, timeout) {
                Ok(stream) => return Ok(stream),
                Err(e) => error = e,
            }
        }
        Err(error)
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

fn get_and_validate_socket_addresses(uri: &Uri, default_port: u16) -> Result<Vec<SocketAddr>> {
    let host = uri
        .host()
        .ok_or_else(|| invalid_input_error(format!("No host in request URL {uri}")))?;
    let port = uri.port_u16().unwrap_or(default_port);
    let addresses = (host, port).to_socket_addrs()?.collect::<Vec<_>>();
    for address in &addresses {
        if BAD_PORTS.binary_search(&address.port()).is_ok() {
            return Err(invalid_input_error(format!(
                "The port {} is not allowed for HTTP(S) because it is dedicated to an other use",
                address.port()
            )));
        }
    }
    Ok(addresses)
}

fn join_urls(base: &Uri, relative: &str) -> Result<Uri> {
    Uri::try_from(
        Url::parse(&base.to_string())
            .map_err(|e| {
                Error::new(
                    ErrorKind::InvalidInput,
                    format!("Invalid base URL '{base}': {e}"),
                )
            })?
            .join(relative)
            .map_err(|e| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("Invalid location header URL '{relative}': {e}"),
                )
            })?
            .to_string(),
    )
    .map_err(|e| {
        Error::new(
            ErrorKind::InvalidData,
            format!("Invalid location header URL '{relative}': {e}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::header::CONTENT_TYPE;

    #[test]
    fn test_http_get_ok() -> Result<()> {
        let client = Client::new();
        let response = client.request(
            Request::builder()
                .uri("http://example.com")
                .body(())
                .unwrap(),
        )?;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get(CONTENT_TYPE).unwrap(), "text/html");
        let body = response.into_body().to_string()?;
        assert!(body.contains("<html"));
        Ok(())
    }

    #[test]
    fn test_http_get_ok_with_user_agent_and_timeout() -> Result<()> {
        let client = Client::new()
            .with_user_agent("OxHTTP/1.0")
            .unwrap()
            .with_global_timeout(Duration::from_secs(5));
        let response = client.request(
            Request::builder()
                .uri("http://example.com")
                .body(())
                .unwrap(),
        )?;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get(CONTENT_TYPE).unwrap(), "text/html");
        Ok(())
    }

    #[test]
    fn test_http_get_ok_explicit_port() -> Result<()> {
        let client = Client::new();
        let response = client.request(
            Request::builder()
                .uri("http://example.com:80")
                .body(())
                .unwrap(),
        )?;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get(CONTENT_TYPE).unwrap(), "text/html");
        Ok(())
    }

    #[test]
    fn test_http_wrong_port() {
        let client = Client::new();
        assert!(client
            .request(
                Request::builder()
                    .uri("http://example.com:22")
                    .body(())
                    .unwrap(),
            )
            .is_err());
    }

    #[cfg(any(
        feature = "rustls-aws-lc-webpki",
        feature = "rustls-ring-webpki",
        feature = "rustls-aws-lc-native",
        feature = "rustls-ring-native",
        feature = "native-tls"
    ))]
    #[test]
    fn test_https_get_ok() -> Result<()> {
        let client = Client::new();
        let response = client.request(
            Request::builder()
                .uri("https://example.com")
                .body(())
                .unwrap(),
        )?;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get(CONTENT_TYPE).unwrap(), "text/html");
        Ok(())
    }

    #[cfg(not(any(
        feature = "rustls-aws-lc-webpki",
        feature = "rustls-ring-webpki",
        feature = "rustls-aws-lc-native",
        feature = "rustls-ring-native",
        feature = "native-tls"
    )))]
    #[test]
    fn test_https_get_err() {
        let client = Client::new();
        assert!(client
            .request(
                Request::builder()
                    .uri("https://example.com")
                    .body(())
                    .unwrap()
            )
            .is_err());
    }

    #[test]
    fn test_http_get_not_found() -> Result<()> {
        let client = Client::new();
        let response = client.request(
            Request::builder()
                .uri("http://example.com/not_existing")
                .body(())
                .unwrap(),
        )?;
        assert!(matches!(
            response.status(),
            StatusCode::NOT_FOUND | StatusCode::INTERNAL_SERVER_ERROR
        ));
        Ok(())
    }

    #[test]
    fn test_file_get_error() {
        let client = Client::new();
        assert!(client
            .request(
                Request::builder()
                    .uri("file://example.com/not_existing")
                    .body(())
                    .unwrap(),
            )
            .is_err());
    }

    #[cfg(any(
        feature = "rustls-aws-lc-webpki",
        feature = "rustls-ring-webpki",
        feature = "rustls-aws-lc-native",
        feature = "rustls-ring-native",
        feature = "native-tls"
    ))]
    #[test]
    fn test_redirection() -> Result<()> {
        let client = Client::new().with_redirection_limit(5);
        let response = client.request(
            Request::builder()
                .uri("http://wikipedia.org")
                .body(())
                .unwrap(),
        )?;
        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }
}
