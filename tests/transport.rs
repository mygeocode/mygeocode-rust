//! The default transport against a small local HTTP server (no internet), then the client end to end.

mod common;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use common::*;
use mygeocode::*;

/// Serves forever on a free port. Paths: /v1/slow waits 1.5 s, /v1/limited answers 429, /v1/chunked answers in
/// chunks, anything else echoes the request as JSON.
fn server() -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!(
        "http://127.0.0.1:{}/v1",
        listener.local_addr().unwrap().port()
    );
    let posts = Arc::new(AtomicUsize::new(0));
    let counter = posts.clone();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let counter = counter.clone();
            thread::spawn(move || handle(stream, counter));
        }
    });
    (base, posts)
}

fn handle(mut stream: TcpStream, posts: Arc<AtomicUsize>) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        let n = match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => n,
        };
        buf.extend_from_slice(&chunk[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let length = head
        .lines()
        .find_map(|l| {
            l.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(|v| v.trim().parse::<usize>().unwrap_or(0))
        })
        .unwrap_or(0);
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < length {
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    let first = head.lines().next().unwrap_or("").to_string();
    let mut parts = first.split(' ');
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
    let header = |name: &str| {
        head.lines().find_map(|l| {
            let (k, v) = l.split_once(':')?;
            if k.trim().eq_ignore_ascii_case(name) {
                Some(v.trim().to_string())
            } else {
                None
            }
        })
    };
    if method == "POST" {
        posts.fetch_add(1, Ordering::SeqCst);
    }
    if path.starts_with("/v1/slow") {
        thread::sleep(Duration::from_millis(1500));
    }
    let reply = |stream: &mut TcpStream, status: &str, extra: &str, body: &str| {
        let _ = write!(
            stream,
            "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{}\r\n{}",
            status,
            body.len(),
            extra,
            body
        );
    };
    if path.starts_with("/v1/limited") {
        reply(
            &mut stream,
            "429 Too Many Requests",
            "Retry-After: 1\r\n",
            r#"{"status":"error","error":{"code":"rate_limited","message":"Slow down."}}"#,
        );
        return;
    }
    if path.starts_with("/v1/chunked") {
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nX-Request-Id: chunky\r\n\r\n6\r\n{\"stat\r\n12\r\nus\":\"ok\",\"result\":\r\n5\r\nnull}\r\n0\r\n\r\n");
        return;
    }
    let echo = Value::Object(vec![
        ("status".into(), Value::String("ok".into())),
        ("method".into(), Value::String(method)),
        ("path".into(), Value::String(path)),
        (
            "ua".into(),
            header("user-agent")
                .map(Value::String)
                .unwrap_or(Value::Null),
        ),
        (
            "key".into(),
            header("x-api-key")
                .map(Value::String)
                .unwrap_or(Value::Null),
        ),
        (
            "content_type".into(),
            header("content-type")
                .map(Value::String)
                .unwrap_or(Value::Null),
        ),
        (
            "length".into(),
            header("content-length")
                .map(Value::String)
                .unwrap_or(Value::Null),
        ),
        (
            "body".into(),
            if body.is_empty() {
                Value::Null
            } else {
                Value::String(String::from_utf8_lossy(&body).to_string())
            },
        ),
        ("results".into(), Value::Array(vec![])),
    ]);
    reply(
        &mut stream,
        "200 OK",
        "X-Request-Id: local-1\r\nX-Quota-Limit: 2500\r\n",
        &echo.to_json(),
    );
}

fn request(
    method: &str,
    url: &str,
    body: Option<&str>,
    millis: u64,
    headers: &[(&str, &str)],
) -> HttpRequest {
    HttpRequest {
        method: method.to_string(),
        url: url.to_string(),
        headers: headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        body: body.map(|b| b.to_string()),
        timeout: Duration::from_millis(millis),
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[test]
fn get_with_headers() {
    let (base, _) = server();
    let r = DefaultTransport::new()
        .send(&request(
            "GET",
            &format!("{}/x?a=1", base),
            None,
            5000,
            &[("X-API-Key", "k1"), ("User-Agent", "ua-test")],
        ))
        .ok()
        .unwrap();
    let b = Value::parse(&r.body).unwrap();
    assert_eq!(r.status, 200);
    assert_eq!(
        (
            b.get("method").as_str(),
            b.get("path").as_str(),
            b.get("key").as_str(),
            b.get("ua").as_str()
        ),
        (Some("GET"), Some("/v1/x?a=1"), Some("k1"), Some("ua-test"))
    );
    assert_eq!(r.header("X-Request-Id"), Some("local-1"));
}

#[test]
fn post_body_in_utf8() {
    let (base, _) = server();
    let text = "{\"queries\":[\"Z\u{fc}rich, Schweiz \u{1f1e8}\u{1f1ed}\"]}";
    let r = DefaultTransport::new()
        .send(&request(
            "POST",
            &format!("{}/x", base),
            Some(text),
            5000,
            &[("Content-Type", "application/json")],
        ))
        .ok()
        .unwrap();
    let b = Value::parse(&r.body).unwrap();
    assert_eq!(
        (
            b.get("method").as_str(),
            b.get("body").as_str(),
            b.get("content_type").as_str()
        ),
        (Some("POST"), Some(text), Some("application/json"))
    );
    assert_eq!(
        b.get("length").as_str(),
        Some(text.len().to_string().as_str())
    );
}

#[test]
fn chunked_answers_and_error_statuses() {
    let (base, _) = server();
    let r = DefaultTransport::new()
        .send(&request(
            "GET",
            &format!("{}/chunked", base),
            None,
            5000,
            &[],
        ))
        .ok()
        .unwrap();
    assert_eq!(
        (r.status, r.body.as_str(), r.header("x-request-id")),
        (200, r#"{"status":"ok","result":null}"#, Some("chunky"))
    );
    let r = DefaultTransport::new()
        .send(&request(
            "GET",
            &format!("{}/limited", base),
            None,
            5000,
            &[],
        ))
        .ok()
        .unwrap();
    assert_eq!((r.status, r.header("retry-after")), (429, Some("1")));
}

#[test]
fn timeout_covers_the_whole_exchange() {
    let (base, _) = server();
    let started = Instant::now();
    let e = DefaultTransport::new()
        .send(&request("GET", &format!("{}/slow", base), None, 300, &[]))
        .err()
        .unwrap();
    assert!(matches!(e, TransportError::Timeout(_)), "{}", e);
    assert!(
        started.elapsed() < Duration::from_millis(1200),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn connection_refused() {
    let e = DefaultTransport::new()
        .send(&request(
            "GET",
            &format!("http://127.0.0.1:{}/v1/x", free_port()),
            None,
            2000,
            &[],
        ))
        .err()
        .unwrap();
    assert!(matches!(e, TransportError::Network(..)), "{}", e);
}

#[test]
fn client_end_to_end() {
    let (base, posts) = server();
    let c = ClientBuilder::new()
        .key("k2")
        .base_url(base.clone())
        .build_blocking();
    let r = c
        .forward("Brandenburg Gate, Berlin", &ForwardOptions::new())
        .unwrap();
    assert_eq!(
        (r.raw.get("method").as_str(), r.raw.get("key").as_str()),
        (Some("GET"), Some("k2"))
    );
    assert!(r
        .raw
        .get("ua")
        .as_str()
        .unwrap()
        .starts_with("mygeocode-rust/"));
    assert!(r
        .raw
        .get("path")
        .as_str()
        .unwrap()
        .contains("q=Brandenburg+Gate%2C+Berlin"));
    assert_eq!(
        (r.meta.request_id.as_deref(), r.meta.quota_limit),
        (Some("local-1"), Some(2500))
    );
    let before = posts.load(Ordering::SeqCst);
    let b = c
        .forward_batch(&["Brandenburg Gate, Berlin"], &ForwardBatchOptions::new())
        .unwrap();
    assert_eq!(b.raw.get("path").as_str(), Some("/v1/forward?lang=en"));
    assert_eq!(
        Value::parse(b.raw.get("body").as_str().unwrap()).unwrap(),
        json(r#"{"queries":["Brandenburg Gate, Berlin"]}"#)
    );
    assert_eq!(posts.load(Ordering::SeqCst), before + 1);
}

#[test]
fn client_errors_end_to_end() {
    let (base, _) = server();
    let e = ClientBuilder::new()
        .base_url(base.replace("/v1", "/v1/limited"))
        .max_retries(0)
        .build_blocking()
        .forward("x", &ForwardOptions::new())
        .unwrap_err();
    assert_eq!(
        (e.code(), e.api().unwrap().retry_after),
        (Some("rate_limited"), Some(Duration::from_secs(1)))
    );
    let e = ClientBuilder::new()
        .base_url(base.replace("/v1", "/v1/slow"))
        .timeout(Duration::from_millis(300))
        .max_retries(0)
        .build_blocking()
        .forward("x", &ForwardOptions::new())
        .unwrap_err();
    assert!(e.is_timeout(), "{}", e);
    let e = ClientBuilder::new()
        .base_url(format!("http://127.0.0.1:{}/v1", free_port()))
        .max_retries(0)
        .build_blocking()
        .forward("x", &ForwardOptions::new())
        .unwrap_err();
    assert!(matches!(e, Error::Network(..)), "{}", e);
}
