use schemars::JsonSchema;
use serde::Serialize;

#[derive(Serialize, JsonSchema)]
struct Widget;

#[derive(Serialize, JsonSchema, gropius::ApiError)]
#[api_error(500)]
struct ApiError;

#[gropius::api(client(blocking))]
trait WidgetApi {
    #[endpoint(GET, "/widgets")]
    async fn list_widgets(&self) -> Result<Widget, ApiError>;
}

fn main() {}
