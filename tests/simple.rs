use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// todo
// merge multiple traits
// raw response type (http::Response)
// empty response
// tuples for path

#[derive(Default, Clone, Serialize, Deserialize, JsonSchema)]
struct GetFooError {
    msg: String,
}

impl gropius::ApiError for GetFooError {
    fn status_code(&self) -> http::StatusCode {
        http::StatusCode::INTERNAL_SERVER_ERROR
    }
}

#[derive(Default, Clone, Serialize, Deserialize, JsonSchema)]
struct FooResponse {
    bar: usize,
    baz: String,
}

#[derive(Default, Clone, Serialize, Deserialize, JsonSchema)]
struct GetFooQuery {
    pretty_please: bool,
}

#[gropius::api]
trait FooApi {
    #[endpoint(GET, "/v1/foo")]
    async fn get_foo(&self, query: gropius::Query<GetFooQuery>)
    -> Result<FooResponse, GetFooError>;
}

#[derive(Default, Clone)]
struct Server;

impl FooApi for Server {
    async fn get_foo(
        &self,
        query: gropius::Query<GetFooQuery>,
    ) -> Result<FooResponse, GetFooError> {
        if query.pretty_please {
            Ok(FooResponse {
                bar: 123,
                baz: "hello world".to_string(),
            })
        } else {
            Err(GetFooError {
                msg: "what's the magic word?".to_string(),
            })
        }
    }
}

#[tokio::test]
async fn api_impl() -> anyhow::Result<()> {
    let srv = Server {};
    let router = gropius::Router::builder()
        .with_endpoints(srv.endpoints())
        .build()?;

    {
        let req = http::Request::builder()
            .method(http::Method::GET)
            .uri("/v1/foo?pretty_please=false")
            .body(bytes::Bytes::new())
            .unwrap();

        let resp = router.dispatch(req).await;

        assert_eq!(resp.status(), 500);
        let body: GetFooError = serde_json::from_slice(resp.body())?;
        assert_eq!(body.msg, "what's the magic word?");
    }

    {
        let req = http::Request::builder()
            .method(http::Method::GET)
            .uri("/v1/foo?pretty_please=true")
            .body(bytes::Bytes::new())
            .unwrap();

        let resp = router.dispatch(req).await;

        assert_eq!(resp.status(), 200);
        let body: FooResponse = serde_json::from_slice(resp.body())?;
        assert_eq!(body.bar, 123);
        assert_eq!(body.baz, "hello world");
    }

    Ok(())
}

#[test]
fn openapi_spec() -> anyhow::Result<()> {
    let spec = gropius::Specification::new("FooApi", "0.1.0")
        .with_endpoints(FOO_API_SPEC)
        .generate()?;

    insta::assert_yaml_snapshot!(spec);

    let json = serde_json::to_string_pretty(&spec)?;
    let parsed: oas3::Spec = oas3::from_json(&json)?;
    assert_eq!(spec, parsed);

    Ok(())
}
