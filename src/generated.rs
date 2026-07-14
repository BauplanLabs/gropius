//! Internal helpers used by generated code.

use std::{pin::Pin, sync::Arc};

use bytes::{BufMut, Bytes, BytesMut};
use schemars::{Schema, SchemaGenerator};
use serde::Serialize;

use crate::RouterError;

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// A type-erased handler closure. Takes the request and matched path
/// parameters by reference, extracts what it needs synchronously, and
/// returns an owned future.
pub type Handler = Arc<
    dyn Fn(
            &http::Request<Bytes>,
            &matchit::Params<'_, '_>,
        ) -> BoxFuture<Result<http::Response<Bytes>, RouterError>>
        + Send
        + Sync,
>;

/// A function pointer to `<T as JsonSchema>::json_schema`) for a type.
pub type SchemaFn = fn(&mut SchemaGenerator) -> Schema;

/// Describes how a success response is produced.
#[derive(Debug, Copy, Clone)]
pub enum ResponseType {
    /// JSON-serialized body with a schema.
    Json(SchemaFn),
    /// No body.
    Empty,
    /// Raw `http::Response<Bytes>` with an optional content type for the spec.
    Raw(Option<&'static str>),
}

/// A generated API description with no handlers. Used to generate the
/// [`Specification`](crate::Specification).
#[derive(Debug, Copy, Clone)]
pub struct Api {
    pub attr: ApiAttributes,
    pub endpoints: &'static [Endpoint],
}

/// A generated API description, with handlers attached. Used in a [`Router`](crate::Router).
pub struct ApiImpl {
    pub attributes: ApiAttributes,
    pub handlers: Vec<(&'static Endpoint, Handler)>,
}

impl std::fmt::Debug for ApiImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiImpl")
            .field("attributes", &self.attributes)
            .field(
                "endpoints",
                &self
                    .handlers
                    .iter()
                    .map(|(ep, _)| ep.name)
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ApiAttributes {
    /// Tags that apply to all the endpoints.
    pub tags: &'static [&'static str],
    /// The trait's doc comment.
    pub doc: Option<&'static str>,
}

/// Represents a single HTTP endpoint.
#[derive(Debug, Copy, Clone)]
pub struct Endpoint {
    /// HTTP method, e.g. `GET`.
    pub method: &'static http::Method,
    /// Path template, e.g. `/v1/shapes/{id}`.
    pub path: &'static str,
    /// The path template's parameter names, in order (e.g. `["id"]`).
    pub path_params: &'static [&'static str],
    /// The method's name.
    pub name: &'static str,
    /// The doc comment on the method.
    pub doc: Option<&'static str>,
    /// Whether the handler expects the raw request.
    pub raw_request: bool,
    /// Schema of the `Query<T>` extractor's inner type.
    pub query_type: Option<SchemaFn>,
    /// Schema of the `Path<T>` extractor's inner type.
    pub path_type: Option<SchemaFn>,
    /// Schema of the request body.
    pub request_type: Option<SchemaFn>,
    /// The kind of success response.
    pub response_type: ResponseType,
    /// Schema of the error response body. `None` for infallible endpoints.
    pub error_type: Option<SchemaFn>,
}

/// Construct a response for an http handler.
pub fn make_json_response<Body: Serialize>(
    body: &Body,
    status: impl Into<http::StatusCode>,
) -> Result<http::Response<Bytes>, RouterError> {
    let mut buf = BytesMut::new().writer();

    let res = if cfg!(debug_assertions) {
        serde_json::to_writer_pretty(&mut buf, &body)
    } else {
        serde_json::to_writer(&mut buf, &body)
    };

    let bytes = match res {
        Ok(_) => buf.into_inner().freeze(),
        Err(e) => return Err(RouterError::ResponseSerialization(e)),
    };

    let resp = http::Response::builder()
        .status(status.into())
        .header("content-type", "application/json")
        .body(bytes)
        .unwrap();

    Ok(resp)
}
