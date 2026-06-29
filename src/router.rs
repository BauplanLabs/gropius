use std::{collections::BTreeMap, pin::Pin, sync::Arc, task::Poll};

use http_body::Body;
use http_body_util::{BodyExt, Full};
use smallvec::SmallVec;

use crate::{
    error::{ErrorHandler, RouterError, default_error_handler},
    generated::{self, Handler},
};

type MethodHandlers = SmallVec<[(&'static http::Method, Handler); 16]>;

/// An error returned when building a [`Router`].
#[derive(Debug, thiserror::Error)]
pub enum BuildRouterError {
    /// Two endpoints registered the same method and path.
    #[error("conflicting route: {0}")]
    Conflict(#[from] matchit::InsertError),
}

/// A wrapper for one or more API implementations that can serve actual
/// requests.
///
/// Implements [`tower::Service`], so you can use it with HTTP server
/// implementations such as `hyper`.
///
/// # Examples
///
/// Wrap a single API:
///
/// ```
/// # use gropius::Router;
/// # use schemars::JsonSchema;
/// # use serde::{Deserialize, Serialize};
/// #
/// # #[derive(Serialize, JsonSchema)]
/// # struct MyError;
/// # impl gropius::ApiError for MyError {
/// #     fn status_code(&self) -> http::StatusCode {
/// #         http::StatusCode::INTERNAL_SERVER_ERROR
/// #     }
/// # }
/// #
/// # #[derive(Serialize, JsonSchema)]
/// # struct Pong;
/// #
/// # #[gropius::api]
/// # trait HealthApi {
/// #     #[endpoint(GET, "/healthz")]
/// #     async fn healthz(&self) -> Result<Pong, MyError>;
/// # }
/// #
/// # #[derive(Clone)]
/// # struct Server;
/// # impl HealthApi for Server {
/// #     async fn healthz(&self) -> Result<Pong, MyError> {
/// #         Ok(Pong)
/// #     }
/// # }
/// #
/// let server = Server;
/// let router = Router::new(server.endpoints());
/// ```
///
/// Combine multiple APIs:
///
/// ```
/// # use gropius::Router;
/// # use schemars::JsonSchema;
/// # use serde::{Deserialize, Serialize};
/// #
/// # #[derive(Serialize, JsonSchema)]
/// # struct MyError;
/// # impl gropius::ApiError for MyError {
/// #     fn status_code(&self) -> http::StatusCode {
/// #         http::StatusCode::INTERNAL_SERVER_ERROR
/// #     }
/// # }
/// #
/// # #[derive(Serialize, JsonSchema)]
/// # struct Pong;
/// #
/// # #[gropius::api]
/// # trait HealthApi {
/// #     #[endpoint(GET, "/healthz")]
/// #     async fn healthz(&self) -> Result<Pong, MyError>;
/// # }
/// #
/// # #[gropius::api]
/// # trait WidgetApi {
/// #     #[endpoint(GET, "/widgets")]
/// #     async fn list_widgets(&self) -> Result<Pong, MyError>;
/// # }
/// #
/// # #[derive(Clone)]
/// # struct Server;
/// # impl HealthApi for Server {
/// #     async fn healthz(&self) -> Result<Pong, MyError> {
/// #         Ok(Pong)
/// #     }
/// # }
/// # impl WidgetApi for Server {
/// #     async fn list_widgets(&self) -> Result<Pong, MyError> {
/// #         Ok(Pong)
/// #     }
/// # }
/// #
/// let server = Server;
/// let router = Router::builder()
///     .with_endpoints(HealthApi::endpoints(&server))
///     .with_endpoints_at("/api/v1", WidgetApi::endpoints(&server))
///     .build();
/// ```
#[derive(Clone)]
pub struct Router {
    router: Arc<matchit::Router<MethodHandlers>>,
    error_handler: ErrorHandler,
}

impl std::fmt::Debug for Router {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Router").finish()
    }
}

/// A builder for creating a [`Router`] instance.
#[derive(Debug)]
pub struct RouterBuilder {
    groups: Vec<(&'static str, generated::ApiImpl)>,
    error_handler: ErrorHandler,
}

impl Default for RouterBuilder {
    fn default() -> Self {
        Self {
            groups: Vec::new(),
            error_handler: default_error_handler,
        }
    }
}

impl RouterBuilder {
    /// Add endpoints to the router.
    pub fn with_endpoints(self, api: generated::ApiImpl) -> Self {
        self.with_endpoints_at("", api)
    }

    /// Add endpoints to the router with the given prefix. For each endpoint,
    /// the prefix will be joined to the endpoint path.
    pub fn with_endpoints_at(mut self, path: &'static str, api: generated::ApiImpl) -> Self {
        self.groups.push((path, api));
        self
    }

    /// Set a generic error handler for the API. See the [crate-level
    /// documentation](crate#error-handling) for an example.
    ///
    /// The default returns an appropriate status code and the following JSON
    /// response:
    ///
    /// ```json
    /// { "error": "<error text>" }
    /// ```
    pub fn with_error_handler(mut self, handler: ErrorHandler) -> Self {
        self.error_handler = handler;
        self
    }

    /// Build the router.
    pub fn build(self) -> Result<Router, BuildRouterError> {
        let mut by_path: BTreeMap<String, MethodHandlers> = BTreeMap::new();

        for (prefix, api) in self.groups {
            let prefix = prefix.strip_suffix('/').unwrap_or(prefix);
            for (ep, handler) in api.handlers {
                let mut path = prefix.to_owned();
                path.push_str(ep.path);
                by_path.entry(path).or_default().push((ep.method, handler));
            }
        }

        let mut router = matchit::Router::new();
        for (path, methods) in by_path {
            router.insert(path, methods)?;
        }

        Ok(Router {
            router: Arc::new(router),
            error_handler: self.error_handler,
        })
    }
}

impl Router {
    /// Wrap a single set of endpoints. The router will use a default
    /// implementation for generic errors (for example, if the request doesn't
    /// match any paths).
    pub fn new(api: generated::ApiImpl) -> Result<Self, BuildRouterError> {
        RouterBuilder::default().with_endpoints(api).build()
    }

    /// Create a new [`RouterBuilder`].
    pub fn builder() -> RouterBuilder {
        RouterBuilder::default()
    }

    /// Runs one request through the router, returning a response.
    pub async fn dispatch(&self, req: crate::Request) -> crate::Response {
        let error_handler = self.error_handler;

        let err = match self.router.at(req.uri().path()) {
            Ok(matched) => {
                for (method, handler) in matched.value.iter() {
                    if **method == *req.method() {
                        let handler = handler.clone();
                        let fut = handler(&req, &matched.params);
                        return match fut.await {
                            Ok(resp) => resp,
                            Err(err) => error_handler(err),
                        };
                    }
                }

                let allowed = matched
                    .value
                    .iter()
                    .map(|(method, _)| (*method).clone())
                    .collect();
                RouterError::MethodNotAllowed { allowed }
            }
            Err(_) => RouterError::NotFound,
        };

        error_handler(err)
    }
}

impl<B> tower::Service<http::Request<B>> for Router
where
    B: Body + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Response = http::Response<Full<bytes::Bytes>>;
    type Error = B::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: http::Request<B>) -> Self::Future {
        let this = self.clone();

        Box::pin(async move {
            let (parts, body) = req.into_parts();
            let body = body.collect().await?.to_bytes();
            let req = http::Request::from_parts(parts, body);

            let resp = this.dispatch(req).await;
            Ok(resp.map(Full::new))
        })
    }
}
