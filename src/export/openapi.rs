use indexmap::IndexMap;
use openapiv3::*;

use crate::domain;

/// Errors that can occur during OpenAPI export.
#[derive(Debug)]
pub enum ExportError {
    /// A request has an unsupported method or malformed URL.
    InvalidRequest(String),
    /// Serialization of the OpenAPI spec to JSON failed.
    SerializationError(serde_json::Error),
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportError::InvalidRequest(msg) => write!(f, "Invalid request: {}", msg),
            ExportError::SerializationError(e) => write!(f, "Serialization error: {}", e),
        }
    }
}

impl std::error::Error for ExportError {}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Convert a [`domain::Workspace`] into an OpenAPI 3.0.3 JSON string.
///
/// Iterates every collection and folder in the workspace, groups requests by
/// their path + method, and emits an OpenAPI spec with:
///
/// - Paths and operations derived from each request's URL and method
/// - Query, header, and path parameters
/// - Request body (with a best-effort schema from the raw body)
/// - Security schemes for Basic, Bearer, and ApiKey auth
/// - Tags from the request's `tags` field (folders are ignored; tag metadata
///   is preserved on each operation)
/// - Server entries deduced from request URLs
pub fn workspace_to_openapi_spec(workspace: &domain::Workspace) -> Result<String, ExportError> {
    let mut spec = OpenAPI {
        openapi: "3.0.3".into(),
        info: Info {
            title: workspace.name.clone(),
            version: "1.0.0".into(),
            ..Default::default()
        },
        paths: Paths::default(),
        components: Some(Components::default()),
        security: None,
        ..Default::default()
    };

    // Collect all unique security scheme names used across operations
    let mut scheme_names: Vec<String> = Vec::new();

    // Collect all unique tag names for the global tag list
    let mut tag_names: Vec<String> = Vec::new();

    // Collect server URLs
    let mut server_urls: Vec<String> = Vec::new();

    // Flatten all collections into a list of (request, folder_name) tuples
    let mut requests: Vec<(&domain::Request, &str)> = Vec::new();
    for collection in &workspace.items {
        collect_requests(&collection.items, "", &mut requests);
    }

    for (req, _folder) in &requests {
        // --- Extract path and server from URL ---
        let processed_url = normalize_url(&req.url);
        let (server_opt, path) = extract_path(&processed_url);
        if let Some(ref srv) = server_opt {
            if !server_urls.contains(srv) {
                server_urls.push(srv.clone());
            }
        }

        // --- Build operation ---
        let operation = request_to_operation(req, &mut spec, &mut scheme_names, &mut tag_names)?;

        // --- Insert into paths ---
        let entry = spec
            .paths
            .paths
            .entry(path.clone())
            .or_insert_with(|| ReferenceOr::Item(PathItem::default()));
        if let ReferenceOr::Item(path_item) = entry {
            set_path_method(path_item, &req.method, operation)?;
        }
    }

    // --- Servers ---
    spec.servers = server_urls
        .into_iter()
        .map(|url| Server {
            url,
            ..Default::default()
        })
        .collect();

    // --- Global security (union of all schemes used) ---
    if !scheme_names.is_empty() {
        let mut sec_req = SecurityRequirement::new();
        for name in &scheme_names {
            sec_req.insert(name.clone(), vec![]);
        }
        spec.security = Some(vec![sec_req]);
    }

    // --- Tags ---
    tag_names.sort();
    tag_names.dedup();
    spec.tags = tag_names
        .into_iter()
        .map(|name| Tag {
            name,
            ..Default::default()
        })
        .collect();

    serde_json::to_string_pretty(&spec).map_err(ExportError::SerializationError)
}

// ---------------------------------------------------------------------------
// Request collection (flatten tree)
// ---------------------------------------------------------------------------

fn collect_requests<'a>(
    items: &'a [domain::CollectionItem],
    _folder_name: &'a str,
    out: &mut Vec<(&'a domain::Request, &'a str)>,
) {
    for item in items {
        match item {
            domain::CollectionItem::Request(req) => out.push((req, _folder_name)),
            domain::CollectionItem::Folder {
                name,
                items: children,
                ..
            } => collect_requests(children, name, out),
        }
    }
}

// ---------------------------------------------------------------------------
// URL / path helpers
// ---------------------------------------------------------------------------

