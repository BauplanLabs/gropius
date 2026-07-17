//! Serving a gropius API with axum.

use axum::routing::get;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

#[derive(Serialize, Deserialize, JsonSchema)]
struct Id(u64);

#[derive(Serialize, Deserialize, JsonSchema)]
struct Widget {
    id: u64,
    name: String,
}

#[derive(Serialize, JsonSchema, gropius::ApiError)]
#[api_error(404)]
struct WidgetError {
    error: String,
}

#[gropius::api]
trait WidgetApi {
    #[endpoint(GET, "/widgets/{id}")]
    async fn get_widget(&self, path: gropius::Path<Id>) -> Result<Widget, WidgetError>;
}

#[derive(Clone)]
struct Server;

impl WidgetApi for Server {
    async fn get_widget(&self, path: gropius::Path<Id>) -> Result<Widget, WidgetError> {
        Ok(Widget {
            id: path.0,
            name: format!("Widget #{}", path.0),
        })
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let srv = Server;
    let gropius = gropius::Router::builder()
        .with_endpoints_at("/v1", srv.endpoints())
        .build()?;

    let app = axum::Router::new()
        .route("/health", get(|| async { "gropius + axum" }))
        .fallback_service(gropius);

    let listener = TcpListener::bind("127.0.0.1:3000").await?;
    eprintln!("listening on http://{}", listener.local_addr()?);

    axum::serve(listener, app).await?;
    Ok(())
}
