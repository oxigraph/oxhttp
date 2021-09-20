//! Simple HTTP client

use crate::io::{decode_response, encode_request};
use crate::model::{Request, Response};
use crate::utils::invalid_input_error;
#[cfg(feature = "native-tls")]
use native_tls::TlsConnector;
use std::io::{BufReader, BufWriter, Error, ErrorKind, Result};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// A simple HTTP client.
///
/// It aims at following the basic concepts of the [Web Fetch standard](https://fetch.spec.whatwg.org/) without the bits specific to web browsers (context, CORS...).
///
/// Missing: HSTS support, authentication, redirects and keep alive.
///
/// ```
/// use oxhttp::Client;
/// use oxhttp::model::{Request, Method, Status, HeaderName};
///
/// let client = Client::new();
/// let response = client.request(Request::builder(Method::GET,"http://example.com".parse()?).build())?;
/// assert_eq!(response.status(), Status::OK);
/// assert_eq!(response.header(&HeaderName::CONTENT_TYPE).unwrap().as_ref(), b"text/html; charset=UTF-8");
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[allow(missing_copy_implementations)]
#[derive(Default)]
pub struct Client {
    timeout: Option<Duration>,
}

impl Client {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the global timout value (applies to both read, write and connection).
    ///
    /// Set to `None` to wait indefinitely.
    pub fn set_global_timeout(&mut self, timeout: Option<Duration>) {
        self.timeout = timeout;
    }

    pub fn request(&self, request: Request) -> Result<Response> {
        let scheme = request.url().scheme();
        let port = if let Some(port) = request.url().port() {
            port
        } else {
            match scheme {
                "http" => 80,
                "https" => 443,
                _ => {
                    return Err(invalid_input_error(format!(
                        "No port provided for scheme '{}'",
                        scheme
                    )))
                }
            }
        };
        if BAD_PORTS.binary_search(&port).is_ok() {
            return Err(invalid_input_error(format!(
                "The port {} is not allowed for HTTP(S) because it is dedicated to an other use",
                port
            )));
        }

        let host = request
            .url()
            .host_str()
            .ok_or_else(|| invalid_input_error("No host provided"))?;

        match scheme {
            "http" => {
                let mut stream = self.connect((host, port))?;
                encode_request(request, BufWriter::new(&mut stream))?;
                decode_response(BufReader::new(stream))
            }
            "https" => {
                #[cfg(feature = "native-tls")]
                {
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
                scheme
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
