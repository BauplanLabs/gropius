#![cfg(any(feature = "client-reqwest", feature = "client-ureq"))]

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::OnceLock;

use gropius::client::ClientError;
use gropius::{Body, EmptyResponse, Path, Query, Response};
use hyper::server::conn::http1;
use hyper_util::{rt::TokioIo, service::TowerToHyperService};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::net::TcpListener;

#[derive(Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
struct Widget {
    id: u64,
    name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
struct CreateWidget {
    name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
struct ListQuery {
    starts_with: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, JsonSchema, gropius::ApiError, Error)]
#[serde(tag = "error", content = "message")]
enum WidgetError {
    #[api_error(404)]
    #[error("not found")]
    NotFound,
}

#[derive(Serialize, Deserialize, JsonSchema)]
struct Shelf {
    aisle: String,
    slot: u64,
}

#[derive(Serialize, Deserialize, JsonSchema)]
struct Echo {
    text: String,
}

// The API under test. Its handlers back both clients, and it generates the
// async client.
#[gropius::api(client(async, cfg(feature = "client-reqwest")))]
trait WidgetApi {
    #[endpoint(GET, "/widgets/{id}")]
    async fn get_widget(&self, path: Path<u64>) -> Result<Widget, WidgetError>;

    #[endpoint(GET, "/catalog/{kind}/{id}")]
    async fn catalog(&self, path: Path<(String, u64)>) -> Result<Widget, WidgetError>;

    // A parameter named after a Rust keyword becomes a raw identifier.
    #[endpoint(GET, "/kinds/{type}")]
    async fn by_kind(&self, path: Path<String>) -> Result<Widget, WidgetError>;

    #[endpoint(GET, "/shelf/{aisle}/{slot}")]
    async fn shelf(&self, path: Path<Shelf>) -> Result<Widget, WidgetError>;

    #[endpoint(GET, "/widgets")]
    async fn list_widgets(&self, query: Query<ListQuery>) -> Result<Vec<Widget>, WidgetError>;

    #[endpoint(GET, "/echo")]
    async fn echo(&self, query: Query<Echo>) -> Result<Echo, WidgetError>;

    #[endpoint(POST, "/widgets")]
    async fn create_widget(&self, body: Body<CreateWidget>) -> Result<Widget, WidgetError>;

    #[endpoint(DELETE, "/widgets/{id}")]
    async fn delete_widget(&self, path: Path<u64>) -> Result<EmptyResponse, WidgetError>;

    #[endpoint(GET, "/image", content_type = "image/png")]
    async fn image(&self) -> Result<Response, WidgetError>;

    #[endpoint(GET, "/health")]
    async fn health(&self) -> Result<Widget, Infallible>;
}

// The macro emits one client per trait, so exercising the blocking client needs
// a second trait. It mirrors `WidgetApi`'s routes and is served by the same
// handlers, so both clients are tested against identical behavior. The trait
// itself is only a vehicle for the generated client, hence `dead_code`.
#[gropius::api(client)]
#[allow(dead_code)]
trait BlockingWidgetApi {
    #[endpoint(GET, "/widgets/{id}")]
    async fn get_widget(&self, path: Path<u64>) -> Result<Widget, WidgetError>;

    #[endpoint(GET, "/catalog/{kind}/{id}")]
    async fn catalog(&self, path: Path<(String, u64)>) -> Result<Widget, WidgetError>;

    #[endpoint(GET, "/kinds/{type}")]
    async fn by_kind(&self, path: Path<String>) -> Result<Widget, WidgetError>;

    #[endpoint(GET, "/shelf/{aisle}/{slot}")]
    async fn shelf(&self, path: Path<Shelf>) -> Result<Widget, WidgetError>;

    #[endpoint(GET, "/widgets")]
    async fn list_widgets(&self, query: Query<ListQuery>) -> Result<Vec<Widget>, WidgetError>;

