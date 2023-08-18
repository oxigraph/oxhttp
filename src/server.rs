use crate::io::encode_response;
use crate::io::{decode_request_body, decode_request_headers};
use crate::model::{
    HeaderName, HeaderValue, InvalidHeader, Request, RequestBuilder, Response, Status,
};
use std::io::{copy, sink, BufReader, BufWriter, Error, ErrorKind, Result, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::thread::{self, Builder};
use std::time::Duration;

/// An HTTP server.
///
/// It uses a very simple threading mechanism: a new thread is started on each connection and kept while the client connection is not closed.
///
/// ```no_run
/// use oxhttp::Server;
/// use oxhttp::model::{Response, Status};
/// use std::time::Duration;
///
/// // Builds a new server that returns a 404 everywhere except for "/" where it returns the body 'home'
/// let mut server = Server::new(|request| {
///     if request.url().path() == "/" {
///         Response::builder(Status::OK).with_body("home")
///     } else {
///         Response::builder(Status::NOT_FOUND).build()
///     }
/// });
/// // Raise a timeout error if the client does not respond after 10s.
/// server.set_global_timeout(Duration::from_secs(10));
/// // Listen to localhost:8080
/// server.listen(("localhost", 8080))?;
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[allow(missing_copy_implementations)]
pub struct Server {
    on_request: Box<dyn Fn(&mut Request) -> Response + Send + Sync + 'static>,
    timeout: Option<Duration>,
    server: Option<HeaderValue>,
}

impl Server {
    /// Builds the server using the given `on_request` method that builds a `Response` from a given `Request`.
    #[inline]
    pub fn new(on_request: impl Fn(&mut Request) -> Response + Send + Sync + 'static) -> Self {
        Self {
            on_request: Box::new(on_request),
            timeout: None,
            server: None,
        }
    }

    /// Sets the default value for the [`Server`](https://httpwg.org/http-core/draft-ietf-httpbis-semantics-latest.html#field.server) header.
    #[inline]
    pub fn set_server_name(
        &mut self,
        server: impl Into<String>,
    ) -> std::result::Result<(), InvalidHeader> {
        self.server = Some(HeaderValue::try_from(server.into())?);
        Ok(())
    }

    /// Sets the global timeout value (applies to both read and write).
    #[inline]
    pub fn set_global_timeout(&mut self, timeout: Duration) {
        self.timeout = Some(timeout);
    }

    /// Runs the server by listening to `address`.
    pub fn listen(&self, address: impl ToSocketAddrs) -> Result<()> {
        thread::scope(|scope| {
            let timeout = self.timeout;
            let threads = open_tcp(address)?
                .into_iter()
                .map(|listener| {
                    let listener_addr = listener.local_addr()?;
                    let thread_name = format!("{}: listener thread of OxHTTP", listener_addr);
                    Builder::new().name(thread_name).spawn_scoped(scope, move || {
                        for stream in listener.incoming() {
                            match stream {
                                Ok(stream) => {
                                    let peer_addr = match stream.peer_addr() {
                                        Ok(peer) => peer,
                                        Err(error) => {
                                            eprintln!("OxHTTP TCP error when attempting to get the peer address: {error}");
                                            continue;
                                        }
                                    };
                                    let thread_name = format!("{}: responding thread of OxHTTP", peer_addr);
                                    if let Err(error) = Builder::new().name(thread_name).spawn_scoped(scope, move || {
                                        if let Err(error) =
                                            accept_request(stream, &*self.on_request, timeout, &self.server)
                                        {
                                            eprintln!(
                                                "OxHTTP TCP error when writing response to {peer_addr}: {error}"
                                            )
                                        }
                                    }) {
                                        eprintln!("OxHTTP thread spawn error: {error}");
                                    }
                                }
                                Err(error) => {
                                    eprintln!("OxHTTP TCP error when opening stream: {error}");
                                }
                            }
                        }
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            for thread in threads {
                thread
                    .join()
                    .map_err(|_| Error::new(ErrorKind::Other, "The server thread panicked"))?;
            }
            Ok(())
        })
    }
}

fn open_tcp(address: impl ToSocketAddrs) -> Result<Vec<TcpListener>> {
    let mut listeners = Vec::new();
    let mut last_error = None;
    for address in address.to_socket_addrs()? {
        match TcpListener::bind(address) {
            Ok(listener) => listeners.push(listener),
            Err(e) => last_error = Some(e),
        }
    }
    if listeners.is_empty() {
        Err(last_error.unwrap_or_else(|| {
            Error::new(
                ErrorKind::InvalidInput,
                "could not resolve to any addresses",
            )
        }))
    } else {
        Ok(listeners)
    }
}

fn accept_request(
    mut stream: TcpStream,
    on_request: &dyn Fn(&mut Request) -> Response,
    timeout: Option<Duration>,
    server: &Option<HeaderValue>,
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
                        read_body_and_build_response(request, reader, on_request)
                    } else {
                        (
                            build_text_response(
                                Status::EXPECTATION_FAILED,
                                format!(
                                    "Expect header value '{}' is not supported.",
                                    String::from_utf8_lossy(expect.as_ref())
                                ),
                            ),
                            ConnectionState::Close,
                        )
                    }
                } else {
                    read_body_and_build_response(request, reader, on_request)
                }
            }
            Err(error) => {
                if error.kind() == ErrorKind::ConnectionAborted {
                    return Ok(()); // The client is disconnected. Let's ignore this error and do not try to write an answer that won't be received.
                } else {
                    (build_error(error), ConnectionState::Close)
                }
            }
        };
        connection_state = new_connection_state;

        // Additional headers
        if let Some(server) = server {
            if !response.headers().contains(&HeaderName::SERVER) {
                response
                    .headers_mut()
                    .set(HeaderName::SERVER, server.clone())
            }
        }

        stream = encode_response(&mut response, BufWriter::new(stream))?
            .into_inner()
            .map_err(|e| e.into_error())?;
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
                (build_error(error), ConnectionState::Close) //TODO: ignore?
            } else {
                let connection_state = request
                    .header(&HeaderName::CONNECTION)
                    .and_then(|v| {
                        v.eq_ignore_ascii_case(b"close")
                            .then_some(ConnectionState::Close)
                    })
                    .unwrap_or(ConnectionState::KeepAlive);
                (response, connection_state)
            }
        }
        Err(error) => (build_error(error), ConnectionState::Close),
    }
}

