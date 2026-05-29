use std::collections::BTreeMap;

use crate::domain;
use openapiv3::{
    APIKeyLocation, OpenAPI, Parameter, ParameterData, ParameterSchemaOrContent, ReferenceOr,
    Schema, SecurityScheme,
};

/// Parse an OpenAPI 3.x JSON or YAML string into a domain Collection.
///
/// The parsed spec creates one collection named after the spec title.
/// Operations are grouped by their first tag into folders. Operations
/// with no tags go at the root of the collection.
pub fn parse_openapi_spec(input: &str) -> domain::Result<domain::Collection> {
    // Try JSON first, fall back to YAML
    let spec: OpenAPI = serde_json::from_str(input)
        .or_else(|_| serde_yaml::from_str(input))
        .map_err(|e| domain::ClientError::Import(e.to_string()))?;

    let spec_name = if spec.info.title.is_empty() {
        "Imported Spec".to_string()
    } else {
        spec.info.title.clone()
    };

    // Extract server base URL(s)
    let base_urls: Vec<String> = spec
        .servers
        .iter()
        .map(|server| server.url.clone())
        .collect();
    let base_url = base_urls.first().cloned().unwrap_or_default();

    // Pre-process security schemes into a BTreeMap for easy lookup
    let security_schemes = spec
        .components
        .as_ref()
        .map(|c| extract_security_schemes(&c.security_schemes))
        .unwrap_or_default();

    // Track folders by tag name
    let mut folder_map: BTreeMap<String, Vec<domain::CollectionItem>> = BTreeMap::new();
    let mut root_items: Vec<domain::CollectionItem> = Vec::new();

    for (path, path_item) in &spec.paths.paths {
        let path_item = match path_item {
            ReferenceOr::Item(item) => item,
            ReferenceOr::Reference { .. } => continue,
        };

        let operations = [
            ("GET", &path_item.get),
            ("POST", &path_item.post),
            ("PUT", &path_item.put),
            ("PATCH", &path_item.patch),
            ("DELETE", &path_item.delete),
            ("OPTIONS", &path_item.options),
        ];

        for (method_str, operation) in operations {
            let Some(operation) = operation else {
                continue;
            };

            let request =
                operation_to_request(path, method_str, operation, &base_url, &security_schemes);

            let tags = &operation.tags;
            if tags.is_empty() {
                root_items.push(domain::CollectionItem::Request(request));
            } else {
                let tag = tags[0].clone();
                folder_map
                    .entry(tag)
                    .or_default()
                    .push(domain::CollectionItem::Request(request));
            }
        }
    }

    // Build items: folders first, then root operations
    let mut items: Vec<domain::CollectionItem> = Vec::new();
    for (tag_name, folder_items) in folder_map {
        items.push(domain::CollectionItem::Folder {
            id: generate_id(),
            name: tag_name,
            items: folder_items,
        });
    }
    items.extend(root_items);

    Ok(domain::Collection {
        id: generate_id(),
        name: spec_name,
        items,
    })
}

/// Copy security scheme references into a plain BTreeMap so we don't
/// need the `indexmap` crate in scope.
type SecuritySchemeMap = BTreeMap<String, SecurityScheme>;

fn extract_security_schemes(
    schemes: &indexmap::IndexMap<String, ReferenceOr<SecurityScheme>>,
) -> SecuritySchemeMap {
    let mut out = BTreeMap::new();
    for (name, scheme) in schemes {
        if let ReferenceOr::Item(s) = scheme {
            out.insert(name.clone(), s.clone());
        }
    }
    out
}

fn operation_to_request(
    path: &str,
    method_str: &str,
    operation: &openapiv3::Operation,
    base_url: &str,
    security_schemes: &SecuritySchemeMap,
) -> domain::Request {
    let operation_id = operation.operation_id.clone();
    let summary = operation.summary.clone();
    let description = operation.description.clone();
    let deprecated = operation.deprecated;
    let tags = operation.tags.clone();

    // Derive name: operation_id > summary > "METHOD /path"
    let name = operation_id
        .as_deref()
        .or(summary.as_deref())
        .unwrap_or(&format!("{method_str} {path}"))
        .to_string();

    let method = domain::Method::from(method_str);

    // Construct URL: base_url + path
    let url = if base_url.is_empty() {
        path.to_string()
    } else {
        let base = base_url.trim_end_matches('/');
        let p = if path.starts_with('/') {
            &path[1..]
        } else {
            path
        };
        format!("{base}/{p}")
    };

    // Parse parameters
    let mut query = Vec::new();
    let mut headers = Vec::new();
    let mut path_params = Vec::new();

    for param in &operation.parameters {
        let param = match param {
            ReferenceOr::Item(item) => item,
            ReferenceOr::Reference { .. } => continue,
        };
        map_parameter(param, &mut query, &mut headers, &mut path_params);
    }

    // Request body
    let body_str = operation
        .request_body
        .as_ref()
        .and_then(|rb| match rb {
            ReferenceOr::Item(body) => body.content.get("application/json"),
            ReferenceOr::Reference { .. } => None,
        })
        .and_then(|media_type| {
            // Check media_type.example first, then schema's example/default
            if let Some(ref example) = media_type.example {
                return serde_json::to_string_pretty(example).ok();
            }
            if let Some(ref schema_ref) = media_type.schema {
                let v = schema_example_value(schema_ref);
                v.and_then(|val| serde_json::to_string_pretty(&val).ok())
            } else {
                None
            }
        })
        .unwrap_or_default();

    let request_body = if body_str.is_empty() {
        domain::Body::Empty
    } else {
        domain::Body::raw_json(body_str)
    };

    // Auth
    let auth = resolve_auth(operation, security_schemes);

    domain::Request {
        id: generate_id(),
        name,
        method,
        url,
        query,
        headers,
        path_params,
        auth,
        body: request_body,
        proxy_url: None,
        history: Vec::new(),
        description,
        summary,
        operation_id,
        tags,
        deprecated,
    }
}

