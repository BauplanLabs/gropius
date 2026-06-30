![Bauhaus-Archiv Darmstadt (1964–1968), Preliminary planning for a plot of land on Rosenhöhe, view from the north-west, Architects: Walter Gropius and Louis McMillen, 1964](docs/gropius.png)

# Gropius

Gropius is a Rust toolkit for designing APIs. It's compatible with OpenAPI 3.1.x, but, unlike most toolkits, it lets you generate the spec from the code, instead of the code from the spec:

```rust
/// A chair designed by Walter Gropius.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct Chair {
    id: String,
    year: u32,
    description: String,
}

#[gropius::api(tags = ["chairs"])]
/// Explore chairs designed by Walter Gropius.
trait ChairAPI {
    /// List chairs by year.
    #[endpoint(GET, "/chairs/{year}")]
    async fn list_widgets(&self, path: gropius::Path<u16>) -> Result<Vec<Chair>, ApiError>;

    /// Get a chair by ID.
    #[endpoint(GET, "/widgets/{year}/{id}")]
    async fn get_widget(&self, path: gropius::Path<(u64, String)>) -> Result<Widget, ApiError>;
}
```

For more details, head to the [docs](https://docs.rs/gropius).

## Features

The crate is new and is currently **alpha quality**. However, it has several distinguishing features compared to similar libraries:

 - Integrates with `tower::Service` and hyper
 - Supports grouping endpoints into multiple traits, which can then be combined in a single API surface
 - Supports OpenAPI 3.1.x, with 3.2.x support planned

## Similar crates

Gropius is inspired by and indebted to these libraries, which have a similar approach:

 - [`oxidecomputer/dropshot`](https://github.com/oxidecomputer/dropshot)
 - [`juhaku/utoipa`](https://github.com/juhaku/utoipa)
