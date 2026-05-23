use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::time::Instant;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::client::conn::http1::handshake;
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;
use tokio::runtime::Builder as RuntimeBuilder;
use tokio_rustls::TlsConnector;
use url::Url;

use crate::domain::{Body, ClientError, Header, Request, ResponseRecord, ResponseTiming, Result};

trait AsyncReadWrite: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin {}
impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin> AsyncReadWrite for T {}

type HyperBody = Full<Bytes>;

pub fn send_http_request(request: &Request) -> Result<ResponseRecord> {
    let rt = RuntimeBuilder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| ClientError::InvalidResponse(e.to_string()))?;
    rt.block_on(send_http_request_async(request))
}

async fn send_http_request_async(request: &Request) -> Result<ResponseRecord> {
    let total_start = Instant::now();
    let mut current_url = Url::parse(&request.url)
        .map_err(|e| ClientError::InvalidResponse(format!("Invalid URL: {e}")))?;

    for redirect_count in 0..=10usize {
        let (body_str, status_code, status_text, resp_headers, resp_cookies, timing) =
            send_single(&current_url, request).await?;

        if redirect_count < 10 {
            if let Some(location_value) = resp_headers
                .iter()
                .find(|h| h.name.eq_ignore_ascii_case("location"))
                .map(|h| h.value.as_str())
            {
                if let Ok(new_url) = current_url.join(location_value) {
                    current_url = new_url;
                    continue;
                }
            }
        }

        let total = total_start.elapsed();
        return Ok(ResponseRecord {
            status: status_code,
            status_text,
            duration_ms: total.as_millis() as u64,
            size_bytes: body_str.len(),
            headers: resp_headers,
            cookies: resp_cookies,
            body: body_str,
            timing: Some(timing),
        });
    }

    Err(ClientError::InvalidResponse("Too many redirects".into()))
}