fn map_parameter(
    param: &Parameter,
    query: &mut Vec<domain::Header>,
    headers: &mut Vec<domain::Header>,
    path_params: &mut Vec<domain::Header>,
) {
    let data = param.parameter_data_ref();
    let value = extract_parameter_value(data);

    match param {
        Parameter::Query { .. } => {
            query.push(domain::Header {
                name: data.name.clone(),
                value,
                enabled: data.required,
            });
        }
        Parameter::Header { .. } => {
            headers.push(domain::Header {
                name: data.name.clone(),
                value,
                enabled: data.required,
            });
        }
        Parameter::Path { .. } => {
            path_params.push(domain::Header {
                name: data.name.clone(),
                value,
                enabled: true,
            });
        }
        Parameter::Cookie { .. } => {
            // Cookies are not currently modeled; skip
        }
    }
}

fn extract_parameter_value(data: &ParameterData) -> String {
    // Try example, schema.example, schema.default, or empty string
    if let Some(ref example) = data.example {
        if let Some(s) = example.as_str() {
            return s.to_string();
        }
        return example.to_string();
    }
    if let ParameterSchemaOrContent::Schema(schema) = &data.format {
        if let Some(val) = schema_example_value(schema) {
            if let Some(s) = val.as_str() {
                return s.to_string();
            }
            return val.to_string();
        }
    }
    String::new()
}

fn schema_example_value(schema: &ReferenceOr<Schema>) -> Option<serde_json::Value> {
    match schema {
        ReferenceOr::Item(s) => {
            // Schema is a struct with schema_data containing example/default
            if let Some(ref example) = s.schema_data.example {
                Some(example.clone())
            } else if let Some(ref default) = s.schema_data.default {
                Some(default.clone())
            } else {
                None
            }
        }
        ReferenceOr::Reference { .. } => None,
    }
}

fn resolve_auth(
    operation: &openapiv3::Operation,
    security_schemes: &SecuritySchemeMap,
) -> domain::Auth {
    let Some(security) = &operation.security else {
        return domain::Auth::None;
    };

    for req in security {
        for (scheme_name, _scopes) in req.iter() {
            if let Some(scheme) = security_schemes.get(scheme_name) {
                match scheme {
                    SecurityScheme::HTTP { scheme, .. } => {
                        if scheme.eq_ignore_ascii_case("basic") {
                            return domain::Auth::Basic {
                                username_ref: String::new(),
                                password_ref: String::new(),
                            };
                        }
                        if scheme.eq_ignore_ascii_case("bearer") {
                            return domain::Auth::Bearer {
                                token_ref: String::new(),
                            };
                        }
                    }
                    SecurityScheme::APIKey { name, location, .. } => {
                        let loc = match location {
                            APIKeyLocation::Query => domain::ApiKeyLocation::Query,
                            APIKeyLocation::Header => domain::ApiKeyLocation::Header,
                            APIKeyLocation::Cookie => domain::ApiKeyLocation::Cookie,
                        };
                        return domain::Auth::ApiKey {
                            name: name.clone(),
                            value_ref: name.clone(),
                            location: loc,
                        };
                    }
                    _ => {}
                }
            }
        }
    }

    domain::Auth::None
}

