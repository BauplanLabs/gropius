use bytes::Bytes;
use gropius::{Binary, MultipartBody};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct WidgetSpec {
    name: String,
    fidgets: bool,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct Widget {
    name: String,
    size: usize,
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

#[derive(JsonSchema)]
#[allow(dead_code)]
struct CreateWidgetBody {
    spec: WidgetSpec,
    file: Binary,
}

/// The widget API.
#[gropius::api(tags = ["widgets"])]
trait WidgetApi {
    /// Create a widget from a spec and its contents.
    #[endpoint(POST, "/widgets")]
    async fn create_widget(
        &self,
        body: MultipartBody<CreateWidgetBody>,
    ) -> Result<Widget, ApiError>;
}

#[derive(Clone)]
struct Server;

impl WidgetApi for Server {
    async fn create_widget(
        &self,
        mut body: MultipartBody<CreateWidgetBody>,
    ) -> Result<Widget, ApiError> {
        let mut spec: Option<WidgetSpec> = None;
        let mut file = None;

        while let Some(field) = body.next_field().await.map_err(|e| ApiError {
            message: e.to_string(),
        })? {
            let read = |e: gropius::RouterError| ApiError {
                message: e.to_string(),
            };

            match field.name() {
                Some("spec") => {
                    let bytes = field.bytes().await.map_err(read)?;
                    spec = serde_json::from_slice(&bytes).ok();
                }
                Some("file") => file = Some(field.bytes().await.map_err(read)?),
                _ => (),
            }
        }

        let (Some(spec), Some(file)) = (spec, file) else {
            return Err(ApiError {
                message: "missing part".into(),
            });
        };

        Ok(Widget {
            name: spec.name,
            size: file.len(),
        })
    }
}

fn multipart_body(boundary: &str, parts: &[(&str, &str, &[u8])]) -> Bytes {
    let mut body = Vec::new();
    for (name, content_type, contents) in parts {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{name}\"\r\n").as_bytes(),
        );
        body.extend_from_slice(format!("Content-Type: {content_type}\r\n\r\n").as_bytes());
        body.extend_from_slice(contents);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    Bytes::from(body)
}

fn multipart_request(boundary: &str, body: Bytes) -> http::Request<Bytes> {
    http::Request::post("/widgets")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(body)
        .unwrap()
}

#[tokio::test]
async fn create() -> anyhow::Result<()> {
    let router = gropius::Router::new(Server.endpoints())?;

    let body = multipart_body(
        "xYzZY",
        &[
            (
                "spec",
                "application/json",
                br#"{"name": "w1", "fidgets": true}"#,
            ),
            ("file", "application/octet-stream", b"\x01\x02\x03\x04"),
        ],
    );

    let resp = router.dispatch(multipart_request("xYzZY", body)).await;
    assert_eq!(resp.status(), 200);

    let widget: Widget = serde_json::from_slice(resp.body())?;
    assert_eq!(widget.name, "w1");
    assert_eq!(widget.size, 4);

    Ok(())
}

#[tokio::test]
async fn malformed_multipart_body() -> anyhow::Result<()> {
    let router = gropius::Router::new(Server.endpoints())?;

    let resp = router
        .dispatch(multipart_request(
            "xYzZY",
            Bytes::from_static(b"this is not multipart at all"),
        ))
        .await;
    assert_eq!(resp.status(), 500);

    Ok(())
}

#[tokio::test]
async fn missing_content_type() -> anyhow::Result<()> {
    let router = gropius::Router::new(Server.endpoints())?;

    let req = http::Request::post("/widgets")
        .body(Bytes::from_static(b"not multipart"))
        .unwrap();

    let resp = router.dispatch(req).await;
    assert_eq!(resp.status(), 415);

    Ok(())
}

// A spec-only trait: `MultipartBody` with no schema type falls back to a
// bare `multipart/form-data` request body.
#[gropius::api]
#[allow(dead_code)]
trait BareWidgetApi {
    #[endpoint(POST, "/widgets")]
    async fn create_widget(&self, body: MultipartBody) -> Result<Widget, ApiError>;
}

#[test]
fn spec_without_schema_type() -> anyhow::Result<()> {
    let spec = gropius::Specification::new("BareWidgetApi", "0.1.0")
        .with_endpoints(BARE_WIDGET_API_SPEC)
        .generate()?;

    insta::assert_yaml_snapshot!(spec);

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
