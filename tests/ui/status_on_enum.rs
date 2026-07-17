use schemars::JsonSchema;
use serde::Serialize;

#[derive(Serialize, JsonSchema, gropius::ApiError)]
#[api_error(400)]
enum ChairError {
    #[api_error(400)]
    WrongYear(String),
}

fn main() {}