/// Generate a unique string id based on timestamp + randomness.
fn generate_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let ms = now.as_millis();
    let random: u64 = now.as_nanos() as u64;
    format!("{:016x}{:016x}", ms, random)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_openapi_spec() {
        let spec = r#"{
            "openapi": "3.0.0",
            "info": { "title": "Petstore", "version": "1.0" },
            "paths": {
                "/pets": {
                    "get": {
                        "operationId": "listPets",
                        "summary": "List all pets",
                        "parameters": [
                            { "name": "limit", "in": "query", "schema": { "type": "integer" } }
                        ],
                        "responses": { "200": { "description": "OK" } }
                    }
                }
            }
        }"#;
        let collection = parse_openapi_spec(spec).unwrap();
        assert_eq!(collection.name, "Petstore");
        assert_eq!(collection.items.len(), 1);
        match &collection.items[0] {
            domain::CollectionItem::Request(request) => {
                assert_eq!(request.operation_id.as_deref(), Some("listPets"));
                assert_eq!(request.summary.as_deref(), Some("List all pets"));
                assert_eq!(request.query.len(), 1);
                assert_eq!(request.query[0].name, "limit");
                assert!(request.url.contains("/pets"));
            }
            _ => panic!("expected request"),
        }
    }

    #[test]
    fn groups_operations_by_tag() {
        let spec = r#"{
            "openapi": "3.0.0",
            "info": { "title": "Tagged API", "version": "1.0" },
            "paths": {
                "/users": {
                    "get": {
                        "tags": ["Users"],
                        "summary": "List users",
                        "responses": { "200": { "description": "OK" } }
                    }
                },
                "/pets": {
                    "get": {
                        "tags": ["Pets"],
                        "summary": "List pets",
                        "responses": { "200": { "description": "OK" } }
                    }
                },
                "/health": {
                    "get": {
                        "summary": "Health check",
                        "responses": { "200": { "description": "OK" } }
                    }
                }
            }
        }"#;
        let collection = parse_openapi_spec(spec).unwrap();
        assert_eq!(collection.items.len(), 3);
        let folder_count = collection
            .items
            .iter()
            .filter(|item| matches!(item, domain::CollectionItem::Folder { .. }))
            .count();
        assert_eq!(folder_count, 2);
        let root_count = collection
            .items
            .iter()
            .filter(|item| matches!(item, domain::CollectionItem::Request(..)))
            .count();
        assert_eq!(root_count, 1);
    }

    #[test]
    fn parses_path_parameters() {
        let spec = r#"{
            "openapi": "3.0.0",
            "info": { "title": "Path Params", "version": "1.0" },
            "paths": {
                "/users/{userId}": {
                    "get": {
                        "operationId": "getUser",
                        "parameters": [
                            { "name": "userId", "in": "path", "required": true, "schema": { "type": "string" } }
                        ],
                        "responses": { "200": { "description": "OK" } }
                    }
                }
            }
        }"#;
        let collection = parse_openapi_spec(spec).unwrap();
        assert_eq!(collection.items.len(), 1);
        match &collection.items[0] {
            domain::CollectionItem::Request(request) => {
                assert_eq!(request.path_params.len(), 1);
                assert_eq!(request.path_params[0].name, "userId");
            }
            _ => panic!("expected request"),
        }
    }

    #[test]
    fn rejects_invalid_json() {
        let result = parse_openapi_spec("not valid json");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Import error"));
    }

    #[test]
    fn creates_body_from_schema_example() {
        let spec = r#"{
            "openapi": "3.0.0",
            "info": { "title": "Body API", "version": "1.0" },
            "paths": {
                "/users": {
                    "post": {
                        "operationId": "createUser",
                        "requestBody": {
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "example": {"name": "Alice", "age": 30}
                                    }
                                }
                            }
                        },
                        "responses": { "201": { "description": "Created" } }
                    }
                }
            }
        }"#;
        let collection = parse_openapi_spec(spec).unwrap();
        match &collection.items[0] {
            domain::CollectionItem::Request(request) => {
                assert_eq!(request.method, domain::Method::Post);
                assert!(matches!(request.body, domain::Body::Raw { .. }));
                if let domain::Body::Raw { value, .. } = &request.body {
                    assert!(value.contains("Alice"));
                }
            }
            _ => panic!("expected request"),
        }
    }

    #[test]
    fn parses_yaml_spec() {
        let spec = r#"
openapi: "3.0.0"
info:
  title: "YAML Petstore"
  version: "1.0"
paths:
  /pets:
    get:
      operationId: listPets
      summary: "List all pets"
      responses:
        "200":
          description: "OK"
    post:
      operationId: createPet
      summary: "Create a pet"
      requestBody:
        content:
          application/json:
            schema:
              example:
                name: "Rex"
                type: "dog"
      responses:
        "201":
          description: "Created"
        "#;
        let collection = parse_openapi_spec(spec).unwrap();
        assert_eq!(collection.name, "YAML Petstore");
        assert_eq!(collection.items.len(), 2);
        let ids: Vec<Option<&str>> = collection
            .items
            .iter()
            .map(|item| match item {
                domain::CollectionItem::Request(r) => r.operation_id.as_deref(),
                _ => None,
            })
            .collect();
        assert!(ids.contains(&Some("listPets")));
        assert!(ids.contains(&Some("createPet")));
    }
}
