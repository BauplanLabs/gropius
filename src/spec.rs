use oas3::Map;
use oas3::spec::{
    Components, Info, MediaType, ObjectOrReference, Operation, Parameter, ParameterIn, PathItem,
    RequestBody, Response, Schema, Spec, Tag,
};
use schemars::SchemaGenerator;

use crate::generated;

/// An error encountered while generating an OpenAPI specification.
#[derive(Debug, thiserror::Error)]
pub enum SpecError {
    /// The JSON schema is not compatible with OpenAPI.
    #[error("the json schema for `{0}` is not a valid OpenAPI schema: {1}")]
    Schema(String, serde_json::Error),
    /// An error was encountered while serializing the spec to JSON.
    #[error("failed to serialize spec: {0}")]
    Json(#[from] serde_json::Error),
    /// An error was encountered while serializing the spec to YAML.
    #[error("failed to serialize spec as YAML: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),
    /// The HTTP method is unsupported by OpenAPI.
    #[error("unsupported HTTP method: {0}")]
    UnsupportedMethod(http::Method),
}

/// A collated OpenAPI 3.1.x specification.
#[derive(Debug, Clone)]
pub struct Specification {
    title: String,
    version: String,
    groups: Vec<(&'static str, generated::Api)>,
}

impl Specification {
    /// Create a new empty specification.
    pub fn new(title: impl Into<String>, version: impl Into<String>) -> Self {
        Specification {
            title: title.into(),
            version: version.into(),
            groups: Vec::new(),
        }
    }

    /// Add endpoints to the specification.
    pub fn with_endpoints(self, endpoints: generated::Api) -> Self {
        self.with_endpoints_at("", endpoints)
    }

    /// Add endpoints to the specification with a path prefix.
    pub fn with_endpoints_at(mut self, prefix: &'static str, endpoints: generated::Api) -> Self {
        self.groups.push((prefix, endpoints));
        self
    }

    /// Generate the OpenAPI specification.
    pub fn generate(self) -> Result<Spec, SpecError> {
        let mut generator = schemars::generate::SchemaSettings::openapi3().into_generator();
        let mut paths: Map<String, PathItem> = Map::new();
        let mut tags: Vec<Tag> = Vec::new();

        for (prefix, group) in &self.groups {
            let prefix = prefix.strip_suffix('/').unwrap_or(prefix);

            for tag_name in group.attr.tags {
                tags.push(Tag {
                    name: tag_name.to_string(),
                    description: group.attr.doc.map(String::from),
                    extensions: Map::new(),
                });
            }

            for ep in group.endpoints {
                let operation = build_operation(ep, group.attr.tags, &mut generator)?;
                let mut path = prefix.to_string();
                path.push_str(ep.path);

                let item = if !paths.contains_key(&path) {
                    paths.insert(path.clone(), PathItem::default());
                    paths.get_mut(&path).unwrap()
                } else {
                    paths.get_mut(&path).unwrap()
                };

                let slot = match *ep.method {
                    http::Method::GET => &mut item.get,
                    http::Method::PUT => &mut item.put,
                    http::Method::POST => &mut item.post,
                    http::Method::DELETE => &mut item.delete,
                    http::Method::PATCH => &mut item.patch,
                    http::Method::HEAD => &mut item.head,
                    http::Method::OPTIONS => &mut item.options,
                    http::Method::TRACE => &mut item.trace,
                    _ => return Err(SpecError::UnsupportedMethod(ep.method.clone())),
                };

                *slot = Some(operation);
            }
        }

        let schemas: Map<String, Schema> = generator
            .take_definitions(true)
            .into_iter()
            .map(|(name, schema)| {
                let s: Schema = serde_json::from_value(schema)
                    .map_err(|e| SpecError::Schema(name.clone(), e))?;
                Ok((name, s))
            })
            .collect::<Result<_, SpecError>>()?;

        let components = if schemas.is_empty() {
            None
        } else {
            Some(Components {
                schemas,
                ..Default::default()
            })
        };

        Ok(Spec {
            openapi: "3.1.0".to_string(),
            info: Info {
                title: self.title,
                summary: None,
                description: None,
                terms_of_service: None,
                version: self.version,
                contact: None,
                license: None,
                extensions: Map::new(),
            },
            paths: Some(paths),
            tags,
            components,
            servers: Vec::new(),
            security: Vec::new(),
            webhooks: Map::new(),
            external_docs: None,
            extensions: Map::new(),
        })
    }

    /// Generate the OpenAPI specification as JSON.
    pub fn generate_json(self) -> Result<String, SpecError> {
        Ok(serde_json::to_string(&self.generate()?)?)
    }

    /// Generate the OpenAPI specification as prettified JSON.
    pub fn generate_json_pretty(self) -> Result<String, SpecError> {
        Ok(serde_json::to_string_pretty(&self.generate()?)?)
    }

