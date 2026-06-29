use gropius::{EmptyResponse, Request, Response};
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Serialize, JsonSchema)]
struct ApiError {
    message: String,
}

impl gropius::ApiError for ApiError {
    fn status_code(&self) -> http::StatusCode {
        http::StatusCode::INTERNAL_SERVER_ERROR
    }
}

#[gropius::api]
trait ImageApi {
    #[endpoint(GET, "/image", content_type = "image/png")]
    async fn get_image(&self) -> Result<Response, ApiError>;

    #[endpoint(POST, "/image")]
    async fn upload_image(&self, request: Request) -> Result<EmptyResponse, ApiError>;
}

#[derive(Clone)]
struct Server;

impl ImageApi for Server {
    async fn get_image(&self) -> Result<Response, ApiError> {
        Ok(http::Response::builder()
            .status(200)
            .header("content-type", "image/png")
            .body(bytes::Bytes::from_static(b"\x89PNG fake image"))
            .unwrap())
    }

    async fn upload_image(&self, _request: Request) -> Result<EmptyResponse, ApiError> {
        Ok(EmptyResponse)
    }
}

#[tokio::test]
async fn api_impl() -> anyhow::Result<()> {
    let srv = Server;
    let router = gropius::Router::new(srv.endpoints())?;

    {
        let req = http::Request::get("/image").body(bytes::Bytes::new())?;
        let resp = router.dispatch(req).await;
        assert_eq!(resp.status(), 200);
        assert_eq!(resp.headers().get("content-type").unwrap(), "image/png");
        assert_eq!(resp.body().as_ref(), b"\x89PNG fake image");
    }

    {
        let req = http::Request::post("/image").body(bytes::Bytes::from_static(b"\x89PNG"))?;
        let resp = router.dispatch(req).await;
        assert_eq!(resp.status(), 200);
        assert!(resp.body().is_empty());
    }

    Ok(())
}

#[test]
fn openapi_spec() -> anyhow::Result<()> {
    let spec = gropius::Specification::new("ImageApi", "0.1.0")
        .with_endpoints(IMAGE_API_SPEC)
        .generate()?;

    insta::assert_yaml_snapshot!(spec);

    let json = serde_json::to_string_pretty(&spec)?;
    let parsed: oas3::Spec = oas3::from_json(&json)?;
    assert_eq!(spec, parsed);

    Ok(())
}
