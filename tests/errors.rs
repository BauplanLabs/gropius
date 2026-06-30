use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct ErrorBody {
    error: String,
    msg: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "error", content = "msg")]
enum ChairError {
    #[serde(rename = "WRONG_YEAR")]
    WrongYear(String),
    #[serde(rename = "CHAIR_NOT_FOUND")]
    NotFound(String),
}

impl gropius::ApiError for ChairError {
    fn status_code(&self) -> http::StatusCode {
        match self {
            ChairError::WrongYear(_) => http::StatusCode::BAD_REQUEST,
            ChairError::NotFound(_) => http::StatusCode::NOT_FOUND,
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize, JsonSchema)]
struct ChairResponse {
    model: String,
    year: u32,
}

#[derive(Deserialize, JsonSchema)]
struct CreateChair {
    model: String,
    year: u32,
}

#[derive(JsonSchema)]
struct Cursed;

impl Serialize for Cursed {
    fn serialize<S: serde::Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("cursed chair"))
    }
}

#[gropius::api]
trait ChairApi {
    #[endpoint(GET, "/v1/chairs/{year}/{id}")]
    async fn get_chair(
        &self,
        path: gropius::Path<(u32, String)>,
    ) -> Result<ChairResponse, ChairError>;

    #[endpoint(POST, "/v1/chairs")]
    async fn create_chair(
        &self,
        body: gropius::Body<CreateChair>,
    ) -> Result<ChairResponse, ChairError>;

    #[endpoint(GET, "/v1/cursed")]
    async fn cursed(&self) -> Result<Cursed, ChairError>;
}

#[derive(Clone)]
struct Server;

impl ChairApi for Server {
    async fn cursed(&self) -> Result<Cursed, ChairError> {
        Ok(Cursed)
    }

    async fn create_chair(
        &self,
        body: gropius::Body<CreateChair>,
    ) -> Result<ChairResponse, ChairError> {
        Ok(ChairResponse {
            model: body.model.clone(),
            year: body.year,
        })
    }

    async fn get_chair(
        &self,
        path: gropius::Path<(u32, String)>,
    ) -> Result<ChairResponse, ChairError> {
        let designed = match path.1.as_str() {
            "F51" => 1920,
            "D51" => 1922,
            "W199" => 1951,
            other => return Err(ChairError::NotFound(format!("no chair model {other}"))),
        };

        if path.0 != designed {
            return Err(ChairError::WrongYear(format!(
                "{} was designed in {designed}, not {}",
                path.1, path.0
            )));
        }

        Ok(ChairResponse {
            model: path.1.clone(),
            year: path.0,
        })
    }
}

