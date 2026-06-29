use schemars::JsonSchema;
use serde::Serialize;

#[derive(Serialize, JsonSchema)]
struct MyError;

impl gropius::ApiError for MyError {
    fn status_code(&self) -> http::StatusCode {
        http::StatusCode::INTERNAL_SERVER_ERROR
    }
}

#[gropius::api]
trait EchoApi {
    #[endpoint(POST, "/echo")]
    async fn echo(&self, req: gropius::Request) -> Result<gropius::Response, MyError>;
}

#[derive(Clone)]
struct Server;

impl EchoApi for Server {
    async fn echo(&self, req: gropius::Request) -> Result<gropius::Response, MyError> {
        Ok(http::Response::new(req.into_body()))
    }
}

#[tokio::test]
async fn raw_request() -> anyhow::Result<()> {
    let router = gropius::Router::new(Server.endpoints())?;
    let req = http::Request::post("/echo").body(bytes::Bytes::from_static(b"hello"))?;
    let resp = router.dispatch(req).await;
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.body().as_ref(), b"hello");
    Ok(())
}
