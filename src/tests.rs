use super::*;
use std::io::{Read, Write};

#[test]
fn creates_edits_sends_and_tracks_history() {
    let mut workspace = Workspace::new("local", "Local workspace");
    workspace.create_request(Request::new(
        "req-1",
        "List users",
        Method::Get,
        "{{base_url}}/users",
    ));

    workspace
        .edit_request("req-1", |request| {
            request.method = Method::Post;
            request
                .headers
                .push(Header::new("Content-Type", "application/json"));
            request.body = Body::raw_json(r#"{"name":"Ada"}"#);
        })
        .unwrap();

    let response = workspace.send_preview("req-1").unwrap();
    let request = workspace.requests()[0];

    assert_eq!(request.method, Method::Post);
    assert_eq!(response.status, 200);
    assert_eq!(request.history.len(), 1);
    assert!(request.history[0].body.contains(r#""requestId": "req-1""#));
}

#[test]
fn parses_and_labels_options_method() {
    assert_eq!(Method::from("OPTIONS"), Method::Options);
    assert_eq!(Method::Options.as_str(), "OPTIONS");
}

#[test]
fn persists_and_reloads_workspace_json() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = default_workspace_path(temp_dir.path(), "local");
    let mut workspace = Workspace::new("local", "Local workspace");
    workspace.create_request(Request::new(
        "req-1",
        "Ping",
        Method::Get,
        "https://api.example.com/ping",
    ));
    workspace.send_preview("req-1").unwrap();

    save_workspace(&path, &workspace).unwrap();
    let loaded = load_workspace(&path).unwrap();

    assert_eq!(loaded.id, "local");
    assert_eq!(loaded.requests().len(), 1);
    assert_eq!(loaded.requests()[0].history.len(), 1);
}

#[test]
fn sends_real_http_request_and_records_history() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0_u8; 1024];
        let read = stream.read(&mut buffer).unwrap();
        let request = String::from_utf8_lossy(&buffer[..read]);
        let request_lower = request.to_ascii_lowercase();
        assert!(request.starts_with("POST /users HTTP/1.1"));
        assert!(request_lower.contains("content-type: application/json"));
        assert!(request.contains(r#"{"name":"Ada"}"#));
        stream
            .write_all(
                b"HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nSet-Cookie: session=abc\r\n\r\n{\"ok\":true}",
            )
            .unwrap();
    });

    let mut workspace = Workspace::new("local", "Local workspace");
    let mut request = Request::new(
        "req-http",
        "Create user",
        Method::Post,
        format!("http://{address}/users"),
    );
    request
        .headers
        .push(Header::new("Content-Type", "application/json"));
    request.body = Body::raw_json(r#"{"name":"Ada"}"#);
    workspace.create_request(request);

    let response = workspace.send_http("req-http").unwrap();
    server.join().unwrap();

    assert_eq!(response.status, 201);
    assert_eq!(response.body, r#"{"ok":true}"#);
    assert_eq!(response.cookies[0].name, "session");
    assert_eq!(workspace.requests()[0].history.len(), 1);
}

#[test]
fn sends_options_http_request() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0_u8; 1024];
        let read = stream.read(&mut buffer).unwrap();
        let request = String::from_utf8_lossy(&buffer[..read]);
        assert!(request.starts_with("OPTIONS /capabilities HTTP/1.1"));
        stream
            .write_all(b"HTTP/1.1 204 No Content\r\nAllow: GET, POST, OPTIONS\r\n\r\n")
            .unwrap();
    });

    let request = Request::new(
        "req-options",
        "Check capabilities",
        Method::Options,
        format!("http://{address}/capabilities"),
    );

    let response = send_http_request(&request).unwrap();
    server.join().unwrap();

    assert_eq!(response.status, 204);
    assert_eq!(response.status_text, "No Content");
}

#[test]
fn parses_sse_event_with_data() {
    use std::time::Instant;
    let start_time = Instant::now();
    let event_data = "data: {\"message\":\"hello\"}";
    let message = parse_sse_event(event_data, start_time).unwrap();
    assert_eq!(message.data, "{\"message\":\"hello\"}");
    assert_eq!(message.direction, StreamDirection::Received);
    assert!(message.event_type.is_none());
    assert!(message.event_id.is_none());
}

#[test]
fn parses_sse_event_with_event_type_and_id() {
    use std::time::Instant;
    let start_time = Instant::now();
    let event_data = "event: update\nid: 123\ndata: value";
    let message = parse_sse_event(event_data, start_time).unwrap();
    assert_eq!(message.data, "value");
    assert_eq!(message.event_type, Some("update".to_string()));
    assert_eq!(message.event_id, Some("123".to_string()));
}

#[test]
fn parses_sse_multiline_data() {
    use std::time::Instant;
    let start_time = Instant::now();
    let event_data = "data: line1\ndata: line2\ndata: line3";
    let message = parse_sse_event(event_data, start_time).unwrap();
    assert_eq!(message.data, "line1\nline2\nline3");
}

#[test]
fn stream_message_fields() {
    let msg = StreamMessage {
        direction: StreamDirection::Received,
        event_type: Some("test".to_string()),
        event_id: Some("id-1".to_string()),
        data: "payload".to_string(),
        size_bytes: 7,
        timestamp_ms: 1000,
        received_at: 0,
        is_binary: false,
        binary_utf8: None,
    };
    assert_eq!(msg.direction, StreamDirection::Received);
    assert_eq!(msg.event_type.as_deref(), Some("test"));
    assert_eq!(msg.data, "payload");
    assert_eq!(msg.size_bytes, 7);
}