fn error_handler(err: gropius::RouterError) -> http::Response<bytes::Bytes> {
    let status = err.status_code();
    let error_code = match &err {
        gropius::RouterError::NotFound => "NOT_FOUND",
        gropius::RouterError::MethodNotAllowed { .. } => "METHOD_NOT_ALLOWED",
        gropius::RouterError::InvalidPath { .. } => "BAD_PATH",
        gropius::RouterError::InvalidQueryString { .. } => "BAD_QUERY",
        gropius::RouterError::InvalidBody { .. } => "BAD_BODY",
        gropius::RouterError::ResponseSerialization(_) => "INTERNAL_ERROR",
    };

    let body = ErrorBody {
        error: error_code.to_string(),
        msg: err.to_string(),
    };

    let resp = http::Response::builder()
        .status(status)
        .header("content-type", "application/json");

    resp.body(bytes::Bytes::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

#[tokio::test]
async fn api_impl() -> anyhow::Result<()> {
    let srv = Server;
    let router = gropius::Router::builder()
        .with_endpoints(srv.endpoints())
        .with_error_handler(error_handler)
        .build()?;

    // Happy path: the catalogue knows F51 was designed in 1920.
    {
        let req = http::Request::builder()
            .method(http::Method::GET)
            .uri("/v1/chairs/1920/F51")
            .body(bytes::Bytes::new())?;

        let resp = router.dispatch(req).await;
        assert_eq!(resp.status(), 200);

        let body: ChairResponse = serde_json::from_slice(resp.body())?;
        assert_eq!(body.model, "F51");
        assert_eq!(body.year, 1920);
    }

    // Application error: right model, wrong year.
    {
        let req = http::Request::builder()
            .method(http::Method::GET)
            .uri("/v1/chairs/1999/F51")
            .body(bytes::Bytes::new())?;

        let resp = router.dispatch(req).await;
        assert_eq!(resp.status(), 400);

        let body: ErrorBody = serde_json::from_slice(resp.body())?;
        assert_eq!(
            body,
            ErrorBody {
                error: "WRONG_YEAR".into(),
                msg: "F51 was designed in 1920, not 1999".into(),
            }
        );
    }

    // Application error: unknown model.
    {
        let req = http::Request::builder()
            .method(http::Method::GET)
            .uri("/v1/chairs/1920/X99")
            .body(bytes::Bytes::new())?;

        let resp = router.dispatch(req).await;
        assert_eq!(resp.status(), 404);

        let body: ErrorBody = serde_json::from_slice(resp.body())?;
        assert_eq!(
            body,
            ErrorBody {
                error: "CHAIR_NOT_FOUND".into(),
                msg: "no chair model X99".into(),
            }
        );
    }

    // Generic 404: no route matches.
    {
        let req = http::Request::builder()
            .method(http::Method::GET)
            .uri("/v1/nonexistent")
            .body(bytes::Bytes::new())?;

        let resp = router.dispatch(req).await;
        assert_eq!(resp.status(), 404);

        let body: ErrorBody = serde_json::from_slice(resp.body())?;
        assert_eq!(
            body,
            ErrorBody {
                error: "NOT_FOUND".into(),
                msg: "not found".into(),
            }
        );
    }

    // Non-integer year fails path deserialization.
    {
        let req = http::Request::builder()
            .method(http::Method::GET)
            .uri("/v1/chairs/nineteen-twenty/F51")
            .body(bytes::Bytes::new())?;

        let resp = router.dispatch(req).await;
        assert_eq!(resp.status(), 404);

        let body: ErrorBody = serde_json::from_slice(resp.body())?;
        assert_eq!(body.error, "BAD_PATH");
    }

    // POST to a GET-only route.
    {
        let req = http::Request::builder()
            .method(http::Method::POST)
            .uri("/v1/chairs/1920/F51")
            .body(bytes::Bytes::new())?;

        let resp = router.dispatch(req).await;
        assert_eq!(resp.status(), 405);

        let body: ErrorBody = serde_json::from_slice(resp.body())?;
        assert_eq!(
            body,
            ErrorBody {
                error: "METHOD_NOT_ALLOWED".into(),
                msg: "method not allowed".into(),
            }
        );
    }

    // A response that fails to serialize is reported as an internal error.
    {
        let req = http::Request::builder()
            .method(http::Method::GET)
            .uri("/v1/cursed")
            .body(bytes::Bytes::new())?;

        let resp = router.dispatch(req).await;
        assert_eq!(resp.status(), 500);

        let body: ErrorBody = serde_json::from_slice(resp.body())?;
        assert_eq!(body.error, "INTERNAL_ERROR");
    }

    // Invalid JSON body.
    {
        let req = http::Request::builder()
            .method(http::Method::POST)
            .uri("/v1/chairs")
            .body(bytes::Bytes::from_static(b"not json"))?;

        let resp = router.dispatch(req).await;
        assert_eq!(resp.status(), 400);

        let body: ErrorBody = serde_json::from_slice(resp.body())?;
        assert_eq!(body.error, "BAD_BODY");
    }

    // JSON body with missing field.
    {
        let req = http::Request::builder()
            .method(http::Method::POST)
            .uri("/v1/chairs")
            .body(bytes::Bytes::from_static(b"{\"model\": \"B32\"}"))?;

        let resp = router.dispatch(req).await;
        assert_eq!(resp.status(), 400);

        let body: ErrorBody = serde_json::from_slice(resp.body())?;
        assert_eq!(body.error, "BAD_BODY");
    }

    // Valid JSON body.
    {
        let req = http::Request::builder()
            .method(http::Method::POST)
            .uri("/v1/chairs")
            .body(bytes::Bytes::from_static(
                b"{\"model\": \"B32\", \"year\": 1928}",
            ))?;

        let resp = router.dispatch(req).await;
        assert_eq!(resp.status(), 200);

        let body: ChairResponse = serde_json::from_slice(resp.body())?;
        assert_eq!(body.model, "B32");
        assert_eq!(body.year, 1928);
    }

    Ok(())
}

#[test]
fn openapi_spec() -> anyhow::Result<()> {
    let spec = gropius::Specification::new("ChairApi", "0.1.0")
        .with_endpoints(CHAIR_API_SPEC)
        .generate()?;

    insta::assert_yaml_snapshot!(spec);

    let json = serde_json::to_string_pretty(&spec)?;
    let parsed: oas3::Spec = oas3::from_json(&json)?;
    assert_eq!(spec, parsed);

    Ok(())
}
