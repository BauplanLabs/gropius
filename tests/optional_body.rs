use bytes::Bytes;
use gropius::{Body, Path};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct DeleteWidget {
    reason: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct Deleted {
    reason: Option<String>,
}

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
trait WidgetApi {
    /// Delete a widget, optionally saying why.
    #[endpoint(DELETE, "/widgets/{id}")]
    async fn delete_widget(
        &self,
        path: Path<u64>,
        body: Option<Body<DeleteWidget>>,
    ) -> Result<Deleted, ApiError>;
}

#[derive(Clone)]
struct Server;

impl WidgetApi for Server {
    async fn delete_widget(
        &self,
        _path: Path<u64>,
        body: Option<Body<DeleteWidget>>,
    ) -> Result<Deleted, ApiError> {
        Ok(Deleted {
            reason: body.map(|body| body.inner.reason),
        })
    }
}

fn delete_request(body: &'static [u8]) -> http::Request<Bytes> {
    http::Request::delete("/widgets/1")
        .body(Bytes::from_static(body))
        .unwrap()
}

#[tokio::test]
async fn empty() -> anyhow::Result<()> {
    let router = gropius::Router::new(Server.endpoints())?;

    let resp = router.dispatch(delete_request(b"")).await;
    assert_eq!(resp.status(), 200);

    let deleted: Deleted = serde_json::from_slice(resp.body())?;
    assert_eq!(deleted.reason, None);

    Ok(())
}

#[tokio::test]
async fn invalid() -> anyhow::Result<()> {
    let router = gropius::Router::new(Server.endpoints())?;

    let resp = router.dispatch(delete_request(br#"{"reason": 1}"#)).await;
    assert_eq!(resp.status(), 400);

    Ok(())
}

#[test]
fn spec() -> anyhow::Result<()> {
    let spec = gropius::Specification::new("WidgetApi", "0.1.0")
        .with_endpoints(WIDGET_API_SPEC)
        .generate()?;

    insta::assert_yaml_snapshot!(spec);

    Ok(())
}
