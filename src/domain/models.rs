use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Options,
}

impl Method {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Options => "OPTIONS",
        }
    }
}

impl From<&str> for Method {
    fn from(value: &str) -> Self {
        match value.to_ascii_uppercase().as_str() {
            "POST" => Self::Post,
            "PUT" => Self::Put,
            "PATCH" => Self::Patch,
            "DELETE" => Self::Delete,
            "OPTIONS" => Self::Options,
            _ => Self::Get,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Auth {
    None,
    Basic {
        username_ref: String,
        password_ref: String,
    },
    Bearer {
        token_ref: String,
    },
    ApiKey {
        name: String,
        value_ref: String,
        location: ApiKeyLocation,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiKeyLocation {
    Header,
    Query,
    Cookie,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    pub name: String,
    pub value: String,
    pub enabled: bool,
}

impl Header {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            enabled: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Body {
    Empty,
    Raw { content_type: String, value: String },
}

impl Body {
    pub fn raw_json(value: impl Into<String>) -> Self {
        Self::Raw {
            content_type: "application/json".to_string(),
            value: value.into(),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Raw { value, .. } => value.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub id: String,
    pub name: String,
    pub method: Method,
    pub url: String,
    pub query: Vec<Header>,
    pub headers: Vec<Header>,
    pub auth: Auth,
    pub body: Body,
    pub proxy_url: Option<String>,
    pub history: Vec<ResponseRecord>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseTiming {
    pub dns_lookup_ms: u64,
    pub connect_ms: u64,
    pub tls_handshake_ms: u64,
    pub time_to_first_byte_ms: u64,
    pub transfer_ms: u64,
}

impl ResponseTiming {
    pub fn total_ms(&self) -> u64 {
        self.dns_lookup_ms
            + self.connect_ms
            + self.tls_handshake_ms
            + self.time_to_first_byte_ms
            + self.transfer_ms
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseRecord {
    pub status: u16,
    pub status_text: String,
    pub duration_ms: u64,
    pub size_bytes: usize,
    pub headers: Vec<Header>,
    pub cookies: Vec<Header>,
    pub body: String,
    #[serde(default)]
    pub timing: Option<ResponseTiming>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollectionItem {
    Folder {
        id: String,
        name: String,
        items: Vec<CollectionItem>,
    },
    Request(Request),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Environment {
    pub name: String,
    pub variables: Vec<Header>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub active_environment: String,
    pub environments: Vec<Environment>,
    pub items: Vec<CollectionItem>,
    #[serde(default)]
    pub expanded_folders: Vec<String>,
}
impl Request {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        method: Method,
        url: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            method,
            url: url.into(),
            query: Vec::new(),
            headers: Vec::new(),
            auth: Auth::None,
            body: Body::Empty,
            proxy_url: None,
            history: Vec::new(),
        }
    }
}
