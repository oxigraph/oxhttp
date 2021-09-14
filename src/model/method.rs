use std::borrow::Borrow;
use std::convert::{TryFrom, TryInto};
use std::error::Error;
use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

/// An [HTTP method](https://httpwg.org/http-core/draft-ietf-httpbis-semantics-latest.html#methods) like `GET` or `POST`.
///
/// ```
/// use oxhttp::model::Method;
/// use std::str::FromStr;
///
/// assert_eq!(Method::GET, Method::from_str("get")?);
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(PartialEq, Eq, Debug, Clone, Hash)]
pub struct Method(MethodAlt);

impl Method {
    /// [CONNECT](https://httpwg.org/http-core/draft-ietf-httpbis-semantics-latest.html#CONNECT).
    pub const CONNECT: Method = Self(MethodAlt::Connect);
    /// [DELETE](https://httpwg.org/http-core/draft-ietf-httpbis-semantics-latest.html#DELETE).
    pub const DELETE: Method = Self(MethodAlt::Delete);
    /// [GET](https://httpwg.org/http-core/draft-ietf-httpbis-semantics-latest.html#GET).
    pub const GET: Method = Self(MethodAlt::Get);
    /// [HEAD](https://httpwg.org/http-core/draft-ietf-httpbis-semantics-latest.html#HEAD).
    pub const HEAD: Method = Self(MethodAlt::Head);
    /// [OPTIONS](https://httpwg.org/http-core/draft-ietf-httpbis-semantics-latest.html#OPTIONS).
    pub const OPTIONS: Method = Self(MethodAlt::Options);
    /// [POST](https://httpwg.org/http-core/draft-ietf-httpbis-semantics-latest.html#POST).
    pub const POST: Method = Self(MethodAlt::Post);
    /// [PUT](https://httpwg.org/http-core/draft-ietf-httpbis-semantics-latest.html#PUT).
    pub const PUT: Method = Self(MethodAlt::Put);
    /// [TRACE](https://httpwg.org/http-core/draft-ietf-httpbis-semantics-latest.html#TRACE).
    pub const TRACE: Method = Self(MethodAlt::Trace);
}

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
enum MethodAlt {
    Connect,
    Delete,
    Get,
    Head,
    Options,
    Post,
    Put,
    Trace,
    Other(String),
}

impl Deref for Method {
    type Target = str;

    fn deref(&self) -> &str {
        match &self.0 {
            MethodAlt::Connect => "CONNECT",
            MethodAlt::Delete => "DELETE",
            MethodAlt::Get => "GET",
            MethodAlt::Head => "HEAD",
            MethodAlt::Options => "OPTIONS",
            MethodAlt::Post => "POST",
            MethodAlt::Put => "PUT",
            MethodAlt::Trace => "TRACE",
            MethodAlt::Other(method) => method,
        }
    }
}

impl AsRef<str> for Method {
    fn as_ref(&self) -> &str {
        self.deref()
    }
}

impl Borrow<str> for Method {
    fn borrow(&self) -> &str {
        self.deref()
    }
}

impl FromStr for Method {
    type Err = InvalidMethod;

    fn from_str(name: &str) -> Result<Self, InvalidMethod> {
        if name.eq_ignore_ascii_case("CONNECT") {
            Ok(Self(MethodAlt::Connect))
        } else if name.eq_ignore_ascii_case("DELETE") {
            Ok(Self(MethodAlt::Delete))
        } else if name.eq_ignore_ascii_case("GET") {
            Ok(Self(MethodAlt::Get))
        } else if name.eq_ignore_ascii_case("HEAD") {
            Ok(Self(MethodAlt::Head))
        } else if name.eq_ignore_ascii_case("OPTIONS") {
            Ok(Self(MethodAlt::Options))
        } else if name.eq_ignore_ascii_case("POST") {
            Ok(Self(MethodAlt::Post))
        } else if name.eq_ignore_ascii_case("PUT") {
            Ok(Self(MethodAlt::Put))
        } else if name.eq_ignore_ascii_case("TRACE") {
            Ok(Self(MethodAlt::Trace))
        } else {
            name.to_owned().try_into()
        }
    }
}

impl TryFrom<String> for Method {
    type Error = InvalidMethod;

    fn try_from(name: String) -> Result<Self, InvalidMethod> {
        if name.eq_ignore_ascii_case("CONNECT") {
            Ok(Self(MethodAlt::Connect))
        } else if name.eq_ignore_ascii_case("DELETE") {
            Ok(Self(MethodAlt::Delete))
        } else if name.eq_ignore_ascii_case("GET") {
            Ok(Self(MethodAlt::Get))
        } else if name.eq_ignore_ascii_case("HEAD") {
            Ok(Self(MethodAlt::Head))
        } else if name.eq_ignore_ascii_case("OPTIONS") {
            Ok(Self(MethodAlt::Options))
        } else if name.eq_ignore_ascii_case("POST") {
            Ok(Self(MethodAlt::Post))
        } else if name.eq_ignore_ascii_case("PUT") {
            Ok(Self(MethodAlt::Put))
        } else if name.eq_ignore_ascii_case("TRACE") {
            Ok(Self(MethodAlt::Trace))
        } else if name.is_empty() {
            Err(InvalidMethod(InvalidMethodAlt::Empty))
        } else {
            for c in name.chars() {
                if !matches!(c, '!' | '#' | '$' | '%' | '&' | '\'' | '*'
       | '+' | '-' | '.' | '^' | '_' | '`' | '|' | '~'
        | '0'..='9' | 'a'..='z')
                {
                    return Err(InvalidMethod(InvalidMethodAlt::InvalidChar {
                        name: name.to_owned(),
                        invalid_char: c,
                    }));
                }
            }
            Ok(Self(MethodAlt::Other(name)))
        }
    }
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

/// Error returned by [`Method::try_from`].
#[derive(Debug, Clone)]
pub struct InvalidMethod(InvalidMethodAlt);

#[derive(Debug, Clone)]
enum InvalidMethodAlt {
    Empty,
    InvalidChar { name: String, invalid_char: char },
}

impl fmt::Display for InvalidMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            InvalidMethodAlt::Empty => f.write_str("HTTP methods should not be empty"),
            InvalidMethodAlt::InvalidChar { name, invalid_char } => write!(
                f,
                "The character '{}' is not valid inside of HTTP method '{}'",
                invalid_char, name
            ),
        }
    }
}

impl Error for InvalidMethod {}
