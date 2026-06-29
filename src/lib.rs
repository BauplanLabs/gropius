//! # Gropius
//!
//! Gropius is a rust library for defining OpenAPI-compatible APIs in Rust
//! traits. While OpenAPI "best practices" recommend that you write your API
//! definitions as YAML and then generate server and client code, this is the
//! reverse: define a rust trait, and you can generate the specification from it.
//!
//! Defining your API as a trait also allows you to have multiple
//! implementations, for example a stub implementation in tests. At Bauplan, we
//! also open source our API trait definition, alongside the client and in a
//! completely separate crate from the implementation.
//!
//! ## Defining your API
//!
//! At a high level, you define your API in one or more traits. Each endpoint
//! defines its own types for the path and query parameters, the request and
//! response body, and the error type:
//!
//! ```rust
//! # use gropius::Path;
//! # use serde::{Deserialize, Serialize};
//! # use schemars::JsonSchema;
//! #
//! #[derive(Serialize, Deserialize, JsonSchema)]
//! struct Id(u64);
//!
//! #[derive(Serialize, Deserialize, JsonSchema)]
//! struct Widget {
//!     name: String
//! }
//!
//! #[derive(Serialize, Deserialize, JsonSchema)]
//! struct Error {
//!     code: String,
//!     msg: String
//! }
//!
//! impl gropius::ApiError for Error {
//!      fn status_code(&self) -> http::StatusCode {
//!          http::StatusCode::INTERNAL_SERVER_ERROR
//!      }
//! }
//!
//! #[gropius::api]
//! trait WidgetApi {
//!     #[endpoint(GET, "/v1/widgets/{id}")]
//!     async fn get_widget(&self, path: Path<Id>) -> Result<Widget, Error>;
//! }
//! ```
//!
//! ### Endpoint request types
//!
//! An endpoint can accept any combination of optional "extractors": [`Path`],
//! [`Query`], [`Body`], and [`Request`], in that order. The former three parse
//! the request, while the latter gives you the raw request from which you can
//! read headers, parse the body yourself, etc.
//!
//! The inner types of the extractors must implement
//! [`serde::de::DeserializeOwned`] and [`schemars::JsonSchema`]:
//!
//! ```rust
//! # use gropius::{Body, Path, Query, Request};
//! # use serde::{Deserialize, Serialize};
//! # use schemars::JsonSchema;
//! # #[derive(Serialize, JsonSchema)]
//! # struct Error;
//! # impl gropius::ApiError for Error {
//! #     fn status_code(&self) -> http::StatusCode { http::StatusCode::INTERNAL_SERVER_ERROR }
//! # }
//! #
//! #[derive(Deserialize, JsonSchema)]
//! struct WidgetQuery {
//!     #[serde(alias = "dryrun")]
//!     dry_run: bool,
//! }
//!
//! #[derive(Serialize, Deserialize, JsonSchema)]
//! struct WidgetUpdate {
//!     name: String,
//!     bucket: u32,
//! }
//!
//! #[gropius::api]
//! trait WidgetApi {
//!     #[endpoint(PUT, "/v1/widgets/{id}")]
//!     async fn update_widget(
//!         &self,
//!         path: Path<u64>,
//!         query: Query<WidgetQuery>,
//!         body: Body<WidgetUpdate>,
//!         req: Request,
//!     ) -> Result<WidgetUpdate, Error>;
//! }
//! ```
//!
//! ### Endpoint response types
//!
//! Endpoints must return `Result<T, E>`. The response type can be either
//!
//!  - A type implementing [`Serialize`](serde::Serialize) and
//!    [`JsonSchema`](schemars::JsonSchema)
//!  - [`EmptyResponse`], for a 200 response with no body
//!  - [`Response`], for anything custom
//!
//! The error type is for non-200 responses. In addition to `Serialize` and
//! `JsonSchema`, it must implement [`ApiError`], in order to provide a status
//! code.
//!
//! ### Custom content-type
//!
//! You can use the `content-type` argument to `#[endpoint(...)]` to customize
//! the content-type:
//!
//! ```rust
//! # use serde::Serialize;
//! # use schemars::JsonSchema;
//! # #[derive(Debug, Serialize, JsonSchema)]
//! # struct ApiError {
//! #    message: String,
//! # }
//! #
//! # impl gropius::ApiError for ApiError {
//! #    fn status_code(&self) -> http::StatusCode {
//! #        http::StatusCode::INTERNAL_SERVER_ERROR
//! #    }
//! # }
//! #
//! #[gropius::api]
//! trait ImageApi {
//!     #[endpoint(GET, "/image", content_type = "image/png")]
//!     async fn get_image(&self) -> Result<gropius::Response, ApiError>;
//! }
//! ```
//!
//! ### Tags
//!
//! Each `gropius::api` accepts any number of operation tags, which will be
//! applied to all endpoints in the trait:
//!
//! ```rust
//! #[gropius::api(tags = ["foo", "bar", "baz"])]
//! trait FooApi {
//!     // ...
//! }
//! ```
//!
//! To have different tags on different endpoints, use multiple traits (see
//! below).
//!
//! ## Implementing the trait
//!
//! Implement the API trait to build a [`tower::Service`]:
//!
//! ```rust
//! # use gropius::Path;
//! # use serde::{Deserialize, Serialize};
//! # use schemars::JsonSchema;
//! #
//! # #[derive(Serialize, Deserialize, JsonSchema)]
//! # struct Id(u64);
//! #
//! # #[derive(Serialize, Deserialize, JsonSchema)]
//! # struct Widget {
//! #     name: String
//! # }
//! #
//! # #[derive(Serialize, Deserialize, JsonSchema)]
//! # struct Error {
//! #     code: String,
//! #     msg: String
//! # }
//! #
//! # impl gropius::ApiError for Error {
//! #     fn status_code(&self) -> http::StatusCode {
//! #         http::StatusCode::INTERNAL_SERVER_ERROR
//! #     }
//! # }
//! #
//! # #[gropius::api]
//! # trait WidgetApi {
//! #     #[endpoint(GET, "/v1/widgets/{id}")]
//! #     async fn get_widget(&self, path: Path<Id>) -> Result<Widget, Error>;
//! # }
//! #
//! #[derive(Clone)]
//! struct Server;
//!
//! impl WidgetApi for Server {
//!     async fn get_widget(
//!         &self,
//!         path: Path<Id>,
//!     )-> Result<Widget, Error> {
//!         // endpoint logic goes here
//!         Ok(Widget {
//!             name: "my widget".to_string()
//!         })
//!     }
//! }
//! ```
//!
//! Using [`Router`], you can combine one or more API traits into one
//! `tower::Service`:
//!
//! ```rust
//! # use gropius::{Path, Router};
//! # use serde::{Deserialize, Serialize};
//! # use schemars::JsonSchema;
//! #
//! # #[derive(Serialize, Deserialize, JsonSchema)]
//! # struct Id(u64);
//! #
//! # #[derive(Serialize, Deserialize, JsonSchema)]
//! # struct Widget {
//! #     name: String
//! # }
//! #
//! # #[derive(Serialize, Deserialize, JsonSchema)]
//! # struct Error {
//! #     code: String,
//! #     msg: String
//! # }
//! #
//! # impl gropius::ApiError for Error {
//! #     fn status_code(&self) -> http::StatusCode {
//! #         http::StatusCode::INTERNAL_SERVER_ERROR
//! #     }
//! # }
//! #
//! # #[gropius::api]
//! # trait WidgetApi {
//! #     #[endpoint(GET, "/v1/widgets/{id}")]
//! #     async fn get_widget(&self, path: Path<Id>) -> Result<Widget, Error>;
//! # }
//! #
//! # #[derive(Clone)]
//! # struct Server;
//! #
//! # impl WidgetApi for Server {
//! #     async fn get_widget(
//! #         &self,
//! #         path: Path<Id>,
//! #     )-> Result<Widget, Error> {
//! #         todo!()
//! #     }
//! # }
//! #
//! let srv = Server;
//! let service = Router::builder()
//!     // .with_endpoints_at adds a prefix to all the endpoints
//!     .with_endpoints_at("/v1", srv.endpoints())
//!     .build()
//!     .unwrap();
//!
//! // service implements tower::Service and can be used with hyper, etc.
//! ```
//!
//! ## Generating the OpenAPI specification
//!
//! The `#[gropius::api]` macro creates a constant next to the trait definition
//! in `SHOUTY_CASE`. This can be used to create a [`Specification`] and
//! generate a YAML or JSON OpenAPI document:
//!
//! ```rust
//! # use gropius::{Path, Specification};
//! # use serde::{Deserialize, Serialize};
//! # use schemars::JsonSchema;
//! #
//! # #[derive(Serialize, Deserialize, JsonSchema)]
//! # struct Id(u64);
//! #
//! # #[derive(Serialize, Deserialize, JsonSchema)]
//! # struct Widget {
//! #     name: String
//! # }
//! #
//! # #[derive(Serialize, Deserialize, JsonSchema)]
//! # struct Error {
//! #     code: String,
//! #     msg: String
//! # }
//! #
//! # impl gropius::ApiError for Error {
//! #     fn status_code(&self) -> http::StatusCode {
//! #         http::StatusCode::INTERNAL_SERVER_ERROR
//! #     }
//! # }
//! #
//! #[gropius::api]
//! trait WidgetApi {
//!     #[endpoint(GET, "/v1/widgets/{id}")]
//!     async fn get_widget(&self, path: Path<Id>) -> Result<Widget, Error>;
//! }
//!
//! # fn main() -> anyhow::Result<()> {
//! let spec = Specification::new("Widget API", "v0.1.0")
//!   // The name is always shouty_case(trait_name) + '_SPEC'.
//!   .with_endpoints(WIDGET_API_SPEC)
//!   .generate_yaml()?;
//! # Ok(())
//! # }
//! ```

