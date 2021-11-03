# Changelog

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