/// Replace `:param` patterns with `{param}` for OpenAPI path templating.
fn normalize_url(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b':' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_alphanumeric() {
            out.push('{');
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                out.push(bytes[i] as char);
                i += 1;
            }
            out.push('}');
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

/// Extract the path component from a URL, and optionally the base server URL.
///
/// Uses string-based extraction rather than `url::Url::parse` to avoid
/// percent-encoding curly braces (`{` / `}`) used by OpenAPI path templating.
fn extract_path(url_str: &str) -> (Option<String>, String) {
    if let Some(pos) = url_str.find("://") {
        let after_scheme = &url_str[pos + 3..];
        // Split on the first '/' to separate host (with optional port) from path
        if let Some(slash_pos) = after_scheme.find('/') {
            let host = &after_scheme[..slash_pos];
            let path = after_scheme[slash_pos..].to_string();
            (Some(format!("{}://{}", &url_str[..pos], host)), path)
        } else {
            // No path after host — use root
            (Some(url_str.to_string()), "/".to_string())
        }
    } else {
        // No protocol — treat the whole string as a path
        if url_str.starts_with('/') {
            (None, url_str.to_string())
        } else {
            (None, format!("/{}", url_str))
        }
    }
}

// ---------------------------------------------------------------------------
// Request → Operation conversion
// ---------------------------------------------------------------------------

fn request_to_operation(
    req: &domain::Request,
    spec: &mut OpenAPI,
    scheme_names: &mut Vec<String>,
    tag_names: &mut Vec<String>,
) -> Result<Operation, ExportError> {
    let mut parameters: Vec<ReferenceOr<Parameter>> = Vec::new();

    // --- Path parameters ---
    for hp in &req.path_params {
        if hp.name.is_empty() {
            continue;
        }
        parameters.push(ReferenceOr::Item(Parameter::Path {
            parameter_data: header_to_parameter_data(hp, true),
            style: PathStyle::Simple,
        }));
    }

    // --- Query parameters ---
    for hp in &req.query {
        if hp.name.is_empty() {
            continue;
        }
        parameters.push(ReferenceOr::Item(Parameter::Query {
            parameter_data: header_to_parameter_data(hp, false),
            allow_reserved: false,
            style: QueryStyle::Form,
            allow_empty_value: None,
        }));
    }

    // --- Header parameters ---
    // Skip Content-Type, Accept, Authorization as they have dedicated fields
    for hp in &req.headers {
        let lower = hp.name.to_ascii_lowercase();
        if hp.name.is_empty()
            || lower == "content-type"
            || lower == "accept"
            || lower == "authorization"
        {
            continue;
        }
        parameters.push(ReferenceOr::Item(Parameter::Header {
            parameter_data: header_to_parameter_data(hp, false),
            style: HeaderStyle::Simple,
        }));
    }

    // --- Request body ---
    let request_body = body_to_request_body(req);

    // --- Security ---
    let security = auth_to_security(req, spec, scheme_names);

    // --- Tags ---
    let tags: Vec<String> = req.tags.clone();
    for t in &tags {
        if !tag_names.contains(t) {
            tag_names.push(t.clone());
        }
    }

    // Default responses (200 OK placeholder)
    let mut responses_map = IndexMap::new();
    responses_map.insert(
        StatusCode::Code(200),
        ReferenceOr::Item(Response {
            description: "Success".into(),
            ..Default::default()
        }),
    );

    Ok(Operation {
        tags,
        summary: req.summary.clone(),
        description: req.description.clone(),
        operation_id: req.operation_id.clone(),
        parameters,
        request_body,
        responses: Responses {
            responses: responses_map,
            ..Default::default()
        },
        deprecated: req.deprecated,
        security,
        ..Default::default()
    })
}

// ---------------------------------------------------------------------------
// Parameter helpers
// ---------------------------------------------------------------------------

fn header_to_parameter_data(hp: &domain::Header, required: bool) -> ParameterData {
    ParameterData {
        name: hp.name.clone(),
        description: None,
        required,
        deprecated: None,
        format: ParameterSchemaOrContent::Schema(ReferenceOr::Item(Schema {
            schema_data: SchemaData::default(),
            schema_kind: SchemaKind::Type(Type::String(StringType::default())),
        })),
        example: None,
        examples: IndexMap::new(),
        explode: None,
        extensions: IndexMap::new(),
    }
}

// ---------------------------------------------------------------------------
// Body helpers
// ---------------------------------------------------------------------------

fn body_to_request_body(req: &domain::Request) -> Option<ReferenceOr<RequestBody>> {
    match &req.body {
        domain::Body::Empty => None,
        domain::Body::Raw {
            content_type,
            value,
        } => {
            let content_type = if content_type.is_empty() {
                "application/octet-stream"
            } else {
                content_type.as_str()
            };

            let schema = json_value_to_schema(value);
            let mut content = IndexMap::new();
            content.insert(
                content_type.to_string(),
                MediaType {
                    schema: Some(ReferenceOr::Item(schema)),
                    example: if value.is_empty() {
                        None
                    } else {
                        Some(serde_json::Value::String(value.clone()))
                    },
                    ..Default::default()
                },
            );

            Some(ReferenceOr::Item(RequestBody {
                description: None,
                content,
                required: true,
                ..Default::default()
            }))
        }
    }
}

/// Try to parse a string as JSON and build a matching schema.  Falls back to
/// a plain string schema when the value is not valid JSON.
fn json_value_to_schema(raw: &str) -> Schema {
    if raw.trim().is_empty() {
        return Schema {
            schema_data: SchemaData::default(),
            schema_kind: SchemaKind::Type(Type::String(StringType::default())),
        };
    }

    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(serde_json::Value::Object(map)) => {
            let mut properties = IndexMap::new();
            for (key, val) in &map {
                let prop_schema = json_kind_to_schema(val);
                properties.insert(key.clone(), ReferenceOr::Item(Box::new(prop_schema)));
            }
            Schema {
                schema_data: SchemaData::default(),
                schema_kind: SchemaKind::Type(Type::Object(ObjectType {
                    properties,
                    required: map.keys().map(|k| k.clone()).collect(),
                    ..Default::default()
                })),
            }
        }
        Ok(serde_json::Value::Array(items)) => {
            let item_schema = items.first().map(|v| json_kind_to_schema(v));
            Schema {
                schema_data: SchemaData::default(),
                schema_kind: SchemaKind::Type(Type::Array(ArrayType {
                    items: item_schema.map(|s| ReferenceOr::Item(Box::new(s))),
                    min_items: None,
                    max_items: None,
                    unique_items: false,
                })),
            }
        }
        Ok(serde_json::Value::String(_)) => Schema {
            schema_data: SchemaData::default(),
            schema_kind: SchemaKind::Type(Type::String(StringType::default())),
        },
        Ok(serde_json::Value::Number(_)) => Schema {
            schema_data: SchemaData::default(),
            schema_kind: SchemaKind::Type(Type::Number(NumberType::default())),
        },
        Ok(serde_json::Value::Bool(_)) => Schema {
            schema_data: SchemaData::default(),
            schema_kind: SchemaKind::Type(Type::Boolean(BooleanType::default())),
        },
        Ok(serde_json::Value::Null) => Schema {
            schema_data: SchemaData {
                nullable: true,
                ..Default::default()
            },
            schema_kind: SchemaKind::Type(Type::String(StringType::default())),
        },
        Err(_) => {
            // Not valid JSON – treat as plain string
            Schema {
                schema_data: SchemaData::default(),
                schema_kind: SchemaKind::Type(Type::String(StringType::default())),
            }
        }
    }
}

