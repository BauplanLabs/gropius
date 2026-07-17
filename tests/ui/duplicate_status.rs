use schemars::JsonSchema;
use serde::Serialize;

#[derive(Serialize, JsonSchema, gropius::ApiError)]
enum ChairError {
    #[api_error(400)]
    #[api_error(404)]
    WrongYear(String),
}

fn main() {}
