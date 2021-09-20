OxHTTP
======

[![actions status](https://github.com/oxigraph/oxhttp/workflows/build/badge.svg)](https://github.com/oxigraph/oxhttp/actions)
[![Latest Version](https://img.shields.io/crates/v/oxhttp.svg)](https://crates.io/crates/oxhttp)
[![Released API docs](https://docs.rs/oxhttp/badge.svg)](https://docs.rs/oxhttp)

OxHTTP is a very simple synchronous implementation of an [HTTP 1.1](https://httpwg.org/http-core/) client and server in Rust.


## Client

OxHTTP provides [a very simple client](https://docs.rs/oxhttp/latest/oxhttp/struct.Client.html).
It aims at following the basic concepts of the [Web Fetch standard](https://fetch.spec.whatwg.org/) without the bits specific to web browsers (context, CORS...).

Example:
```rust
let client = Client::new();
let response = client.request(Request::new(Method::GET, "http://example.com".parse()?))?;
```

## Server

OxHTTP provides [a very simple threaded HTTP server](https://docs.rs/oxhttp/latest/oxhttp/struct.Server.html).
It is still a work in progress. Use at your own risks!

Example:
```rust
// Builds a new server that returns a 404 everywhere except for "/" where it returns the body 'home' "/
let mut server = Server::new(|request| {
    if request.url().path() == "/" {
        Response::new(Status::OK).with_body("home")
    } else {
        Response::new(Status::NOT_FOUND)
    }
});
// Raise a timeout error if the client does not respond after 10s.
server.set_global_timeout(Some(Duration::from_secs(10)));
// Listen to localhost:8080
server.listen(("localhost", 8080))?;
```

## License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)
   
at your option.


### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Futures by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
