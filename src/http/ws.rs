use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::runtime::Builder as RuntimeBuilder;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use url::Url;

use crate::domain::{ClientError, Result, StreamDirection, StreamMessage};

trait WsIo: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send {}
impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send> WsIo for T {}

/// Synchronous entry point — builds a current-thread tokio runtime and
/// drives the async WebSocket connection to completion.
pub fn connect_ws(
    url: String,
    message_tx: mpsc::Sender<StreamMessage>,
    cancel_token: Arc<AtomicBool>,
) -> Result<()> {
    let rt = RuntimeBuilder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| ClientError::InvalidResponse(e.to_string()))?;
    rt.block_on(connect_ws_async(url, message_tx, cancel_token))
}

/// Connect to a WebSocket endpoint and relay received messages on `message_tx`.
///
/// Respects `cancel_token` — when set, the connection is closed gracefully.
/// Text frames become `StreamMessage` with `direction: Received`. Binary
/// frames are hex-dumped. Ping frames are answered automatically by
/// `tokio-tungstenite`.
async fn connect_ws_async(
    url: String,
    message_tx: mpsc::Sender<StreamMessage>,
    cancel_token: Arc<AtomicBool>,
) -> Result<()> {
    let parsed = Url::parse(&url)
        .map_err(|e| ClientError::InvalidResponse(format!("Invalid WebSocket URL: {e}")))?;

    let is_secure = parsed.scheme() == "wss";
    let host = parsed
        .host_str()
        .ok_or_else(|| ClientError::InvalidResponse("URL has no host".into()))?;
    let port = parsed
        .port_or_known_default()
        .unwrap_or(if is_secure { 443 } else { 80 });

    // TCP connect
    let addr = format!("{host}:{port}");
    let stream = TcpStream::connect(&addr)
        .await
        .map_err(|e| ClientError::InvalidResponse(format!("TCP connect failed: {e}")))?;

    // Optional TLS wrapping (reuses the same rustls setup as the HTTP client)
    let maybe_tls: Box<dyn WsIo> = if is_secure {
        use rustls::ClientConfig;
        use rustls_pki_types::ServerName;
        use tokio_rustls::TlsConnector;

        let dns_name = ServerName::try_from(host.to_string())
            .map_err(|_| ClientError::InvalidResponse("Invalid DNS name for TLS".into()))?;
        let root_store =
            rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let config = ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        let connector = TlsConnector::from(Arc::new(config));
        let tls_stream = connector
            .connect(dns_name, stream)
            .await
            .map_err(|e| ClientError::InvalidResponse(format!("TLS handshake failed: {e}")))?;
        Box::new(tls_stream)
    } else {
        Box::new(stream)
    };

    // Build the WebSocket request
    let ws_url = format!(
        "ws{}://{}{}{}",
        if is_secure { "s" } else { "" },
        if port == 80 || port == 443 {
            host.to_string()
        } else {
            format!("{host}:{port}")
        },
        parsed.path(),
        parsed.query().map(|q| format!("?{q}")).unwrap_or_default()
    );

    let request = http::Request::builder()
        .uri(&ws_url)
        .header("Host", &addr)
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .body(())
        .map_err(|e| ClientError::InvalidResponse(e.to_string()))?;

    // WebSocket handshake
    let (ws_stream, _) = tokio_tungstenite::client_async(request, maybe_tls)
        .await
        .map_err(|e| ClientError::InvalidResponse(format!("WebSocket handshake failed: {e}")))?;

    let (mut write, mut read) = ws_stream.split();

    // Signal connection established with an empty message (matches SSE behaviour)
    let start = Instant::now();
    let _ = message_tx
        .send(StreamMessage {
            direction: StreamDirection::Received,
            event_type: None,
            event_id: None,
            data: String::new(),
            size_bytes: 0,
            timestamp_ms: 0,
            received_at: now_epoch_ms(),
        })
        .await;

    // Read loop
    loop {
        if cancel_token.load(Ordering::SeqCst) {
            break;
        }

        tokio::select! {
            frame = read.next() => {
                match frame {
                    Some(Ok(msg)) => {
                        match msg {
                            Message::Text(text) => {
                                let data: String = text.into();
                                let size = data.len();
                                let msg = StreamMessage {
                                    direction: StreamDirection::Received,
                                    event_type: None,
                                    event_id: None,
                                    data,
                                    size_bytes: size,
                                    timestamp_ms: start.elapsed().as_millis() as u64,
                                    received_at: now_epoch_ms(),
                                };
                                if message_tx.send(msg).await.is_err() {
                                    break;
                                }
                            }
                            Message::Binary(bytes) => {
                                let data = format_bytes_hex(&bytes);
                                let msg = StreamMessage {
                                    direction: StreamDirection::Received,
                                    event_type: None,
                                    event_id: None,
                                    data,
                                    size_bytes: bytes.len(),
                                    timestamp_ms: start.elapsed().as_millis() as u64,
                                    received_at: now_epoch_ms(),
                                };
                                if message_tx.send(msg).await.is_err() {
                                    break;
                                }
                            }
                            Message::Ping(data) => {
                                let _ = write.send(Message::Pong(data)).await;
                            }
                            Message::Pong(_) => { /* ignore */ }
                            Message::Close(_) => break,
                            Message::Frame(_) => { /* raw frame, ignore */ }
                        }
                    }
                    Some(Err(_)) => break,
                    None => break,
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                // Periodic cancel check
            }
        }
    }

    // Graceful close
    let _ = write.send(Message::Close(None)).await;

    Ok(())
}

/// Format a byte slice as a hex dump (one line of hex pairs).
fn format_bytes_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Current wall-clock time as Unix-epoch milliseconds.
fn now_epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bytes_hex_works() {
        assert_eq!(format_bytes_hex(&[0x48, 0x65, 0x6c]), "48 65 6c");
        assert_eq!(format_bytes_hex(&[]), "");
    }

    #[test]
    fn rejects_non_ws_url() {
        let rt = RuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (tx, _rx) = mpsc::channel(1);
        let cancel = Arc::new(AtomicBool::new(false));
        let result = rt.block_on(connect_ws_async("https://example.com".into(), tx, cancel));
        assert!(result.is_err());
    }

    #[test]
    fn rejects_invalid_url() {
        let rt = RuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (tx, _rx) = mpsc::channel(1);
        let cancel = Arc::new(AtomicBool::new(false));
        let result = rt.block_on(connect_ws_async("not a url".into(), tx, cancel));
        assert!(result.is_err());
    }
}
