use std::{fmt, io};

#[derive(Debug)]
pub enum ClientError {
    Io(io::Error),
    Json(serde_json::Error),
    Import(String),
    MissingRequest(String),
    UnsupportedUrl(String),
    InvalidResponse(String),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "I/O error: {error}"),
            Self::Json(error) => write!(f, "JSON error: {error}"),
            Self::Import(message) => write!(f, "Import error: {message}"),
            Self::MissingRequest(id) => write!(f, "Request not found: {id}"),
            Self::UnsupportedUrl(url) => write!(f, "Unsupported URL: {url}"),
            Self::InvalidResponse(message) => write!(f, "Invalid HTTP response: {message}"),
        }
    }
}

impl std::error::Error for ClientError {}

impl From<io::Error> for ClientError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for ClientError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

pub type Result<T> = std::result::Result<T, ClientError>;
