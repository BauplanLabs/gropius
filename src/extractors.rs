use std::{fmt, ops::Deref};

use bytes::Bytes;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::{Request, RouterError, path};

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
        let de = path::de::PathDeserializer::new(params);
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

/// Extracts a `multipart/form-data` body as a sequence of fields.
///
/// The optional type parameter documents the body in the OpenAPI
/// specification, using a type deriving [`JsonSchema`]; [`Binary`] marks the
/// file parts:
///
/// ```
/// # use schemars::JsonSchema;
/// # use serde::Serialize;
/// # #[derive(Serialize, JsonSchema)] struct MyError;
/// # impl gropius::ApiError for MyError {
/// #     fn status_code(&self) -> http::StatusCode { http::StatusCode::INTERNAL_SERVER_ERROR }
/// # }
/// #[derive(JsonSchema)]
/// struct CreateWidgetBody {
///     name: String,
///     file: gropius::Binary,
/// }
///
/// #[gropius::api]
/// trait WidgetApi {
///     #[endpoint(POST, "/widgets")]
///     async fn create_widget(
///         &self,
///         body: gropius::MultipartBody<CreateWidgetBody>,
///     ) -> Result<(), MyError>;
/// }
///
/// # #[derive(Clone)] struct Server;
/// impl WidgetApi for Server {
///     async fn create_widget(
///         &self,
///         mut body: gropius::MultipartBody<CreateWidgetBody>,
///     ) -> Result<(), MyError> {
///         while let Some(field) = body.next_field().await.unwrap() {
///             let name = field.name().map(str::to_owned);
///             let contents = field.bytes().await.unwrap();
///             // ...
///         }
///         Ok(())
///     }
/// }
/// ```
pub struct MultipartBody<S = ()> {
    pub(crate) inner: multer::Multipart<'static>,
    pub(crate) _schema: std::marker::PhantomData<S>,
}

impl<S> MultipartBody<S> {
    /// Return the next field in the body, or `None` when there are no more.
    pub async fn next_field(&mut self) -> Result<Option<Field>, RouterError> {
        let field = self
            .inner
            .next_field()
            .await
            .map_err(|source| RouterError::InvalidMultipart { source })?;

        Ok(field.map(Field))
    }
}

impl<S> fmt::Debug for MultipartBody<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MultipartBody").finish()
    }
}

/// A schema-only marker for the binary file parts of a [`MultipartBody`].
/// Documented as a binary string in the OpenAPI specification.
#[derive(Debug, Clone, Copy)]
pub struct Binary;

impl JsonSchema for Binary {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Binary".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "format": "binary",
        })
    }
}

/// A single field of a [`MultipartBody`].
pub struct Field(multer::Field<'static>);

impl Field {
    /// The name of the field, from the `Content-Disposition` header.
    pub fn name(&self) -> Option<&str> {
        self.0.name()
    }

    /// The content-type of the field, if one was set.
    pub fn content_type(&self) -> Option<&str> {
        self.0.content_type().map(|m| m.as_ref())
    }

    /// Read the full contents of the field.
    pub async fn bytes(self) -> Result<Bytes, RouterError> {
        self.0
            .bytes()
            .await
            .map_err(|source| RouterError::InvalidMultipart { source })
    }
}

impl fmt::Debug for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Field").finish()
    }
}