fn json_kind_to_schema(val: &serde_json::Value) -> Schema {
    match val {
        serde_json::Value::Object(map) => {
            let mut properties = IndexMap::new();
            for (key, v) in map {
                properties.insert(
                    key.clone(),
                    ReferenceOr::Item(Box::new(json_kind_to_schema(v))),
                );
            }
            Schema {
                schema_data: SchemaData::default(),
                schema_kind: SchemaKind::Type(Type::Object(ObjectType {
                    properties,
                    ..Default::default()
                })),
            }
        }
        serde_json::Value::Array(items) => {
            let item_schema = items.first().map(|v| json_kind_to_schema(v));
            Schema {
                schema_data: SchemaData::default(),
                schema_kind: SchemaKind::Type(Type::Array(ArrayType {
                    items: item_schema.map(|s| ReferenceOr::Item(Box::new(s))),
                    min_items: None,
                    max_items: None,
                    unique_items: false,
                })),
            }
        }
        serde_json::Value::String(_) => Schema {
            schema_data: SchemaData::default(),
            schema_kind: SchemaKind::Type(Type::String(StringType::default())),
        },
        serde_json::Value::Number(_) => Schema {
            schema_data: SchemaData::default(),
            schema_kind: SchemaKind::Type(Type::Number(NumberType::default())),
        },
        serde_json::Value::Bool(_) => Schema {
            schema_data: SchemaData::default(),
            schema_kind: SchemaKind::Type(Type::Boolean(BooleanType::default())),
        },
        serde_json::Value::Null => Schema {
            schema_data: SchemaData {
                nullable: true,
                ..Default::default()
            },
            schema_kind: SchemaKind::Type(Type::String(StringType::default())),
        },
    }
}

