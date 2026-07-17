//! Serving a gropius API with hyper.

use std::net::SocketAddr;

use hyper::server::conn::http1;
use hyper_util::{rt::TokioIo, service::TowerToHyperService};
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
    let router = gropius::Router::builder()
        .with_endpoints(srv.endpoints())
        .build()?;

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await?;
    eprintln!("listening on http://{addr}");

    loop {
        let (stream, _) = listener.accept().await?;
        let service = TowerToHyperService::new(router.clone());

        tokio::spawn(async move {
            if let Err(e) = http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service)
                .await
            {
                eprintln!("connection error: {e}");
            }
        });
    }
}
