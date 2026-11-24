//! The errors the client returns.

use std::fmt;
use std::time::Duration;

use crate::json::Value;

/// What every method returns: the answer, or an [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

/// The API answered with an error: an error body, or any HTTP status of 400 or more.
#[derive(Clone, Debug, PartialEq)]
pub struct ApiError {
    /// The HTTP status.
    pub status: u16,
    /// The API's error code, such as `invalid_request` or `quota_exceeded`; `http_error` when the answer had none.
    pub code: String,
    /// The API's explanation.
    pub message: String,
    /// The parameter the API names as the cause.
    pub param: Option<String>,
    /// The `X-Request-Id` header. Quote it when you contact support.
    pub request_id: Option<String>,
    /// How long the API asks you to wait: `Retry-After`, or on a 429 the time until `X-Quota-Reset`.
    pub retry_after: Option<Duration>,
    /// The decoded error answer, or [`Value::Null`] when it was not JSON.
    pub body: Value,
}

/// Every error the client returns.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The API answered with an error (boxed, so a `Result` stays small).
    Api(Box<ApiError>),
    /// No answer arrived within the client's `timeout`.
    RequestTimeout(String),
    /// The API could not be reached: DNS, a refused or reset connection, TLS. The source error is kept.
    Network(String, Option<Box<dyn std::error::Error + Send + Sync>>),
    /// The API answered with a success status but a body that is not a JSON object.
    Decode(String),
}

/// The same type under the name every My Geocode SDK uses for its base error.
pub type MyGeocodeError = Error;

impl Error {
    /// The [`ApiError`], when the API answered with one.
    pub fn api(&self) -> Option<&ApiError> {
        match self {
            Error::Api(e) => Some(e.as_ref()),
            _ => None,
        }
    }

    /// The API's error code, when the API answered with an error.
    pub fn code(&self) -> Option<&str> {
        self.api().map(|e| e.code.as_str())
    }

    /// True when no answer arrived within the timeout.
    pub fn is_timeout(&self) -> bool {
        matches!(self, Error::RequestTimeout(_))
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (HTTP {}, {})", self.message, self.status, self.code)
    }
}

impl std::error::Error for ApiError {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Api(e) => write!(f, "API error: {}", e),
            Error::RequestTimeout(m) => write!(f, "request timed out: {}", m),
            Error::Network(m, _) => write!(f, "network error: {}", m),
            Error::Decode(m) => write!(f, "could not read the answer: {}", m),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Api(e) => Some(e.as_ref()),
            Error::Network(_, Some(source)) => Some(source.as_ref()),
            _ => None,
        }
    }
}
