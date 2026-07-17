use schemars::JsonSchema;
use serde::Serialize;

#[derive(Serialize, JsonSchema, gropius::ApiError)]
enum ChairError {
    #[api_error(code = 400)]
    WrongYear(String),
}

fn main() {}
