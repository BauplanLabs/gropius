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
trait Api {
    async fn missing_endpoint(&self) -> Result<(), MyError>;
}

fn main() {}
