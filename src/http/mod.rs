use std::time::Duration;

use crate::domain::{Body, ClientError, Header, Request, ResponseRecord, Result};

pub fn send_http_request(request: &Request) -> Result<ResponseRecord> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::limited(10))
        .cookie_store(true)
        .build()
        .map_err(|error| ClientError::InvalidResponse(error.to_string()))?;
    let method = reqwest::Method::from_bytes(request.method.as_str().as_bytes())
        .map_err(|error| ClientError::InvalidResponse(error.to_string()))?;
    let mut builder = client.request(method, &request.url);
    for query in request.query.iter().filter(|query| query.enabled) {
        builder = builder.query(&[(query.name.as_str(), query.value.as_str())]);
    }
    for header in request.headers.iter().filter(|header| header.enabled) {
        builder = builder.header(header.name.as_str(), header.value.as_str());
    }
    if let Body::Raw {
        content_type,
        value,
    } = &request.body
    {
        if !request
            .headers
            .iter()
            .any(|header| header.name.eq_ignore_ascii_case("content-type"))
        {
            builder = builder.header("content-type", content_type.as_str());
        }
        builder = builder.body(value.clone());
    }
    let started = std::time::Instant::now();
    let response = builder
        .send()
        .map_err(|error| ClientError::InvalidResponse(error.to_string()))?;
    let duration = started.elapsed();
    let status = response.status();
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| Header::new(name.as_str(), value.to_str().unwrap_or("")))
        .collect::<Vec<_>>();
    let cookies = response
        .cookies()
        .map(|cookie| Header::new(cookie.name().to_string(), cookie.value().to_string()))
        .collect::<Vec<_>>();
    let body = response
        .text()
        .map_err(|error| ClientError::InvalidResponse(error.to_string()))?;
    Ok(ResponseRecord {
        status: status.as_u16(),
        status_text: status.canonical_reason().unwrap_or("").to_string(),
        duration_ms: duration.as_millis() as u64,
        size_bytes: body.len(),
        headers,
        cookies,
        body,
    })
}
