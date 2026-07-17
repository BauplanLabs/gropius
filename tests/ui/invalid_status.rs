use schemars::JsonSchema;
use serde::Serialize;

#[derive(Serialize, JsonSchema, gropius::ApiError)]
enum ChairError {
    #[api_error(42)]
    WrongYear(String),
}

fn main() {}
