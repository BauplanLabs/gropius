use schemars::JsonSchema;
use serde::Serialize;

#[derive(Serialize, JsonSchema, gropius::ApiError)]
enum ChairError {
    #[api_error(transparent)]
    WrongYear { year: u32 },
}

fn main() {}
