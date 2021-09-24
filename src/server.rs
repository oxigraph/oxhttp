use crate::io::encode_response;
use crate::io::{decode_request_body, decode_request_headers};
use crate::model::{
    HeaderName, HeaderValue, InvalidHeader, Request, RequestBuilder, Response, Status,
};
use std::convert::TryFrom;
use std::io::{copy, sink, BufReader, BufWriter, Error, ErrorKind, Result, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::thread::spawn;
use std::time::Duration;

/// A simple HTTP server.
///
/// Warning: It currently starts a new thread on each connection and keep them open while the client connection is not closed.
/// Use it at our own risks!
///
/// ```no_run
/// use oxhttp::Server;
/// use oxhttp::model::{Response, Status};
/// use std::time::Duration;
///
/// // Builds a new server that returns a 404 everywhere except for "/" where it returns the body 'home' "/
/// let mut server = Server::new(|request| {
///     if request.url().path() == "/" {
///         Response::builder(Status::OK).with_body("home")
///     } else {
///         Response::builder(Status::NOT_FOUND).build()
///     }
/// });
/// // Raise a timeout error if the client does not respond after 10s.
/// server.set_global_timeout(Some(Duration::from_secs(10)));
/// // Listen to localhost:8080
/// server.listen(("localhost", 8080))?;
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[allow(missing_copy_implementations)]
pub struct Server {
    on_request: Arc<dyn Fn(&mut Request) -> Response + Send + Sync + 'static>,
    timeout: Option<Duration>,
    server: Option<HeaderValue>,
}

impl Server {
    /// Builds the server using the given `on_request` method that builds a `Response` from a given `Request`.
    pub fn new(on_request: impl Fn(&mut Request) -> Response + Send + Sync + 'static) -> Self {
        Self {
            on_request: Arc::new(on_request),
            timeout: None,
            server: None,
        }
    }

    /// Sets the default value for the [`Server`](https://httpwg.org/http-core/draft-ietf-httpbis-semantics-latest.html#field.server) header.
    pub fn set_server(&mut self, server: String) -> std::result::Result<(), InvalidHeader> {
        self.server = Some(HeaderValue::try_from(server)?);
        Ok(())
    }

    /// Sets the global timout value (applies to both read and write).
    ///
    /// Set to `None` to wait indefinitely.
    pub fn set_global_timeout(&mut self, timeout: Option<Duration>) {
        self.timeout = timeout;
    }

    /// Runs the server
    pub fn listen(&self, address: impl ToSocketAddrs) -> Result<()> {
        //TODO: socket timeout
        for stream in TcpListener::bind(address)?.incoming() {
            match stream {
                Ok(stream) => {
                    let on_request = self.on_request.clone();
                    let timeout = self.timeout;
                    let server = self.server.clone();
                    spawn(move || {
                        if let Err(error) = accept_request(stream, on_request, timeout, server) {
                            eprint!("TCP error when writing response: {}", error);
                        }
                    });
                }
                Err(error) => {
                    eprint!("TCP error when opening stream: {}", error);
                }
            }
        }
        Ok(())
    }
}

fn accept_request(
    mut stream: TcpStream,
    on_request: Arc<dyn Fn(&mut Request) -> Response>,
    timeout: Option<Duration>,
    server: Option<HeaderValue>,
) -> Result<()> {
    stream.set_read_timeout(timeout)?;
    stream.set_write_timeout(timeout)?;
    let mut connection_state = ConnectionState::KeepAlive;
    while connection_state == ConnectionState::KeepAlive {
        let mut reader = BufReader::new(stream.try_clone()?);
        let (mut response, new_connection_state) = match decode_request_headers(&mut reader, false)
        {
            Ok(request) => {
                // Handles Expect header
                if let Some(expect) = request.header(&HeaderName::EXPECT).cloned() {
                    if expect.eq_ignore_ascii_case(b"100-continue") {
                        stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n")?;
                        read_body_and_build_response(request, reader, on_request.as_ref())
                    } else {
                        (
                            build_error(
                                Error::new(
                                    ErrorKind::Other,
                                    format!(
                                        "Expect header value '{}' is not supported",
                                        String::from_utf8_lossy(expect.as_ref())
                                    ),
                                ),
                                Status::EXPECTATION_FAILED,
                            ),
                            ConnectionState::Close,
                        )
                    }
                } else {
                    read_body_and_build_response(request, reader, on_request.as_ref())
                }
            }
            Err(error) => (
                build_error(error, Status::BAD_REQUEST),
                ConnectionState::Close,
            ),
        };
        connection_state = new_connection_state;

        // Additional headers
        set_header_fallback(&mut response, HeaderName::SERVER, &server);

        encode_response(response, BufWriter::new(&mut stream))?;
    }
    Ok(())
}

#[derive(Eq, PartialEq, Debug, Copy, Clone)]
enum ConnectionState {
    Close,
    KeepAlive,
}

fn read_body_and_build_response(
    request: RequestBuilder,
    reader: BufReader<TcpStream>,
    on_request: &dyn Fn(&mut Request) -> Response,
) -> (Response, ConnectionState) {
    match decode_request_body(request, reader) {
        Ok(mut request) => {
            let response = on_request(&mut request);
            // We make sure to finish reading the body
            if let Err(error) = copy(request.body_mut(), &mut sink()) {
                (
                    build_error(error, Status::BAD_REQUEST),
                    ConnectionState::Close,
                ) //TODO: ignore?
            } else {
                let connection_state = request
                    .header(&HeaderName::CONNECTION)
                    .and_then(|v| {
                        v.eq_ignore_ascii_case(b"close")
                            .then(|| ConnectionState::Close)
                    })
                    .unwrap_or(ConnectionState::KeepAlive);
                (response, connection_state)
            }
        }
        Err(error) => (
            build_error(error, Status::BAD_REQUEST),
            ConnectionState::Close,
        ),
    }
}

fn build_error(error: Error, other_kind_status: Status) -> Response {
    Response::builder(match error.kind() {
        ErrorKind::TimedOut => Status::REQUEST_TIMEOUT,
        ErrorKind::InvalidData => Status::BAD_REQUEST,
        _ => other_kind_status,
    })
    .with_body(error.to_string())
}

fn set_header_fallback(
    response: &mut Response,
    header_name: HeaderName,
    header_value: &Option<HeaderValue>,
) {
    if let Some(header_value) = header_value {
        if !response.headers().contains(&header_name) {
            response
                .headers_mut()
                .set(header_name, header_value.clone())
        }
    }
}