// ---------------------------------------------------------------------------
// Auth helpers
// ---------------------------------------------------------------------------

fn auth_to_security(
    req: &domain::Request,
    spec: &mut OpenAPI,
    scheme_names: &mut Vec<String>,
) -> Option<Vec<SecurityRequirement>> {
    match &req.auth {
        domain::Auth::None => None,
        domain::Auth::Basic { .. } => {
            let name = "basicAuth";
            if !scheme_names.contains(&name.to_string()) {
                scheme_names.push(name.to_string());
                let components = spec.components.as_mut().unwrap();
                components.security_schemes.insert(
                    name.to_string(),
                    ReferenceOr::Item(SecurityScheme::HTTP {
                        scheme: "basic".into(),
                        bearer_format: None,
                        description: None,
                        extensions: IndexMap::new(),
                    }),
                );
            }
            let mut req_map = SecurityRequirement::new();
            req_map.insert(name.to_string(), vec![]);
            Some(vec![req_map])
        }
        domain::Auth::Bearer { .. } => {
            let name = "bearerAuth";
            if !scheme_names.contains(&name.to_string()) {
                scheme_names.push(name.to_string());
                let components = spec.components.as_mut().unwrap();
                components.security_schemes.insert(
                    name.to_string(),
                    ReferenceOr::Item(SecurityScheme::HTTP {
                        scheme: "bearer".into(),
                        bearer_format: Some("JWT".into()),
                        description: None,
                        extensions: IndexMap::new(),
                    }),
                );
            }
            let mut req_map = SecurityRequirement::new();
            req_map.insert(name.to_string(), vec![]);
            Some(vec![req_map])
        }
        domain::Auth::ApiKey { name, location, .. } => {
            let scheme_name = "apiKeyAuth";
            if !scheme_names.contains(&scheme_name.to_string()) {
                scheme_names.push(scheme_name.to_string());
                let api_location = match location {
                    domain::ApiKeyLocation::Header => APIKeyLocation::Header,
                    domain::ApiKeyLocation::Query => APIKeyLocation::Query,
                    domain::ApiKeyLocation::Cookie => APIKeyLocation::Cookie,
                };
                let components = spec.components.as_mut().unwrap();
                components.security_schemes.insert(
                    scheme_name.to_string(),
                    ReferenceOr::Item(SecurityScheme::APIKey {
                        name: name.clone(),
                        location: api_location,
                        description: None,
                        extensions: IndexMap::new(),
                    }),
                );
            }
            let mut req_map = SecurityRequirement::new();
            req_map.insert(scheme_name.to_string(), vec![]);
            Some(vec![req_map])
        }
    }
}

// ---------------------------------------------------------------------------
// Method → PathItem setter
// ---------------------------------------------------------------------------

