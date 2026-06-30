use schemars::JsonSchema;
use serde::Serialize;

#[derive(Serialize, JsonSchema)]
struct MyError;

impl gropius::ApiError for MyError {
    fn status_code(&self) -> http::StatusCode {
        http::StatusCode::INTERNAL_SERVER_ERROR
    }
}

// Each bad path should produce its own diagnostic.
#[gropius::api]
trait Api {
    #[endpoint(GET, "/files/{*rest}")]
    async fn catch_all(&self) -> Result<(), MyError>;

    #[endpoint(GET, "/literal/{{brace}}")]
    async fn escaped_braces(&self) -> Result<(), MyError>;

    #[endpoint(GET, "/dup")]
    async fn dup_one(&self) -> Result<(), MyError>;

    #[endpoint(GET, "/dup")]
    async fn dup_two(&self) -> Result<(), MyError>;

    #[endpoint(GET, "/items/{id}")]
    async fn by_id(&self) -> Result<(), MyError>;

    #[endpoint(GET, "/items/{name}")]
    async fn by_name(&self) -> Result<(), MyError>;
}

fn main() {}
