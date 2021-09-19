use crate::io::encode_response;
use crate::io::{decode_request_body, decode_request_headers};
use crate::model::{HeaderName, Request, Response, Status};
use std::io::{BufReader, BufWriter, Error, ErrorKind, Result, Write};
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
///         Response::new(Status::OK).with_body("home")
///     } else {
///         Response::new(Status::NOT_FOUND)
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
    on_request: Arc<dyn Fn(Request) -> Response + Send + Sync + 'static>,
    timeout: Option<Duration>,
}

impl Server {
    /// Builds the server using the given `on_request` method that builds a `Response` from a given `Request`.
    pub fn new(on_request: impl Fn(Request) -> Response + Send + Sync + 'static) -> Self {
        Self {
            on_request: Arc::new(on_request),
            timeout: None,
        }
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
                    spawn(move || {
                        if let Err(error) = accept_request(stream, on_request, timeout) {
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
    on_request: Arc<dyn Fn(Request) -> Response>,
    timeout: Option<Duration>,
) -> Result<()> {
    stream.set_read_timeout(timeout)?;
    stream.set_write_timeout(timeout)?;
    let mut close = false;
    while !close {
        let mut reader = BufReader::new(stream.try_clone()?);
        let response = match decode_request_headers(&mut reader, false) {
            Ok(request) => {
                // handle close
                close = request
                    .headers()
                    .get(&HeaderName::CONNECTION)
                    .map_or(false, |v| v.eq_ignore_ascii_case(b"close"));
                // Handles Expect header
                if let Some(expect) = request.headers().get(&HeaderName::EXPECT).cloned() {
                    if expect.eq_ignore_ascii_case(b"100-continue") {
                        stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n")?;
                        match decode_request_body(request, reader) {
                            Ok(request) => on_request(request),
                            Err(error) => build_error(error, Status::BAD_REQUEST),
                        }
                    } else {
                        build_error(
                            Error::new(
                                ErrorKind::Other,
                                format!(
                                    "Expect header value '{}' is not supported",
                                    String::from_utf8_lossy(expect.as_ref())
                                ),
                            ),
                            Status::EXPECTATION_FAILED,
                        )
                    }
                } else {
                    match decode_request_body(request, reader) {
                        Ok(request) => on_request(request),
                        Err(error) => build_error(error, Status::BAD_REQUEST),
                    }
                }
            }
            Err(error) => build_error(error, Status::BAD_REQUEST),
        };
        encode_response(response, BufWriter::new(&mut stream))?;
    }
    Ok(())
}

fn build_error(error: Error, other_kind_status: Status) -> Response {
    Response::new(match error.kind() {
        ErrorKind::TimedOut => Status::REQUEST_TIMEOUT,
        ErrorKind::InvalidData => Status::BAD_REQUEST,
        _ => other_kind_status,
    })
    .with_body(error.to_string())
}
