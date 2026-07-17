use schemars::JsonSchema;
use serde::Serialize;

#[derive(Serialize, JsonSchema, gropius::ApiError)]
enum ChairError {
    #[api_error(400)]
    WrongYear(String),
    NotFound(String),
    Stolen(String),
}

fn main() {}
