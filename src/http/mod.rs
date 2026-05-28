use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::client::conn::http1::handshake;
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::runtime::Builder as RuntimeBuilder;
use tokio::sync::mpsc;
use tokio_rustls::TlsConnector;
use url::Url;

use crate::domain::{
    Body, ClientError, Header, Request, ResponseRecord, ResponseTiming, Result, StreamDirection,
    StreamMessage,
};

trait AsyncReadWrite: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin {}
impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin> AsyncReadWrite for T {}

trait RawIo: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send {}
impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send> RawIo for T {}

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
        let root_store =
            rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
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

    let mut req_builder = http::Request::builder().method(method).uri(uri);

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
        Body::Raw {
            content_type,
            value,
        } if !value.is_empty() => {
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

pub fn connect_sse(
    url: String,
    message_tx: mpsc::Sender<StreamMessage>,
    cancel_token: Arc<AtomicBool>,
) -> Result<()> {
    let rt = RuntimeBuilder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| ClientError::InvalidResponse(e.to_string()))?;
    rt.block_on(connect_sse_async(url, message_tx, cancel_token))
}

/// Connect to an SSE stream and read events, with automatic reconnection.
/// The server may close the connection after periods of inactivity (common for SSE).
/// This function automatically reconnects so the caller never sees a disconnect.
/// Only returns when the user cancels via `cancel_token`, or on a non-recoverable error.
async fn connect_sse_async(
    url: String,
    message_tx: mpsc::Sender<StreamMessage>,
    cancel_token: Arc<AtomicBool>,
) -> Result<()> {
    let parsed_url =
        Url::parse(&url).map_err(|e| ClientError::InvalidResponse(format!("Invalid URL: {e}")))?;

    let host = parsed_url
        .host_str()
        .ok_or_else(|| ClientError::InvalidResponse("URL has no host".into()))?;
    let port = parsed_url
        .port_or_known_default()
        .ok_or_else(|| ClientError::InvalidResponse("URL has no known port".into()))?;
    let is_https = parsed_url.scheme() == "https";
    let path = parsed_url.path();
    let query_str = parsed_url.query().unwrap_or("");

    let has_sent_first_message = std::sync::atomic::AtomicBool::new(false);

    // Outer loop: reconnect automatically when the server closes the connection.
    while !cancel_token.load(Ordering::SeqCst) {
        // Connect (TCP + optional TLS)
        let connect_result = TcpStream::connect((host, port)).await;
        let stream = match connect_result {
            Ok(s) => s,
            Err(_) => {
                // Brief delay before retry to avoid busy-looping on network errors.
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                continue;
            }
        };

        let mut io: Box<dyn RawIo> = if is_https {
            use rustls::ClientConfig;
            use rustls_pki_types::ServerName;

            let dns_name = match ServerName::try_from(host.to_string()) {
                Ok(n) => n,
                Err(_) => continue,
            };
            let root_store =
                rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            let config = ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_no_client_auth();
            let connector = TlsConnector::from(Arc::new(config));
            match connector.connect(dns_name, stream).await {
                Ok(tls) => Box::new(tls) as Box<dyn RawIo>,
                Err(_) => {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    continue;
                }
            }
        } else {
            Box::new(stream) as Box<dyn RawIo>
        };

        // Send HTTP request
        let request_line = format!(
            "GET {}{} HTTP/1.1\r\nHost: {}\r\nAccept: text/event-stream\r\nUser-Agent: apie-stream-client\r\nConnection: keep-alive\r\n\r\n",
            path,
            if query_str.is_empty() { "" } else { "?" },
            host
        );
        if io.write_all(request_line.as_bytes()).await.is_err() {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            continue;
        }

        // Read response headers
        let mut response_buf: Vec<u8> = Vec::new();
        let mut header_read_ok = false;
        loop {
            let mut buf = [0u8; 4096];
            let n = match tokio::time::timeout(std::time::Duration::from_secs(5), io.read(&mut buf))
                .await
            {
                Ok(Ok(0)) => break,
                Ok(Ok(n)) => n,
                _ => break,
            };
            response_buf.extend_from_slice(&buf[..n]);
            if response_buf.windows(4).any(|w| w == b"\r\n\r\n") {
                header_read_ok = true;
                break;
            }
        }
        if !header_read_ok {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            continue;
        }

        let response_str = String::from_utf8_lossy(&response_buf);
        let status_line = response_str.lines().next().unwrap_or("");
        let status_code_str = status_line.split_whitespace().nth(1).unwrap_or("");
        let status_code: u16 = match status_code_str.parse() {
            Ok(c) => c,
            Err(_) => continue,
        };
        if status_code != 200 {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            continue;
        }

        // Body offset
        let body_offset = response_str
            .find("\r\n\r\n")
            .map(|i| i + 4)
            .unwrap_or(response_str.len());
        let mut buffer = String::new();
        if body_offset < response_str.len() {
            buffer.push_str(&response_str[body_offset..]);
        }

        // Send a fake first message only once (not on every reconnect).
        if !has_sent_first_message.load(Ordering::SeqCst) {
            has_sent_first_message.store(true, Ordering::SeqCst);
            let _ = message_tx
                .send(StreamMessage {
                    direction: StreamDirection::Received,
                    event_type: None,
                    event_id: None,
                    data: String::new(),
                    size_bytes: 0,
                    timestamp_ms: 0,
                    received_at: 0,
                })
                .await;
        }

        // Inner loop: read SSE body from the stream.
        // When the server closes the connection (read returns 0),
        // we break back to the outer loop to reconnect.
        let start_time = Instant::now();
        loop {
            if cancel_token.load(Ordering::SeqCst) {
                return Ok(());
            }

            let mut buf = [0u8; 8192];
            let n = tokio::time::timeout(std::time::Duration::from_millis(100), io.read(&mut buf))
                .await;

            match n {
                Ok(Ok(0)) => {
                    // Stream closed by server — reconnect in outer loop.
                    break;
                }
                Ok(Ok(n)) => {
                    let chunk_str = String::from_utf8_lossy(&buf[..n]);
                    buffer.push_str(&chunk_str);

                    while let Some(event_end) = buffer.find("\n\n") {
                        let event_data = buffer[..event_end].to_string();
                        buffer = buffer[event_end + 2..].to_string();

                        if let Ok(message) = parse_sse_event(&event_data, start_time) {
                            if message_tx.send(message).await.is_err() {
                                return Ok(());
                            }
                        }
                    }
                }
                Ok(Err(e)) => {
                    // Read error — retry connection after brief delay.
                    return Err(ClientError::InvalidResponse(format!("Stream error: {e}")));
                }
                Err(_) => {
                    // Timeout — no data yet, but connection is still alive.
                    continue;
                }
            }
        }

        // Brief delay before reconnecting to avoid hammering the server.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    Ok(())
}

pub fn parse_sse_event(event_data: &str, start_time: Instant) -> Result<StreamMessage> {
    let mut event_type = None;
    let mut event_id = None;
    let mut data_lines = Vec::new();

    for line in event_data.lines() {
        if let Some(rest) = line.strip_prefix("event:") {
            event_type = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("id:") {
            event_id = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.trim().to_string());
        }
    }

    let data = data_lines.join("\n");
    let size_bytes = data.len();
    let timestamp_ms = start_time.elapsed().as_millis() as u64;
    let received_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    Ok(StreamMessage {
        direction: StreamDirection::Received,
        event_type,
        event_id,
        data,
        size_bytes,
        timestamp_ms,
        received_at,
    })
}
