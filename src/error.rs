use schemars::JsonSchema;
use serde::Serialize;

/// An error type for an endpoint.
///
/// Can be derived; see [`derive(ApiError)`](derive@crate::ApiError).
pub trait ApiError: Serialize + JsonSchema {
    /// The status code of the error.
    fn status_code(&self) -> http::StatusCode;
}

/// An error that can occur while handling a request.
#[derive(Debug, thiserror::Error)]
pub enum RouterError {
    /// No route matched the request.
    #[error("not found")]
    NotFound,
    /// No endpoint handles the request's HTTP method at that path.
    #[error("method not allowed")]
    MethodNotAllowed {
        /// The methods that are allowed for this path.
        allowed: Vec<http::Method>,
    },
    /// Failed to deserialize path parameters.
    #[error("invalid path parameter")]
    InvalidPath {
        /// The parameter that was invalid, if any.
        field: Option<String>,
        /// The underlying serde error.
        #[source]
        source: serde::de::value::Error,
    },
    /// Failed to deserialize query string.
    #[error("invalid query string")]
    InvalidQueryString {
        /// The field that was invalid, if any.
        field: Option<String>,
        /// The underlying serde error.
        #[source]
        source: serde::de::value::Error,
    },
    /// Failed to read the request body from the client. This can happen if
    /// the client hangs up early.
    #[error("failed to read request body")]
    ReadBody,
    /// Failed to deserialize the request body.
    #[error("invalid request body")]
    InvalidBody {
        /// The field that was invalid, if any.
        field: Option<String>,
        /// The underlying serde error.
        #[source]
        source: serde_json::Error,
    },
    /// Failed to serialize the response body.
    #[error("failed to serialize response")]
    ResponseSerialization(#[source] serde_json::Error),
}

impl RouterError {
    /// The appropriate HTTP status code for this error.
    pub fn status_code(&self) -> http::StatusCode {
        match self {
            Self::NotFound => http::StatusCode::NOT_FOUND,
            Self::MethodNotAllowed { .. } => http::StatusCode::METHOD_NOT_ALLOWED,
            Self::InvalidPath { .. } => http::StatusCode::NOT_FOUND,
            Self::InvalidQueryString { .. } => http::StatusCode::BAD_REQUEST,
            Self::ReadBody => http::StatusCode::BAD_REQUEST,
            Self::InvalidBody { .. } => http::StatusCode::BAD_REQUEST,
            Self::ResponseSerialization(_) => http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// A function that converts a [`RouterError`] into an HTTP response.
pub type ErrorHandler = fn(RouterError) -> http::Response<bytes::Bytes>;

/// The default error handler. Returns a JSON response with the error
/// message and appropriate status code, with the following shape:
///
/// ```json
/// { "error": "method not allowed" }
/// ```
pub fn default_error_handler(err: RouterError) -> http::Response<bytes::Bytes> {
    let status = err.status_code();
    let body = serde_json::json!({ "error": err.to_string() });
    let mut resp = http::Response::builder()
        .status(status)
        .header("content-type", "application/json");

    if let RouterError::MethodNotAllowed { ref allowed } = err {
        let allow = allowed
            .iter()
            .map(|m| m.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        resp = resp.header(http::header::ALLOW, allow);
    }

    resp.body(bytes::Bytes::from(body.to_string())).unwrap()
}