#![warn(
    anonymous_parameters,
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    nonstandard_style,
    rust_2018_idioms,
    single_use_lifetimes,
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    unused_extern_crates,
    unused_qualifications,
    variant_size_differences
)]

mod error;
mod extractors;
mod router;
mod spec;

#[doc(hidden)]
pub mod generated;

pub use error::{ApiError, ErrorHandler, RouterError, default_error_handler};
pub use extractors::*;
pub use gropius_macros::api;
pub use router::*;
pub use spec::{SpecError, Specification};

use bytes::Bytes;

/// An HTTP request.
///
/// # Examples
///
/// You can use [`Request`] as an argument in an API endpoint, and it will be
/// passed the raw request. This can be useful if you need to inspect the
/// headers, read the raw bytes of the request, or do something else custom.
///
/// ```
/// # use schemars::JsonSchema;
/// # use serde::Serialize;
/// # #[derive(Serialize, JsonSchema)] struct MyError;
/// # impl gropius::ApiError for MyError {
/// #     fn status_code(&self) -> http::StatusCode { http::StatusCode::INTERNAL_SERVER_ERROR }
/// # }
/// #
///
/// #[gropius::api]
/// trait MyApi {
///      #[endpoint(POST, "/foo")]
///      async fn index(&self, req: gropius::Request) -> Result<(), MyError>;
/// }
/// ```
pub type Request = http::Request<Bytes>;

/// An HTTP response.
pub type Response = http::Response<Bytes>;

/// An empty 200 response.
#[derive(Debug, Copy, Clone)]
pub struct EmptyResponse;
