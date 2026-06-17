OxHTTP is a Rust crate implementing an HTTP 1.1 client and server.
It does not rely on async I/O but blocking I/Os and Rust threads.

It provides TLS support only for the client. To enable it use the related cargo features.