fn build_error(error: Error) -> Response {
    build_text_response(
        match error.kind() {
            ErrorKind::TimedOut => Status::REQUEST_TIMEOUT,
            ErrorKind::InvalidData => Status::BAD_REQUEST,
            _ => Status::INTERNAL_SERVER_ERROR,
        },
        error.to_string(),
    )
}

fn build_text_response(status: Status, text: String) -> Response {
    Response::builder(status)
        .with_header(HeaderName::CONTENT_TYPE, "text/plain; charset=utf-8")
        .unwrap()
        .with_body(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;
    use std::io::Read;
    use std::thread::{sleep, spawn};

    #[test]
    fn test_regular_http_operations() -> Result<()> {
        test_server("localhost", 9999, [
            "GET / HTTP/1.1\nhost: localhost:9999\n\n",
            "POST /foo HTTP/1.1\nhost: localhost:9999\nexpect: 100-continue\nconnection:close\ncontent-length:4\n\nabcd",
        ], [
            "HTTP/1.1 200 OK\r\nserver: OxHTTP/1.0\r\ncontent-length: 4\r\n\r\nhome",
            "HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 404 Not Found\r\nserver: OxHTTP/1.0\r\ncontent-length: 0\r\n\r\n"
        ])
    }

    #[test]
    fn test_bad_request() -> Result<()> {
        test_server(
            "::1", 9998,
            ["GET / HTTP/1.1\nhost: localhost:9999\nfoo\n\n"],
            ["HTTP/1.1 400 Bad Request\r\ncontent-type: text/plain; charset=utf-8\r\nserver: OxHTTP/1.0\r\ncontent-length: 19\r\n\r\ninvalid header name"],
        )
    }

    #[test]
    fn test_bad_expect() -> Result<()> {
        test_server(
            "127.0.0.1", 9997,
            ["GET / HTTP/1.1\nhost: localhost:9999\nexpect: bad\n\n"],
            ["HTTP/1.1 417 Expectation Failed\r\ncontent-type: text/plain; charset=utf-8\r\nserver: OxHTTP/1.0\r\ncontent-length: 43\r\n\r\nExpect header value 'bad' is not supported."],
        )
    }

    fn test_server(
        request_host: &'static str,
        server_port: u16,
        requests: impl IntoIterator<Item = &'static str>,
        responses: impl IntoIterator<Item = &'static str>,
    ) -> Result<()> {
        spawn(move || {
            let mut server = Server::new(|request| {
                if request.url().path() == "/" {
                    Response::builder(Status::OK).with_body("home")
                } else {
                    Response::builder(Status::NOT_FOUND).build()
                }
            });
            server.set_server_name("OxHTTP/1.0").unwrap();
            server.set_global_timeout(Duration::from_secs(1));
            server.listen(("localhost", server_port)).unwrap();
        });
        sleep(Duration::from_millis(100)); // Makes sure the server is up
        let mut stream = TcpStream::connect((request_host, server_port))?;
        for (request, response) in requests.into_iter().zip(responses) {
            stream.write_all(request.as_bytes())?;
            let mut output = vec![b'\0'; response.len()];
            stream.read_exact(&mut output)?;
            assert_eq!(String::from_utf8(output).unwrap(), response);
        }
        Ok(())
    }
}
