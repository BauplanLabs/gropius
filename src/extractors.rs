use std::{fmt, ops::Deref};

use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::{Request, RouterError};

mod path;

/// Extracts typed path parameters from the request URL.
///
/// # Examples
///
/// A struct with named fields maps each field to a path segment:
///
/// ```
/// # use schemars::JsonSchema;
/// # use serde::{Deserialize, Serialize};
/// # #[derive(Serialize, JsonSchema)] struct MyError;
/// # impl gropius::ApiError for MyError {
/// #     fn status_code(&self) -> http::StatusCode { http::StatusCode::INTERNAL_SERVER_ERROR }
/// # }
/// #
/// #[derive(Deserialize, JsonSchema)]
/// struct WidgetPath {
///     #[serde(rename = "type")]
///     widget_type: String,
///     id: u64,
/// }
///
/// #[gropius::api]
/// trait WidgetApi {
///     #[endpoint(GET, "/widgets/{type}/by-id/{id}")]
///     async fn get_widget(
///         &self,
///         path: gropius::Path<WidgetPath>,
///     ) -> Result<(), MyError>;
/// }
/// ```
///
/// For a single path parameter, you can use a single primitive type, or one
/// wrapped in a newtype:
///
/// ```
/// # use schemars::JsonSchema;
/// # use serde::{Deserialize, Serialize};
/// # #[derive(Serialize, JsonSchema)] struct MyError;
/// # impl gropius::ApiError for MyError {
/// #     fn status_code(&self) -> http::StatusCode { http::StatusCode::INTERNAL_SERVER_ERROR }
/// # }
///
/// #[gropius::api]
/// trait WidgetApi {
///     #[endpoint(GET, "/widgets/{id}")]
///     async fn get_widget(
///         &self,
///         path: gropius::Path<u64>,
///     ) -> Result<(), MyError>;
/// }
/// ```
///
/// You can also use tuples of primitive types and newtypes:
///
/// ```
/// # use schemars::JsonSchema;
/// # use serde::{Deserialize, Serialize};
/// # #[derive(Serialize, JsonSchema)] struct MyError;
/// # impl gropius::ApiError for MyError {
/// #     fn status_code(&self) -> http::StatusCode { http::StatusCode::INTERNAL_SERVER_ERROR }
/// # }
/// #[derive(Deserialize, JsonSchema)]
/// struct Id(u32);
///
/// #[gropius::api]
/// trait ChairApi {
///     #[endpoint(GET, "/chairs/{year}/{id}")]
///     async fn get_chair(
///         &self,
///         path: gropius::Path<(Id, String)>,
///     ) -> Result<(), MyError>;
/// }
/// ```
pub struct Path<T: DeserializeOwned + JsonSchema> {
    inner: T,
}

impl<T: DeserializeOwned + JsonSchema> Path<T> {
    #[doc(hidden)]
    pub fn extract(params: &matchit::Params<'_, '_>) -> Result<Self, RouterError> {
        let de = path::PathDeserializer::new(params);
        match T::deserialize(de) {
            Ok(inner) => Ok(Self { inner }),
            Err(err) => Err(RouterError::InvalidPath {
                field: None,
                source: err,
            }),
        }
    }
}

impl<T: DeserializeOwned + JsonSchema> Deref for Path<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: DeserializeOwned + JsonSchema + fmt::Debug> fmt::Debug for Path<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T: DeserializeOwned + JsonSchema + fmt::Display> fmt::Display for Path<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

/// Extracts typed query string parameters from the request URL.
///
/// ```
/// # use schemars::JsonSchema;
/// # use serde::{Deserialize, Serialize};
/// # #[derive(Serialize, JsonSchema)] struct MyError;
/// # impl gropius::ApiError for MyError {
/// #     fn status_code(&self) -> http::StatusCode { http::StatusCode::INTERNAL_SERVER_ERROR }
/// # }
/// #[derive(Deserialize, JsonSchema)]
/// struct ListQuery {
///     page: Option<u32>,
///     per_page: Option<u32>,
/// }
///
/// #[gropius::api]
/// trait WidgetApi {
///     #[endpoint(GET, "/widgets")]
///     async fn list_widgets(
///         &self,
///         query: gropius::Query<ListQuery>,
///     ) -> Result<(), MyError>;
/// }
/// ```
pub struct Query<T: DeserializeOwned + JsonSchema> {
    inner: T,
}

impl<T: DeserializeOwned + JsonSchema> Query<T> {
    #[doc(hidden)]
    pub fn extract(req: &Request) -> Result<Self, RouterError> {
        let qs = req.uri().query().unwrap_or("");
        let parser = form_urlencoded::parse(qs.as_bytes());
        let de = serde_urlencoded::Deserializer::new(parser);
        let mut track = serde_path_to_error::Track::new();
        let jd = serde_path_to_error::Deserializer::new(de, &mut track);
        match T::deserialize(jd) {
            Ok(inner) => Ok(Self { inner }),
            Err(err) => {
                let field = {
                    let path = track.path().to_string();
                    if path.is_empty() { None } else { Some(path) }
                };
                Err(RouterError::InvalidQueryString { field, source: err })
            }
        }
    }
}

impl<T: DeserializeOwned + JsonSchema> Deref for Query<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: DeserializeOwned + JsonSchema + fmt::Debug> fmt::Debug for Query<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T: DeserializeOwned + JsonSchema + fmt::Display> fmt::Display for Query<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

/// Extracts a typed JSON body from the request.
///
/// ```
/// # use schemars::JsonSchema;
/// # use serde::{Deserialize, Serialize};
/// # #[derive(Serialize, JsonSchema)] struct MyError;
/// # impl gropius::ApiError for MyError {
/// #     fn status_code(&self) -> http::StatusCode { http::StatusCode::INTERNAL_SERVER_ERROR }
/// # }
/// #[derive(Deserialize, JsonSchema)]
/// struct CreateWidget {
///     name: String,
/// }
///
/// #[gropius::api]
/// trait WidgetApi {
///     #[endpoint(POST, "/widgets")]
///     async fn create_widget(
///         &self,
///         body: gropius::Body<CreateWidget>,
///     ) -> Result<(), MyError>;
/// }
/// ```
pub struct Body<T: DeserializeOwned + JsonSchema> {
    inner: T,
}

impl<T: DeserializeOwned + JsonSchema> Body<T> {
    #[doc(hidden)]
    pub fn extract(req: &Request) -> Result<Self, RouterError> {
        let jd = &mut serde_json::Deserializer::from_slice(req.body());
        match serde_path_to_error::deserialize(jd) {
            Ok(inner) => Ok(Self { inner }),
            Err(err) => {
                let field = Some(err.path().to_string()).filter(|s| !s.is_empty());
                Err(RouterError::InvalidBody {
                    field,
                    source: err.into_inner(),
                })
            }
        }
    }
}

impl<T: DeserializeOwned + JsonSchema> Deref for Body<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: DeserializeOwned + JsonSchema + fmt::Debug> fmt::Debug for Body<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T: DeserializeOwned + JsonSchema + fmt::Display> fmt::Display for Body<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}