    #[endpoint(GET, "/echo")]
    async fn echo(&self, query: Query<Echo>) -> Result<Echo, WidgetError>;

    #[endpoint(POST, "/widgets")]
    async fn create_widget(&self, body: Body<CreateWidget>) -> Result<Widget, WidgetError>;

    #[endpoint(DELETE, "/widgets/{id}")]
    async fn delete_widget(&self, path: Path<u64>) -> Result<EmptyResponse, WidgetError>;

    #[endpoint(GET, "/image", content_type = "image/png")]
    async fn image(&self) -> Result<Response, WidgetError>;

    #[endpoint(GET, "/health")]
    async fn health(&self) -> Result<Widget, Infallible>;
}

#[derive(Clone)]
struct Server;

impl WidgetApi for Server {
    async fn get_widget(&self, path: Path<u64>) -> Result<Widget, WidgetError> {
        if *path == 1 {
            Ok(Widget {
                id: 1,
                name: "sprocket".into(),
            })
        } else {
            Err(WidgetError::NotFound)
        }
    }

    async fn catalog(&self, path: Path<(String, u64)>) -> Result<Widget, WidgetError> {
        Ok(Widget {
            id: path.1,
            name: format!("{}-{}", path.0, path.1),
        })
    }

    async fn by_kind(&self, path: Path<String>) -> Result<Widget, WidgetError> {
        Ok(Widget {
            id: 0,
            name: (*path).clone(),
        })
    }

    async fn shelf(&self, path: Path<Shelf>) -> Result<Widget, WidgetError> {
        Ok(Widget {
            id: path.slot,
            name: format!("{}-{}", path.aisle, path.slot),
        })
    }

    async fn list_widgets(&self, query: Query<ListQuery>) -> Result<Vec<Widget>, WidgetError> {
        let all = [
            Widget {
                id: 1,
                name: "sprocket".into(),
            },
            Widget {
                id: 2,
                name: "gizmo".into(),
            },
        ];

        Ok(all
            .into_iter()
            .filter(|w| match &query.starts_with {
                Some(prefix) => w.name.starts_with(prefix),
                None => true,
            })
            .collect())
    }

    async fn echo(&self, query: Query<Echo>) -> Result<Echo, WidgetError> {
        Ok(Echo {
            text: query.text.clone(),
        })
    }

    async fn create_widget(&self, body: Body<CreateWidget>) -> Result<Widget, WidgetError> {
        Ok(Widget {
            id: 42,
            name: body.name.clone(),
        })
    }

    async fn delete_widget(&self, _path: Path<u64>) -> Result<EmptyResponse, WidgetError> {
        Ok(EmptyResponse)
    }

    async fn image(&self) -> Result<Response, WidgetError> {
        Ok(http::Response::builder()
            .status(200)
            .header("content-type", "image/png")
            .body(bytes::Bytes::from_static(b"\x89PNG"))
            .unwrap())
    }

    async fn health(&self) -> Result<Widget, Infallible> {
        Ok(Widget {
            id: 0,
            name: "ok".into(),
        })
    }
}

/// Start the server once in a background thread and return its base URL.
fn server_url() -> &'static str {
    static URL: OnceLock<String> = OnceLock::new();

    URL.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async move {
                let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
                let addr: SocketAddr = listener.local_addr().unwrap();
                tx.send(addr).unwrap();

                let router = gropius::Router::builder()
                    .with_endpoints(WidgetApi::endpoints(&Server))
                    .build()
                    .unwrap();

                loop {
                    let (stream, _) = listener.accept().await.unwrap();
                    let service = TowerToHyperService::new(router.clone());
                    tokio::spawn(async move {
                        let _ = http1::Builder::new()
                            .serve_connection(TokioIo::new(stream), service)
                            .await;
                    });
                }
            });
        });

        format!("http://{}", rx.recv().unwrap())
    })
}

