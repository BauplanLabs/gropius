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
struct Pagination;

#[derive(Deserialize, JsonSchema)]
struct WidgetId;

#[gropius::api]
trait Api {
    #[endpoint(GET, "/widgets/{id}")]
    async fn out_of_order(
        &self,
        query: gropius::Query<Pagination>,
        path: gropius::Path<WidgetId>,
    ) -> Result<(), MyError>;
}

fn main() {}