    /// Generate the OpenAPI specification as YAML.
    pub fn generate_yaml(self) -> Result<String, SpecError> {
        Ok(serde_yaml_ng::to_string(&self.generate()?)?)
    }
}

fn build_operation(
    ep: &generated::Endpoint,
    tags: &[&str],
    generator: &mut SchemaGenerator,
) -> Result<Operation, SpecError> {
    let mut parameters = Vec::new();

    if let Some(schema_fn) = ep.query_type {
        parameters.extend(params_from_schema(
            schema_fn,
            ParameterIn::Query,
            generator,
        )?);
    }

    if let Some(schema_fn) = ep.path_type {
        parameters.extend(params_from_schema(schema_fn, ParameterIn::Path, generator)?);
    }

    let request_body = match ep.request_type {
        Some(schema_fn) => {
            let schema = resolve_schema(schema_fn, generator)?;
            Some(ObjectOrReference::Object(RequestBody {
                content: json_content(schema),
                required: Some(true),
                ..Default::default()
            }))
        }
        None => None,
    };

    let error_schema = resolve_schema(ep.error_type, generator)?;

    let mut responses = Map::new();

    let success_response = match ep.response_type {
        generated::ResponseType::Json(schema_fn) => {
            let schema = resolve_schema(schema_fn, generator)?;
            Response {
                description: Some("successful operation".to_string()),
                content: json_content(schema),
                ..Default::default()
            }
        }
        generated::ResponseType::Empty => Response {
            description: Some("successful operation".to_string()),
            ..Default::default()
        },
        generated::ResponseType::Raw(content_type) => {
            let mut resp = Response {
                description: Some("successful operation".to_string()),
                ..Default::default()
            };

            if let Some(ct) = content_type {
                let mut content = Map::new();
                content.insert(ct.to_string(), MediaType::default());
                resp.content = content;
            }
            resp
        }
    };
    responses.insert(
        "200".to_string(),
        ObjectOrReference::Object(success_response),
    );
    responses.insert(
        "default".to_string(),
        ObjectOrReference::Object(Response {
            description: Some("error".to_string()),
            content: json_content(error_schema),
            ..Default::default()
        }),
    );

    Ok(Operation {
        operation_id: Some(ep.name.to_string()),
        description: ep.doc.map(String::from),
        tags: tags.iter().map(|t| t.to_string()).collect(),
        parameters,
        request_body,
        responses: Some(responses),
        ..Default::default()
    })
}

/// Generate a schema, producing a `$ref` for named types.
fn resolve_schema(
    schema_fn: generated::SchemaFn,
    generator: &mut SchemaGenerator,
) -> Result<Schema, SpecError> {
    Ok(to_oas3_schema(schema_fn(generator))?)
}

/// Pull apart an object schema's properties into individual OpenAPI
/// parameters.
fn params_from_schema(
    schema_fn: generated::SchemaFn,
    location: ParameterIn,
    generator: &mut SchemaGenerator,
) -> Result<Vec<ObjectOrReference<Parameter>>, SpecError> {
    let schema = resolve_inline(schema_fn, generator)?;

    let Schema::Object(boxed) = schema else {
        return Ok(Vec::new());
    };
    let ObjectOrReference::Object(obj) = *boxed else {
        return Ok(Vec::new());
    };

    let is_path = location == ParameterIn::Path;

    Ok(obj
        .properties
        .into_iter()
        .map(|(name, prop_schema)| {
            ObjectOrReference::Object(Parameter {
                name: name.clone(),
                location,
                description: None,
                required: Some(is_path || obj.required.contains(&name)),
                deprecated: None,
                allow_empty_value: None,
                style: None,
                explode: None,
                allow_reserved: None,
                schema: Some(prop_schema),
                example: None,
                examples: Map::new(),
                content: None,
                extensions: Map::new(),
            })
        })
        .collect())
}

/// Resolve a schema function to an inline oas3 Schema, following `$ref`s
/// into the generator's definitions.
fn resolve_inline(
    schema_fn: generated::SchemaFn,
    generator: &mut SchemaGenerator,
) -> Result<Schema, SpecError> {
    let schema = to_oas3_schema(schema_fn(generator))?;

    // If the generator produced a $ref, look up the actual definition.
    if let Schema::Object(boxed) = &schema
        && let ObjectOrReference::Ref { ref_path, .. } = boxed.as_ref()
    {
        let name = ref_path.rsplit('/').next().unwrap_or(ref_path);
        if let Some(def) = generator.definitions().get(name) {
            return Ok(to_oas3_schema(schemars::Schema::from(
                def.as_object().cloned().unwrap_or_default(),
            ))?);
        }
    }

    Ok(schema)
}

fn to_oas3_schema(schema: schemars::Schema) -> Result<Schema, serde_json::Error> {
    serde_json::from_value(serde_json::Value::from(schema))
}

fn json_content(schema: Schema) -> Map<String, MediaType> {
    let mut content = Map::new();
    content.insert(
        "application/json".to_string(),
        MediaType {
            schema: Some(schema),
            ..Default::default()
        },
    );
    content
}