#[cfg(feature = "client-reqwest")]
#[tokio::test]
async fn async_client() -> anyhow::Result<()> {
    let client = WidgetApiClient::new(server_url());

    // Path parameters: scalar, tuple, struct, and a keyword-named segment.
    assert_eq!(client.get_widget(1).await?.name, "sprocket");
    assert_eq!(client.catalog(("chairs".into(), 7)).await?.name, "chairs-7");
    assert_eq!(
        client
            .shelf(Shelf {
                aisle: "A".into(),
                slot: 3,
            })
            .await?
            .name,
        "A-3"
    );
    assert_eq!(client.by_kind("bolt".into()).await?.name, "bolt");

    // Reserved characters in a path segment round-trip through percent-encoding.
    assert_eq!(
        client.catalog(("a b/c~d.e".into(), 7)).await?.name,
        "a b/c~d.e-7"
    );

    // Query parameters, including a value round-tripped through url-encoding.
    let found = client
        .list_widgets(ListQuery {
            starts_with: Some("giz".into()),
        })
        .await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "gizmo");
    assert_eq!(
        client
            .echo(Echo {
                text: "a b & c=d".into(),
            })
            .await?
            .text,
        "a b & c=d"
    );

    // Request body.
    let created = client
        .create_widget(CreateWidget { name: "cog".into() })
        .await?;
    assert_eq!(created.id, 42);
    assert_eq!(created.name, "cog");

    // Empty response deserializes to the unit type.
    let () = client.delete_widget(1).await?;

    // Raw response keeps the status, headers, and body.
    let image = client.image().await?;
    assert_eq!(image.status(), 200);
    assert_eq!(image.headers().get("content-type").unwrap(), "image/png");
    assert_eq!(image.body().as_ref(), b"\x89PNG");

    // Infallible endpoint.
    assert_eq!(client.health().await?.name, "ok");

    // A 404 deserializes into the endpoint's error type.
    let err = client.get_widget(99).await.unwrap_err();
    assert!(matches!(err, ClientError::Api(WidgetError::NotFound)));

    Ok(())
}

#[test]
fn blocking_client() -> anyhow::Result<()> {
    let client = BlockingWidgetApiClient::new(server_url());

    // Path parameters: scalar, tuple, struct, and a keyword-named segment.
    assert_eq!(client.get_widget(1)?.name, "sprocket");
    assert_eq!(client.catalog(("chairs".into(), 7))?.name, "chairs-7");
    assert_eq!(
        client
            .shelf(Shelf {
                aisle: "A".into(),
                slot: 3,
            })?
            .name,
        "A-3"
    );
    assert_eq!(client.by_kind("bolt".into())?.name, "bolt");

    // Reserved characters in a path segment round-trip through percent-encoding.
    assert_eq!(client.catalog(("a b/c~d.e".into(), 7))?.name, "a b/c~d.e-7");

    // Query parameters, including a value round-tripped through url-encoding.
    let found = client.list_widgets(ListQuery {
        starts_with: Some("giz".into()),
    })?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "gizmo");
    assert_eq!(
        client
            .echo(Echo {
                text: "a b & c=d".into(),
            })?
            .text,
        "a b & c=d"
    );

    // Request body.
    let created = client.create_widget(CreateWidget { name: "cog".into() })?;
    assert_eq!(created.id, 42);
    assert_eq!(created.name, "cog");

    // Empty response deserializes to the unit type.
    let () = client.delete_widget(1)?;

    // Raw response keeps the status, headers, and body.
    let image = client.image()?;
    assert_eq!(image.status(), 200);
    assert_eq!(image.headers().get("content-type").unwrap(), "image/png");
    assert_eq!(image.body().as_ref(), b"\x89PNG");

    // Infallible endpoint.
    assert_eq!(client.health()?.name, "ok");

    // A 404 deserializes into the endpoint's error type.
    let err = client.get_widget(99).unwrap_err();
    assert!(matches!(err, ClientError::Api(WidgetError::NotFound)));

    Ok(())
}