fn set_path_method(
    path_item: &mut PathItem,
    method: &domain::Method,
    operation: Operation,
) -> Result<(), ExportError> {
    match method {
        domain::Method::Get => path_item.get = Some(operation),
        domain::Method::Post => path_item.post = Some(operation),
        domain::Method::Put => path_item.put = Some(operation),
        domain::Method::Patch => path_item.patch = Some(operation),
        domain::Method::Delete => path_item.delete = Some(operation),
        domain::Method::Options => path_item.options = Some(operation),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain;

    // -----------------------------------------------------------------------
    // Helper: build a minimal workspace with one request
    // -----------------------------------------------------------------------

    fn single_request_workspace(
        method: domain::Method,
        url: &str,
        adjust: impl FnOnce(&mut domain::Request),
    ) -> domain::Workspace {
        let mut req = domain::Request::new("req-1", "Test Request", method, url);
        adjust(&mut req);
        domain::Workspace {
            id: "ws-1".into(),
            name: "Test Workspace".into(),
            active_environment: String::new(),
            environments: vec![],
            items: vec![domain::Collection {
                id: "col-1".into(),
                name: "Test Collection".into(),
                items: vec![domain::CollectionItem::Request(req)],
            }],
            expanded_folders: vec![],
            expanded_collections: vec![],
        }
    }

    // -----------------------------------------------------------------------
    // Basic export smoke test
    // -----------------------------------------------------------------------

    #[test]
    fn exports_minimal_workspace() {
        let ws = single_request_workspace(
            domain::Method::Get,
            "https://api.example.com/v1/users",
            |_| {},
        );
        let json = workspace_to_openapi_spec(&ws).unwrap();
        let parsed: openapiv3::OpenAPI = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.openapi, "3.0.3");
        assert_eq!(parsed.info.title, "Test Workspace");
        assert_eq!(parsed.info.version, "1.0.0");

        // One path: /v1/users with GET
        let path = parsed.paths.paths.get("/v1/users").unwrap();
        let ReferenceOr::Item(item) = path else {
            panic!("expected inline path item");
        };
        assert!(item.get.is_some(), "GET operation expected");
    }

    // -----------------------------------------------------------------------
    // Multiple methods on the same path
    // -----------------------------------------------------------------------

    #[test]
    fn exports_multiple_methods_on_same_path() {
        let mut ws =
            single_request_workspace(domain::Method::Get, "https://api.example.com/items", |_| {});
        // Add a POST to the same path via a second collection item
        let post_req = domain::Request::new(
            "req-2",
            "Create Item",
            domain::Method::Post,
            "https://api.example.com/items",
        );
        ws.items[0]
            .items
            .push(domain::CollectionItem::Request(post_req));

        let json = workspace_to_openapi_spec(&ws).unwrap();
        let parsed: openapiv3::OpenAPI = serde_json::from_str(&json).unwrap();

        let path = parsed.paths.paths.get("/items").unwrap();
        let ReferenceOr::Item(item) = path else {
            panic!("inline")
        };
        assert!(item.get.is_some(), "GET missing");
        assert!(item.post.is_some(), "POST missing");
    }

    // -----------------------------------------------------------------------
    // Path parameters
    // -----------------------------------------------------------------------

    #[test]
    fn exports_path_parameters() {
        let ws = single_request_workspace(
            domain::Method::Get,
            "https://api.example.com/users/:userId",
            |req| {
                req.path_params.push(domain::Header::new("userId", "42"));
            },
        );
        let json = workspace_to_openapi_spec(&ws).unwrap();
        let parsed: openapiv3::OpenAPI = serde_json::from_str(&json).unwrap();

        let path = parsed.paths.paths.get("/users/{userId}").unwrap();
        let ReferenceOr::Item(item) = path else {
            panic!("inline")
        };
        let op = item.get.as_ref().unwrap();
        let param = param_by_name(&op.parameters, "userId").unwrap();
        assert!(matches!(param, Parameter::Path { .. }));
    }

    // -----------------------------------------------------------------------
    // Query parameters
    // -----------------------------------------------------------------------

    #[test]
    fn exports_query_parameters() {
        let ws = single_request_workspace(
            domain::Method::Get,
            "https://api.example.com/search",
            |req| {
                req.query.push(domain::Header::new("q", "hello"));
                req.query.push(domain::Header::new("page", "1"));
            },
        );
        let json = workspace_to_openapi_spec(&ws).unwrap();
        let parsed: openapiv3::OpenAPI = serde_json::from_str(&json).unwrap();

        let path = parsed.paths.paths.get("/search").unwrap();
        let ReferenceOr::Item(item) = path else {
            panic!("inline")
        };
        let op = item.get.as_ref().unwrap();
        assert!(param_by_name(&op.parameters, "q").is_some());
        assert!(param_by_name(&op.parameters, "page").is_some());
    }

    // -----------------------------------------------------------------------
    // Request body
    // -----------------------------------------------------------------------

    #[test]
    fn exports_json_request_body() {
        let ws = single_request_workspace(
            domain::Method::Post,
            "https://api.example.com/users",
            |req| {
                req.body = domain::Body::Raw {
                    content_type: "application/json".into(),
                    value: r#"{"name": "Alice", "email": "alice@example.com"}"#.into(),
                };
            },
        );
        let json = workspace_to_openapi_spec(&ws).unwrap();
        let parsed: openapiv3::OpenAPI = serde_json::from_str(&json).unwrap();

        let path = parsed.paths.paths.get("/users").unwrap();
        let ReferenceOr::Item(item) = path else {
            panic!("inline")
        };
        let op = item.post.as_ref().unwrap();
        let rb = op.request_body.as_ref().unwrap();
        let ReferenceOr::Item(body) = rb else {
            panic!("inline")
        };
        assert!(body.content.contains_key("application/json"));
    }

    // -----------------------------------------------------------------------
    // Auth – Basic
    // -----------------------------------------------------------------------

    #[test]
    fn exports_basic_auth() {
        let ws = single_request_workspace(
            domain::Method::Get,
            "https://api.example.com/secure",
            |req| {
                req.auth = domain::Auth::Basic {
                    username_key: "admin".into(),
                    password_key: "secret".into(),
                };
            },
        );
        let json = workspace_to_openapi_spec(&ws).unwrap();
        let parsed: openapiv3::OpenAPI = serde_json::from_str(&json).unwrap();

        let components = parsed.components.as_ref().unwrap();
        let scheme = components.security_schemes.get("basicAuth").unwrap();
        assert!(matches!(
            scheme,
            ReferenceOr::Item(SecurityScheme::HTTP { .. })
        ));

        // Operation should have security
        let path = parsed.paths.paths.get("/secure").unwrap();
        let ReferenceOr::Item(item) = path else {
            panic!("inline")
        };
        let op = item.get.as_ref().unwrap();
        assert!(op.security.is_some());
    }

    // -----------------------------------------------------------------------
    // Auth – Bearer
    // -----------------------------------------------------------------------

    #[test]
    fn exports_bearer_auth() {
        let ws = single_request_workspace(
            domain::Method::Get,
            "https://api.example.com/token",
            |req| {
                req.auth = domain::Auth::Bearer {
                    token_key: "mytoken".into(),
                };
            },
        );
        let json = workspace_to_openapi_spec(&ws).unwrap();
        let parsed: openapiv3::OpenAPI = serde_json::from_str(&json).unwrap();

        let components = parsed.components.as_ref().unwrap();
        let bearer = components.security_schemes.get("bearerAuth").unwrap();
        match bearer {
            ReferenceOr::Item(SecurityScheme::HTTP {
                scheme,
                bearer_format,
                ..
            }) => {
                assert_eq!(scheme, "bearer");
                assert_eq!(bearer_format.as_deref(), Some("JWT"));
            }
            _ => panic!("expected bearer HTTP scheme"),
        }
    }

    // -----------------------------------------------------------------------
    // Auth – ApiKey
    // -----------------------------------------------------------------------

    #[test]
    fn exports_apikey_auth() {
        let ws = single_request_workspace(
            domain::Method::Get,
            "https://api.example.com/keyed",
            |req| {
                req.auth = domain::Auth::ApiKey {
                    name: "X-API-Key".into(),
                    value_key: "abc123".into(),
                    location: domain::ApiKeyLocation::Header,
                };
            },
        );
        let json = workspace_to_openapi_spec(&ws).unwrap();
        let parsed: openapiv3::OpenAPI = serde_json::from_str(&json).unwrap();

        let components = parsed.components.as_ref().unwrap();
        let apikey = components.security_schemes.get("apiKeyAuth").unwrap();
        match apikey {
            ReferenceOr::Item(SecurityScheme::APIKey { name, location, .. }) => {
                assert_eq!(name, "X-API-Key");
                assert_eq!(*location, APIKeyLocation::Header);
            }
            _ => panic!("expected APIKey scheme"),
        }
    }

    // -----------------------------------------------------------------------
    // Metadata fields (operation_id, summary, description, tags, deprecated)
    // -----------------------------------------------------------------------

    #[test]
    fn exports_operation_metadata() {
        let ws = single_request_workspace(
            domain::Method::Get,
            "https://api.example.com/users",
            |req| {
                req.operation_id = Some("getUsers".into());
                req.summary = Some("List all users".into());
                req.description = Some("Returns a paginated list of users".into());
                req.tags = vec!["Users".into(), "Admin".into()];
                req.deprecated = true;
            },
        );
        let json = workspace_to_openapi_spec(&ws).unwrap();
        let parsed: openapiv3::OpenAPI = serde_json::from_str(&json).unwrap();

        let path = parsed.paths.paths.get("/users").unwrap();
        let ReferenceOr::Item(item) = path else {
            panic!("inline")
        };
        let op = item.get.as_ref().unwrap();

        assert_eq!(op.operation_id.as_deref(), Some("getUsers"));
        assert_eq!(op.summary.as_deref(), Some("List all users"));
        assert_eq!(
            op.description.as_deref(),
            Some("Returns a paginated list of users")
        );
        assert!(op.tags.contains(&"Users".into()));
        assert!(op.tags.contains(&"Admin".into()));
        assert!(op.deprecated);

        // Tag definitions should appear at the spec level
        let tag_names: Vec<&str> = parsed.tags.iter().map(|t| t.name.as_str()).collect();
        assert!(tag_names.contains(&"Admin"));
        assert!(tag_names.contains(&"Users"));
    }

    // -----------------------------------------------------------------------
    // URL path parameter colon → curly-brace conversion
    // -----------------------------------------------------------------------

    #[test]
    fn exports_colon_to_curly_path() {
        let ws = single_request_workspace(
            domain::Method::Get,
            "https://api.example.com/users/:userId/posts/:postId",
            |_| {},
        );
        let json = workspace_to_openapi_spec(&ws).unwrap();
        let parsed: openapiv3::OpenAPI = serde_json::from_str(&json).unwrap();

        assert!(
            parsed
                .paths
                .paths
                .contains_key("/users/{userId}/posts/{postId}"),
            "expected colon notation converted to curly braces"
        );
    }

    // -----------------------------------------------------------------------
    // Server URL extraction
    // -----------------------------------------------------------------------

    #[test]
    fn exports_server_from_url() {
        let ws = single_request_workspace(
            domain::Method::Get,
            "https://api.staging.example.com/v2/items",
            |_| {},
        );
        let json = workspace_to_openapi_spec(&ws).unwrap();
        let parsed: openapiv3::OpenAPI = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.servers.len(), 1);
        assert_eq!(parsed.servers[0].url, "https://api.staging.example.com");
    }

    // -----------------------------------------------------------------------
    // Multiple servers from different URLs
    // -----------------------------------------------------------------------

    #[test]
    fn exports_multiple_servers() {
        let mut ws = single_request_workspace(
            domain::Method::Get,
            "https://api.example.com/v1/items",
            |_| {},
        );
        let req2 = domain::Request::new(
            "req-2",
            "Other endpoint",
            domain::Method::Get,
            "https://api.example.com/v2/items",
        );
        ws.items[0]
            .items
            .push(domain::CollectionItem::Request(req2));

        let json = workspace_to_openapi_spec(&ws).unwrap();
        let parsed: openapiv3::OpenAPI = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.servers.len(), 1); // Same host
        assert_eq!(parsed.servers[0].url, "https://api.example.com");
    }

    // -----------------------------------------------------------------------
    // No body → no requestBody in output
    // -----------------------------------------------------------------------

    #[test]
    fn exports_empty_body_omits_request_body() {
        let ws = single_request_workspace(
            domain::Method::Post,
            "https://api.example.com/submit",
            |req| {
                req.body = domain::Body::Empty;
            },
        );
        let json = workspace_to_openapi_spec(&ws).unwrap();
        let parsed: openapiv3::OpenAPI = serde_json::from_str(&json).unwrap();
        let path = parsed.paths.paths.get("/submit").unwrap();
        let ReferenceOr::Item(item) = path else {
            panic!("inline")
        };
        let op = item.post.as_ref().unwrap();
        assert!(op.request_body.is_none());
    }

    // -----------------------------------------------------------------------
    // Folders are traversed and requests inside them are collected
    // -----------------------------------------------------------------------

    #[test]
    fn exports_requests_inside_folders() {
        let req_in_folder = domain::Request::new(
            "req-f1",
            "Folder GET",
            domain::Method::Get,
            "https://api.example.com/folder-item",
        );
        let ws = domain::Workspace {
            id: "ws-1".into(),
            name: "Folder Test".into(),
            active_environment: String::new(),
            environments: vec![],
            items: vec![domain::Collection {
                id: "col-1".into(),
                name: "Collection".into(),
                items: vec![domain::CollectionItem::Folder {
                    id: "f-1".into(),
                    name: "SubFolder".into(),
                    items: vec![domain::CollectionItem::Request(req_in_folder)],
                }],
            }],
            expanded_folders: vec![],
            expanded_collections: vec![],
        };

        let json = workspace_to_openapi_spec(&ws).unwrap();
        let parsed: openapiv3::OpenAPI = serde_json::from_str(&json).unwrap();
        assert!(
            parsed.paths.paths.contains_key("/folder-item"),
            "requests inside folders must be exported"
        );
    }

    // -----------------------------------------------------------------------
    // Round-trip: export → import produces same number of requests
    // -----------------------------------------------------------------------

    #[test]
    fn round_trip_export_import() {
        let mut req1 = domain::Request::new(
            "r1",
            "List Users",
            domain::Method::Get,
            "https://api.example.com/users",
        );
        req1.query.push(domain::Header::new("page", "1"));
        req1.tags = vec!["Users".into()];

        let mut req2 = domain::Request::new(
            "r2",
            "Create User",
            domain::Method::Post,
            "https://api.example.com/users",
        );
        req2.body = domain::Body::raw_json(r#"{"name":"Alice"}"#);
        req2.tags = vec!["Users".into()];

        let mut req3 = domain::Request::new(
            "r3",
            "Get User",
            domain::Method::Get,
            "https://api.example.com/users/:id",
        );
        req3.path_params.push(domain::Header::new("id", "42"));
        req3.tags = vec!["Users".into()];

        let ws = domain::Workspace {
            id: "ws-rt".into(),
            name: "Round Trip".into(),
            active_environment: String::new(),
            environments: vec![],
            items: vec![domain::Collection {
                id: "col-rt".into(),
                name: "API".into(),
                items: vec![
                    domain::CollectionItem::Request(req1),
                    domain::CollectionItem::Request(req2),
                    domain::CollectionItem::Request(req3),
                ],
            }],
            expanded_folders: vec![],
            expanded_collections: vec![],
        };

        let json = workspace_to_openapi_spec(&ws).unwrap();

        // Re-import
        let imported = crate::import::openapi::parse_openapi_spec(&json).unwrap();

        // Count request items recursively (they may be inside folders)
        fn count_requests(items: &[domain::CollectionItem]) -> usize {
            items
                .iter()
                .map(|item| match item {
                    domain::CollectionItem::Request(_) => 1,
                    domain::CollectionItem::Folder {
                        items: children, ..
                    } => count_requests(children),
                })
                .sum()
        }
        let request_count = count_requests(&imported.items);
        assert_eq!(request_count, 3, "round-trip should yield 3 requests");
    }

    // -----------------------------------------------------------------------
    // Invalid method → Error
    // -----------------------------------------------------------------------

    // (Not applicable: domain::Method is an enum of only valid methods)

    #[test]
    fn exports_single_collection_from_multi_collection_workspace() {
        // Simulate "export selected collection" — workspace has two collections
        // but we export a workspace containing only one of them.
        fn make_req(id: &str, method: domain::Method, url: &str) -> domain::Request {
            domain::Request::new(id, format!("Req {id}"), method, url)
        }

        let collection_a = domain::Collection {
            id: "col-a".into(),
            name: "Collection A".into(),
            items: vec![
                domain::CollectionItem::Request(make_req(
                    "1",
                    domain::Method::Get,
                    "https://api.example.com/users",
                )),
                domain::CollectionItem::Request(make_req(
                    "2",
                    domain::Method::Post,
                    "https://api.example.com/users",
                )),
            ],
        };
        let collection_b = domain::Collection {
            id: "col-b".into(),
            name: "Collection B".into(),
            items: vec![domain::CollectionItem::Request(make_req(
                "3",
                domain::Method::Delete,
                "https://api.example.com/items/1",
            ))],
        };

        let full_ws = domain::Workspace {
            id: "ws-1".into(),
            name: "Full".into(),
            active_environment: String::new(),
            environments: vec![],
            items: vec![collection_a.clone(), collection_b],
            expanded_folders: vec![],
            expanded_collections: vec![],
        };

        // "Selected collection" export: workspace with only collection A
        let selected_ws = domain::Workspace {
            id: "ws-1".into(),
            name: "Selected".into(),
            active_environment: String::new(),
            environments: vec![],
            items: vec![collection_a],
            expanded_folders: vec![],
            expanded_collections: vec![],
        };

        let full_json = workspace_to_openapi_spec(&full_ws).unwrap();
        let selected_json = workspace_to_openapi_spec(&selected_ws).unwrap();

        let full_spec: OpenAPI = serde_json::from_str(&full_json).unwrap();
        let selected_spec: OpenAPI = serde_json::from_str(&selected_json).unwrap();

        // Full spec has both paths
        assert!(full_spec.paths.paths.contains_key("/users"));
        assert!(full_spec.paths.paths.contains_key("/items/1"));

        // Selected spec has only the /users path from collection A
        assert!(selected_spec.paths.paths.contains_key("/users"));
        assert!(!selected_spec.paths.paths.contains_key("/items/1"));
    }

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    /// Find an operation parameter by name (ignoring reference status).
    fn param_by_name<'a>(
        params: &'a [ReferenceOr<Parameter>],
        name: &str,
    ) -> Option<&'a Parameter> {
        params
            .iter()
            .filter_map(|r| match r {
                ReferenceOr::Item(p) => {
                    if p.parameter_data_ref().name == name {
                        Some(p)
                    } else {
                        None
                    }
                }
                ReferenceOr::Reference { .. } => None,
            })
            .next()
    }
}
