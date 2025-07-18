# Changelog

## [0.3.1] - 2025-06-06

### Changed

- Bump`rustls-platform-verifier` to 0.6.

## [0.3.0] - 2025-02-02

### Changed

- Uses the `http` crate structs to represent the HTTP model (`Request`, `Response`, `StatusCode`...) instead of the ones
  defined by `oxhttp`. Only the `Body` struct is implemented by `oxhttp`.
- The `rustls-*-native` features now rely on the `rustls-platform-verifier` crate to support certificate revocation.
  The now redundant `rustls-*-platform-verifier` features have been removed.

## [0.2.7] - 2024-12-23

### Changed

- Increases read and write buffer sizes to 16kB.
- Bump `rustls-platform-verifier` to v0.5.

## [0.2.6] - 2024-12-09

### Changed

- Bump MSRV to 1.74.
- Set TCP_NODELAY in client and server `TcpStream`s.

## [0.2.5] - 2024-12-06

### Changed

- Makes chunked transfer encoding decoder properly return `Ok(0)` after end.

## [0.2.4] - 2024-11-06

### Changed

- Upgrades to `rustls-platform-verifier` 0.4.

## [0.2.3] - 2024-10-03

### Changed

- Allows setting `HeaderName` in `append_header` and `with_header`.

## [0.2.2] - 2024-09-15

### Changed

- Upgrades to `rustls-native-certs` 0.8.

## [0.2.1] - 2024-08-20

### Added

- `rustls-aws-lc-platform-verifier` and `rustls-ring-platform-verifier` features to use the rustls-platform-verifier
  crate to validate TLS certificates.

## [0.2.0] - 2024-03-23

No change compared to the alpha releases.

## [0.2.0-alpha.4] - 2024-02-02

### Changed

- Upgrades `rustls` to 0.23 and its dependencies to compatible versions.
- Splits the `rustls-native` and `rustls-webkpi` features
  into `rustls-ring-native`, `rustls-ring-webpki`, `rustls-aws-lc-native` and `rustls-aws-lc-webpki` to allow choosing
  which cryptographic library to use.

## [0.2.0-alpha.3] - 2023-12-07

### Changed

- Upgrades `rustls` to 0.22 and its dependencies to compatible versions.
- Relaxes dependency requirements on `flate2`

## [0.2.0-alpha.2] - 2023-11-18

### Added

- `Server.bind` to set a socket the server should listen to.
- `Server.spawn` to spawn the server in a new set of threads and return a handle to it.

### Removed

- `Server.listen` function that is now replaced by `Server.bind(address).spawn().join()`.

### Changed

- Renames `Server.max_num_threads` to `Server.with_max_concurrent_connections`

## [0.2.0-alpha.1] - 2023-09-23

### Added

- When the `flate2` crate is installed, the HTTP client and server are able to decode bodies with `Content-Encoding` set
  to `gzip` and `deflate` (no encoding yet).
- `client` and  `server` features to enable the HTTP client and server. They are both enabled by default.
- `Server::with_max_num_threads` allows to set an upper bound to the number of threads running at the same time.

### Removed

- Rayon-based thread pool.

### Changed

- The `rustls` feature has been split into `rustls-native` and `rustls-webpki` to either rust the platform certificates
  or the ones from the [Common CA Database](https://www.ccadb.org/).
- All the `set_` methods on `Client` and `Server` have been renamed to `with_` and now takes and returns the mutated
  objects by value (builder pattern).
- Upgrades minimum supported Rust version to 1.70.
- Upgrades `webpki-roots` dependency to 0.25.

## [0.1.7] - 2023-08-23

### Changed

- Upgrades `rustls` dependency to 0.21.

## [0.1.6] - 2023-03-18

### Added

- `IntoHeaderName` trait that allows to call methods with plain strings instead of explicit `HeaderName` objects.
- `client` and `server` features to enable/disable the HTTP client and/or server (both features are enabled by default).

### Changed

- Bindings to server localhost now properly binds to both IPv4 and IPv6 at the same time.
- Set minimum supported Rust version to 1.60.

## [0.1.5] - 2022-08-16

### Changed

- A body is now always written on POST and PUT request and on response that have not the status 1xx, 204 and 304.
  This allows clients to not wait for an existing body in case the connection is kept alive.
- The TLS configuration is now initialized once and shared between clients and saved during the complete process
  lifetime.

## [0.1.4] - 2022-01-24

### Added

- `Server`: It is now possible to use a [Rayon](https://github.com/rayon-rs/rayon) thread pool instead of spawning a new
  thread on each call.

### Changed

- [Chunk Transfer Encoding](https://httpwg.org/http-core/draft-ietf-httpbis-messaging-latest.html#chunked.encoding)
  serialization was invalid: the last empty chunk was ending with two line jumps instead of one as expected by the
  specification.
- `Server`: Thread spawn operation is restarted if it fails.
- `Server`: `text/plain; charset=utf8` media type is now returned on errors instead of the simpler `text/plain`.

## [0.1.3] - 2021-12-05

### Added

- [Rustls](https://github.com/rustls/rustls) usage is now available behind the `rustls` feature (disabled by default).

## [0.1.2] - 2021-11-03

### Added

- Redirections support to the `Client`. By default the client does not follow redirects.
  The `Client::set_redirection_limit` method allows to set the maximum number of allowed consecutive redirects (0 by
  default).

### Changed

- `Server`: Do not display a TCP error if the client disconnects without having sent the `Connection: close` header.

## [0.1.1] - 2021-09-30

### Changed

- Fixes a possible DOS attack vector by sending very long headers.

## [0.1.0] - 2021-09-29

### Added

- Basic `Client` and `Server` implementations.