async fn send_single(
    url: &Url,
    request: &Request,
) -> Result<(
    String,
    u16,
    String,
    Vec<Header>,
    Vec<Header>,
    ResponseTiming,
)> {
    let host = url
        .host_str()
        .ok_or_else(|| ClientError::InvalidResponse("URL has no host".into()))?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| ClientError::InvalidResponse("URL has no known port".into()))?;
    let is_https = url.scheme() == "https";

    let dns_start = Instant::now();
    let addrs: Vec<_> = (host, port)
        .to_socket_addrs()
        .map_err(|e| ClientError::InvalidResponse(format!("DNS resolution failed: {e}")))?
        .collect();
    let dns_ms = dns_start.elapsed().as_millis() as u64;

    let addr = addrs
        .into_iter()
        .next()
        .ok_or_else(|| ClientError::InvalidResponse("DNS returned no addresses".into()))?;

    let tcp_start = Instant::now();
    let stream = TcpStream::connect(addr)
        .await
        .map_err(|e| ClientError::InvalidResponse(format!("TCP connection failed: {e}")))?;
    let connect_ms = tcp_start.elapsed().as_millis() as u64;

    let tls_ms;
    let io: TokioIo<Box<dyn AsyncReadWrite>> = if is_https {
        use rustls::ClientConfig;
        use rustls_pki_types::ServerName;

        let dns_name = ServerName::try_from(host.to_string())
            .map_err(|_| ClientError::InvalidResponse("Invalid DNS name for TLS".into()))?;
        let root_store = rustls::RootCertStore::from_iter(
            webpki_roots::TLS_SERVER_ROOTS.iter().cloned(),
        );
        let config = ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        let connector = TlsConnector::from(Arc::new(config));
        let tls_start = Instant::now();
        let tls_stream = connector
            .connect(dns_name, stream)
            .await
            .map_err(|e| ClientError::InvalidResponse(format!("TLS handshake failed: {e}")))?;
        tls_ms = tls_start.elapsed().as_millis() as u64;
        TokioIo::new(Box::new(tls_stream) as Box<dyn AsyncReadWrite>)
    } else {
        tls_ms = 0;
        TokioIo::new(Box::new(stream) as Box<dyn AsyncReadWrite>)
    };

    let (mut sender, conn) = handshake::<_, HyperBody>(io)
        .await
        .map_err(|e| ClientError::InvalidResponse(format!("HTTP handshake failed: {e}")))?;
    tokio::spawn(conn);

    let path = url.path();
    let uri: http::Uri = match url.query() {
        Some(query) => format!("{}?{}", path, query).parse(),
        None => path.parse(),
    }
    .map_err(|e| ClientError::InvalidResponse(format!("Invalid request URI: {e}")))?;

    let method = http::Method::from_bytes(request.method.as_str().as_bytes())
        .map_err(|e| ClientError::InvalidResponse(e.to_string()))?;

    let mut req_builder = http::Request::builder()
        .method(method)
        .uri(uri);

    for header in request.headers.iter().filter(|h| h.enabled) {
        if let (Ok(name), Ok(value)) = (
            http::HeaderName::from_bytes(header.name.as_bytes()),
            http::HeaderValue::from_str(&header.value),
        ) {
            req_builder = req_builder.header(name, value);
        }
    }

    let host_header = if port == 80 || port == 443 {
        host.to_string()
    } else {
        format!("{host}:{port}")
    };
    if let Ok(host_value) = http::HeaderValue::from_str(&host_header) {
        req_builder = req_builder.header("host", host_value);
    }

    let body_value = match &request.body {
        Body::Raw { content_type, value } if !value.is_empty() => {
            let has_content_type = request
                .headers
                .iter()
                .any(|h| h.enabled && h.name.eq_ignore_ascii_case("content-type"));
            if !has_content_type {
                if let Ok(ct) = http::HeaderValue::from_str(content_type) {
                    req_builder = req_builder.header("content-type", ct);
                }
            }
            value.clone()
        }
        _ => String::new(),
    };

    let http_request = req_builder
        .body(Full::new(Bytes::from(body_value)))
        .map_err(|e| ClientError::InvalidResponse(e.to_string()))?;

    let ttfb_start = Instant::now();
    let response = sender
        .send_request(http_request)
        .await
        .map_err(|e| ClientError::InvalidResponse(format!("HTTP request failed: {e}")))?;
    let ttfb_ms = ttfb_start.elapsed().as_millis() as u64;

    let status_code = response.status().as_u16();
    let status_text = response
        .status()
        .canonical_reason()
        .unwrap_or("")
        .to_string();
    let resp_headers_map = response.headers().clone();

    let transfer_start = Instant::now();
    let body_bytes = response
        .into_body()
        .collect()
        .await
        .map_err(|e| ClientError::InvalidResponse(e.to_string()))?
        .to_bytes();
    let transfer_ms = transfer_start.elapsed().as_millis() as u64;

    let body_str = String::from_utf8_lossy(&body_bytes).to_string();

    let mut headers = Vec::new();
    let mut cookies = Vec::new();
    for (name, value) in resp_headers_map.iter() {
        let name_lower = name.as_str().to_ascii_lowercase();
        if name_lower == "set-cookie" {
            if let Ok(cookie_str) = value.to_str() {
                if let Some((cname, rest)) = cookie_str.split_once('=') {
                    let cvalue = rest.split(';').next().unwrap_or(rest).to_string();
                    cookies.push(Header::new(cname.trim(), cvalue));
                }
            }
        } else {
            headers.push(Header::new(name.as_str(), value.to_str().unwrap_or("")));
        }
    }

    let timing = ResponseTiming {
        dns_lookup_ms: dns_ms,
        connect_ms,
        tls_handshake_ms: tls_ms,
        time_to_first_byte_ms: ttfb_ms,
        transfer_ms,
    };

    Ok((body_str, status_code, status_text, headers, cookies, timing))
}
