use gropius::{Body, MultipartBody};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, JsonSchema)]
struct MyError;

impl gropius::ApiError for MyError {
    fn status_code(&self) -> http::StatusCode {
        http::StatusCode::INTERNAL_SERVER_ERROR
    }
}

#[derive(Deserialize, JsonSchema)]
struct WidgetSpec {
    name: String,
}

#[gropius::api]
trait WidgetApi {
    #[endpoint(POST, "/widgets")]
    async fn create_widget(
        &self,
        body: Body<WidgetSpec>,
        upload: MultipartBody,
    ) -> Result<(), MyError>;
}

fn main() {}
