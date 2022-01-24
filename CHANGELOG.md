# Changelog

## [0.1.4] - 2022-01-24

### Added
- `Server`: It is now possible to use a [Rayon](https://github.com/rayon-rs/rayon) thread pool instead of spawning a new thread on each call.

### Changed
- [Chunk Transfer Encoding](https://httpwg.org/http-core/draft-ietf-httpbis-messaging-latest.html#chunked.encoding) serialization was invalid: the last empty chunk was ending with two line jumps instead of one as expected by the specification.
- `Server`: Thread spawn operation is restarted if it fails.
- `Server`: `text/plain; charset=utf8` media type is now returned on errors instead of the simpler `text/plain`.


## [0.1.3] - 2021-12-05

### Added
- [Rustls](https://github.com/rustls/rustls) usage is now available behind the `rustls` feature (disabled by default).


## [0.1.2] - 2021-11-03

### Added
- Redirections support to the `Client`. By default the client does not follow redirects. The `Client::set_redirection_limit` method allows to set the maximum number of allowed consecutive redirects (0 by default).

### Changed
- `Server`: Do not display a TCP error if the client disconnects without having sent the `Connection: close` header.


## [0.1.1] - 2021-09-30

### Changed
- Fixes a possible DOS attack vector by sending very long headers.


## [0.1.0] - 2021-09-29

### Added
- Basic `Client` and `Server` implementations.