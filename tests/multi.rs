use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, JsonSchema)]
struct ApiError {
    message: String,
}

impl gropius::ApiError for ApiError {
    fn status_code(&self) -> http::StatusCode {
        http::StatusCode::INTERNAL_SERVER_ERROR
    }
}

#[gropius::api(tags = ["health"])]
trait HealthApi {
    /// Check service health.
    #[endpoint(GET, "/healthz")]
    async fn healthz(&self) -> Result<gropius::EmptyResponse, ApiError>;
}

#[derive(Serialize, Deserialize, JsonSchema)]
struct Widget {
    id: u64,
    name: String,
}

#[derive(Deserialize, JsonSchema)]
struct WidgetPath {
    id: u64,
}

/// Widget management.
#[gropius::api(tags = ["widgets"])]
trait WidgetApi {
    /// List all widgets.
    #[endpoint(GET, "/widgets")]
    async fn list_widgets(&self) -> Result<Vec<Widget>, ApiError>;

    /// Get a widget by ID.
    #[endpoint(GET, "/widgets/{id}")]
    async fn get_widget(&self, path: gropius::Path<WidgetPath>) -> Result<Widget, ApiError>;
}

#[derive(Clone)]
struct Server;

impl HealthApi for Server {
    async fn healthz(&self) -> Result<gropius::EmptyResponse, ApiError> {
        Ok(gropius::EmptyResponse)
    }
}

impl WidgetApi for Server {
    async fn list_widgets(&self) -> Result<Vec<Widget>, ApiError> {
        Ok(vec![
            Widget {
                id: 1,
                name: "sprocket".into(),
            },
            Widget {
                id: 2,
                name: "gizmo".into(),
            },
        ])
    }

    async fn get_widget(&self, path: gropius::Path<WidgetPath>) -> Result<Widget, ApiError> {
        if path.id == 1 {
            Ok(Widget {
                id: 1,
                name: "sprocket".into(),
            })
        } else {
            Err(ApiError {
                message: "not found".into(),
            })
        }
    }
}

#[tokio::test]
async fn api_impl() -> anyhow::Result<()> {
    let srv = Server;
    let router = gropius::Router::builder()
        .with_endpoints(HealthApi::endpoints(&srv))
        .with_endpoints_at("/v1", WidgetApi::endpoints(&srv))
        .build()?;

    // Health check at root.
    {
        let req = http::Request::get("/healthz").body(bytes::Bytes::new())?;
        let resp = router.dispatch(req).await;
        assert_eq!(resp.status(), 200);
    }

    // List widgets at /v1 prefix.
    {
        let req = http::Request::get("/v1/widgets").body(bytes::Bytes::new())?;
        let resp = router.dispatch(req).await;
        assert_eq!(resp.status(), 200);
        let body: Vec<Widget> = serde_json::from_slice(resp.body())?;
        assert_eq!(body.len(), 2);
        assert_eq!(body[0].name, "sprocket");
    }

    // Get widget by ID.
    {
        let req = http::Request::get("/v1/widgets/1").body(bytes::Bytes::new())?;
        let resp = router.dispatch(req).await;
        assert_eq!(resp.status(), 200);
        let body: Widget = serde_json::from_slice(resp.body())?;
        assert_eq!(body.id, 1);
    }

    // Widget not found.
    {
        let req = http::Request::get("/v1/widgets/99").body(bytes::Bytes::new())?;
        let resp = router.dispatch(req).await;
        assert_eq!(resp.status(), 500);
    }

    // /v1/healthz doesn't exist.
    {
        let req = http::Request::get("/v1/healthz").body(bytes::Bytes::new())?;
        let resp = router.dispatch(req).await;
        assert_eq!(resp.status(), 404);
    }

    // /widgets without prefix doesn't exist.
    {
        let req = http::Request::get("/widgets").body(bytes::Bytes::new())?;
        let resp = router.dispatch(req).await;
        assert_eq!(resp.status(), 404);
    }

    // POST to a GET-only endpoint.
    {
        let req = http::Request::post("/healthz").body(bytes::Bytes::new())?;
        let resp = router.dispatch(req).await;
        assert_eq!(resp.status(), 405);
        assert_eq!(resp.headers().get("allow").unwrap(), "GET");
    }

    Ok(())
}

#[test]
fn openapi_spec() -> anyhow::Result<()> {
    let spec = gropius::Specification::new("WidgetService", "1.0.0")
        .with_endpoints(HEALTH_API_SPEC)
        .with_endpoints_at("/v1", WIDGET_API_SPEC)
        .generate()?;

    insta::assert_yaml_snapshot!(spec);

    let json = serde_json::to_string_pretty(&spec)?;
    let parsed: oas3::Spec = oas3::from_json(&json)?;
    assert_eq!(spec, parsed);

    Ok(())
}
