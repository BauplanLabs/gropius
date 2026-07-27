//! Runtime support for generated API clients.
use bytes::Bytes;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde::ser::Error as _;

// Reexports used by the generated code.
pub use http;
pub use serde_json;

use crate::{Response, path};

// Re-exported so the request-building helpers and `RequestError` can name the
// path serializer's output and error without exposing `crate::path`.
pub use crate::path::ser::{Error as PathError, PathParams};

/// An error returned by a generated client method.
#[derive(Debug, thiserror::Error)]
pub enum ClientError<E> {
    /// The server returned an application error.
    #[error(transparent)]
    Api(E),
    /// The server returned an error status, but the body did not match the
    /// endpoint's error type.
    #[error("server returned unexpected status {status}")]
    Unexpected {
        /// The response status code.
        status: http::StatusCode,
        /// The raw response body.
        body: Bytes,
    },
    /// The request could not be built.
    #[error("failed to build request")]
    InvalidRequest(#[from] RequestError),
    /// The underlying HTTP client failed, e.g. the connection was refused.
    #[error("transport error")]
    Transport(#[from] TransportError),
    /// A successful response body could not be deserialized.
    #[error("failed to deserialize response")]
    Deserialize(#[from] serde_json::Error),
}

/// An error building a request from an endpoint's arguments.
#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    /// Failed to build request path.
    #[error("invalid path parameter: {0}")]
    Path(PathError),
    /// The query parameters failed to serialize.
    #[error("failed to serialize query string")]
    Query(#[from] serde_urlencoded::ser::Error),
    /// The request body failed to serialize.
    #[error("failed to serialize request body")]
    Body(#[from] serde_json::Error),
}

#[cfg(feature = "client-reqwest")]
pub use reqwest_impl::{
    AsyncClient, AsyncTransport, BlockingClient, BlockingTransport, TransportError,
};

#[cfg(all(feature = "client-ureq", not(feature = "client-reqwest")))]
pub use ureq_impl::{BlockingClient, BlockingTransport, TransportError};

// https://www.rfc-editor.org/info/rfc3986/#section-2.3
const PATH_SEGMENT: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

pub fn path_params<T: Serialize>(value: &T) -> Result<PathParams, RequestError> {
    value
        .serialize(path::ser::PathSerializer)
        .map_err(RequestError::Path)
}

/// Look up a serialized path parameter by name (a struct field) or position (a
/// tuple or bare scalar) and percent-encode it as a single path segment.
pub fn segment<'a>(
    params: &'a PathParams,
    name: &str,
    index: usize,
) -> Result<percent_encoding::PercentEncode<'a>, RequestError> {
    let value = params.get(name, index).ok_or_else(|| {
        RequestError::Path(PathError::custom(format!(
            "no value for path parameter `{name}`"
        )))
    })?;

    Ok(percent_encoding::utf8_percent_encode(value, PATH_SEGMENT))
}

/// Append a serialized query string to the URI, if it isn't empty.
pub fn append_query<T: Serialize>(uri: &mut String, query: &T) -> Result<(), RequestError> {
    let encoded = serde_urlencoded::to_string(query).map_err(RequestError::Query)?;
    if !encoded.is_empty() {
        uri.push('?');
        uri.push_str(&encoded);
    }

    Ok(())
}

/// Serialize a request body as JSON.
pub fn encode_body<T: Serialize>(body: &T) -> Result<Bytes, RequestError> {
    let bytes = serde_json::to_vec(body).map_err(RequestError::Body)?;
    Ok(Bytes::from(bytes))
}

/// Parse a JSON response body, or an error body on a non-success status.
pub fn parse_json<T, E>(
    resp: Response,
    parse_err: impl FnOnce(&Bytes) -> Option<E>,
) -> Result<T, ClientError<E>>
where
    T: DeserializeOwned,
{
    let status = resp.status();
    let body = resp.into_body();
    if status.is_success() {
        serde_json::from_slice(&body).map_err(ClientError::Deserialize)
    } else {
        Err(error_response(status, body, parse_err))
    }
}

/// Discard the response body, or parse an error body on a non-success status.
pub fn parse_empty<E>(
    resp: Response,
    parse_err: impl FnOnce(&Bytes) -> Option<E>,
) -> Result<(), ClientError<E>> {
    let status = resp.status();
    if status.is_success() {
        Ok(())
    } else {
        Err(error_response(status, resp.into_body(), parse_err))
    }
}

/// Return the raw response, or parse an error body on a non-success status.
pub fn parse_raw<E>(
    resp: Response,
    parse_err: impl FnOnce(&Bytes) -> Option<E>,
) -> Result<Response, ClientError<E>> {
    if resp.status().is_success() {
        Ok(resp)
    } else {
        let status = resp.status();
        Err(error_response(status, resp.into_body(), parse_err))
    }
}

fn error_response<E>(
    status: http::StatusCode,
    body: Bytes,
    parse_err: impl FnOnce(&Bytes) -> Option<E>,
) -> ClientError<E> {
    match parse_err(&body) {
        Some(error) => ClientError::Api(error),
        None => ClientError::Unexpected { status, body },
    }
}

#[cfg(feature = "client-reqwest")]
mod reqwest_impl {
    use bytes::Bytes;

    use crate::Response;

    pub type BlockingTransport = reqwest::blocking::Client;
    pub type AsyncTransport = reqwest::Client;
    pub type TransportError = reqwest::Error;

    fn build_response(status: http::StatusCode, headers: http::HeaderMap, body: Bytes) -> Response {
        let mut resp = http::Response::new(body);
        *resp.status_mut() = status;
        *resp.headers_mut() = headers;
        resp
    }

    #[derive(Debug, Clone)]
    pub struct AsyncClient {
        base_url: String,
        http: reqwest::Client,
    }

    impl AsyncClient {
        pub fn new(base_url: impl Into<String>) -> Self {
            Self::with_transport(base_url, reqwest::Client::new())
        }

        pub fn with_transport(base_url: impl Into<String>, http: AsyncTransport) -> Self {
            Self {
                base_url: base_url.into().trim_end_matches('/').to_owned(),
                http,
            }
        }

        #[doc(hidden)]
        pub async fn run(
            &self,
            method: http::Method,
            path: &str,
            body: Option<Bytes>,
        ) -> Result<Response, TransportError> {
            let mut req = self
                .http
                .request(method, format!("{}{path}", self.base_url));
            if let Some(body) = body {
                req = req
                    .header(http::header::CONTENT_TYPE, "application/json")
                    .body(body);
            }

            let resp = req.send().await?;
            let status = resp.status();
            let headers = resp.headers().clone();
            let body = resp.bytes().await?;
            Ok(build_response(status, headers, body))
        }
    }

    /// A blocking client backed by `reqwest`.
    #[derive(Debug, Clone)]
    pub struct BlockingClient {
        base_url: String,
        http: reqwest::blocking::Client,
    }

    impl BlockingClient {
        /// Create a client for the given base URL.
        pub fn new(base_url: impl Into<String>) -> Self {
            Self::with_transport(base_url, reqwest::blocking::Client::new())
        }

        /// Create a client from an existing [`reqwest::blocking::Client`].
        pub fn with_transport(base_url: impl Into<String>, http: BlockingTransport) -> Self {
            Self {
                base_url: base_url.into().trim_end_matches('/').to_owned(),
                http,
            }
        }

        #[doc(hidden)]
        pub fn run(
            &self,
            method: http::Method,
            path: &str,
            body: Option<Bytes>,
        ) -> Result<Response, TransportError> {
            let mut req = self
                .http
                .request(method, format!("{}{path}", self.base_url));
            if let Some(body) = body {
                req = req
                    .header(http::header::CONTENT_TYPE, "application/json")
                    .body(body);
            }

            let resp = req.send()?;
            let status = resp.status();
            let headers = resp.headers().clone();
            let body = resp.bytes()?;
            Ok(build_response(status, headers, body))
        }
    }
}

#[cfg(all(feature = "client-ureq", not(feature = "client-reqwest")))]
mod ureq_impl {
    use bytes::Bytes;

    use crate::Response;

    pub type BlockingTransport = ureq::Agent;
    pub type TransportError = ureq::Error;

    #[derive(Debug, Clone)]
    pub struct BlockingClient {
        base_url: String,
        agent: ureq::Agent,
    }

    impl BlockingClient {
        pub fn new(base_url: impl Into<String>) -> Self {
            // ureq doesn't make the body available for 4xx and 5xx unless we
            // set this flag.
            let agent = ureq::Agent::config_builder()
                .http_status_as_error(false)
                .build()
                .into();
            Self::with_transport(base_url, agent)
        }

        pub fn with_transport(base_url: impl Into<String>, agent: BlockingTransport) -> Self {
            Self {
                base_url: base_url.into().trim_end_matches('/').to_owned(),
                agent,
            }
        }

        #[doc(hidden)]
        pub fn run(
            &self,
            method: http::Method,
            path: &str,
            body: Option<Bytes>,
        ) -> Result<Response, TransportError> {
            let mut req = http::Request::builder()
                .method(method)
                .uri(format!("{}{path}", self.base_url));
            if body.is_some() {
                req = req.header(http::header::CONTENT_TYPE, "application/json");
            }

            let body = body.map(|b| b.to_vec()).unwrap_or_default();
            let req = req.body(body)?;

            let resp = self.agent.run(req)?;

            let (parts, mut body) = resp.into_parts();
            let body = body.read_to_vec()?;
            Ok(http::Response::from_parts(parts, Bytes::from(body)))
        }
    }
}
