use std::time::Duration;

use super::{Header, Method, Request, ResponseRecord};

pub(crate) fn run_preview(request: &Request, duration: Duration) -> ResponseRecord {
    let body = format!(
        "{{\n  \"requestId\": \"{}\",\n  \"method\": \"{}\",\n  \"url\": \"{}\",\n  \"proxy\": {},\n  \"requestBodyBytes\": {}\n}}",
        request.id,
        request.method.as_str(),
        request.url,
        request
            .proxy_url
            .as_ref()
            .map(|proxy| format!("\"{proxy}\""))
            .unwrap_or_else(|| "null".to_string()),
        request.body.len()
    );

    ResponseRecord {
        status: if request.method == Method::Delete {
            204
        } else {
            200
        },
        status_text: if request.method == Method::Delete {
            "No Content".to_string()
        } else {
            "OK".to_string()
        },
        duration_ms: duration.as_millis() as u64,
        size_bytes: body.len(),
        headers: vec![Header::new("content-type", "application/json")],
        cookies: Vec::new(),
        body,
    }
}
