OxHTTP
======

[![actions status](https://github.com/oxigraph/oxhttp/workflows/build/badge.svg)](https://github.com/oxigraph/oxhttp/actions)
[![Latest Version](https://img.shields.io/crates/v/oxhttp.svg)](https://crates.io/crates/oxhttp)
[![Released API docs](https://docs.rs/oxhttp/badge.svg)](https://docs.rs/oxhttp)

OxHTTP is a small synchronous [HTTP/1.1](https://httpwg.org/http-core/) client and server for Rust.
It uses blocking I/O and threads and intentionally implements only a subset of HTTP.

The `client` and `server` Cargo features are enabled by default.
When default features are disabled, either can be enabled independently.

Optional features:

- `flate2`: enables automatic decoding of `gzip` and `deflate` response bodies in the client.
- `native-tls`: enables HTTPS using the platform's native TLS implementation.
- `rustls-ring-native`: enables HTTPS using [Rustls](https://github.com/rustls/rustls), [Ring](https://github.com/briansmith/ring), and the platform's certificate store.
- `rustls-ring-webpki`: enables HTTPS using [Rustls](https://github.com/rustls/rustls), [Ring](https://github.com/briansmith/ring), and the [Common CA Database](https://www.ccadb.org/).
  provided by `webpki-roots`.
- `rustls-aws-lc-native`: enables HTTPS using [Rustls](https://github.com/rustls/rustls), [AWS Libcrypto for Rust](https://github.com/aws/aws-lc-rs), and the platform's certificate store.
- `rustls-aws-lc-webpki`: enables HTTPS using [Rustls](https://github.com/rustls/rustls), [AWS Libcrypto for Rust](https://github.com/aws/aws-lc-rs), and the [Common CA Database](https://www.ccadb.org/).
  provided by `webpki-roots`.

## Client

OxHTTP provides [a client](https://docs.rs/oxhttp/latest/oxhttp/struct.Client.html) based on the core concepts of the
[Fetch standard](https://fetch.spec.whatwg.org/), excluding browser-specific behavior such as CORS and browsing
contexts.

HTTPS requires one of the optional TLS features listed above. Redirects are not followed by default; use
[`Client::with_redirection_limit`](https://docs.rs/oxhttp/latest/oxhttp/struct.Client.html#method.with_redirection_limit)
to enable them. The client does not currently support authentication, HSTS, or persistent connections.

Example:

```rust no_run
use oxhttp::Client;
use oxhttp::model::header::CONTENT_TYPE;
use oxhttp::model::{Body, Request, StatusCode};

let client = Client::new();
let response = client
    .request(
        Request::builder()
            .uri("http://example.com")
            .body(Body::empty())
            .unwrap(),
    )
    .unwrap();
assert_eq!(response.status(), StatusCode::OK);
assert_eq!(response.headers().get(CONTENT_TYPE).unwrap(), "text/html");

let _body = response.into_body().to_string().unwrap();
```

## Server

OxHTTP provides [a threaded HTTP server](https://docs.rs/oxhttp/latest/oxhttp/struct.Server.html).
The server starts one thread per connection. It is still a work in progress; use it with care and place it behind a
reverse proxy when exposing it to untrusted traffic.

Example:

```rust no_run
use oxhttp::Server;
use oxhttp::model::{Body, Response, StatusCode};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::time::Duration;

// Return "home" for "/" and 404 for every other path.
let mut server = Server::new(|request| {
    if request.uri().path() == "/" {
        Response::builder().body(Body::from("home")).unwrap()
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap()
    }
});

// Bind to localhost over both IPv4 and IPv6.
server = server
    .bind((Ipv4Addr::LOCALHOST, 8080))
    .bind((Ipv6Addr::LOCALHOST, 8080));
// Raise a timeout error if the client does not respond within 10 seconds.
server = server.with_global_timeout(Duration::from_secs(10));
// Limit the number of concurrent connections to 128.
server = server.with_max_concurrent_connections(128);
// Spawn the server and wait for it to terminate.
server.spawn().unwrap().join().unwrap();
```

## License

This project is licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
  `<http://www.apache.org/licenses/LICENSE-2.0>`)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or
  `<http://opensource.org/licenses/MIT>`)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in OxHTTP by you, as
defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
