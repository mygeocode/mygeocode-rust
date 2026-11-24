//! The HTTP layer: the request and response the client hands to a [`Transport`], and the default transport.

use std::fmt;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

/// What the client asks a [`Transport`] to send.
#[derive(Clone, Debug, PartialEq)]
pub struct HttpRequest {
    /// `GET` or `POST`.
    pub method: String,
    /// The full URL with the query string.
    pub url: String,
    /// The request headers.
    pub headers: Vec<(String, String)>,
    /// The JSON body of a POST.
    pub body: Option<String>,
    /// How long to wait for the whole answer.
    pub timeout: Duration,
}

impl HttpRequest {
    /// One request header, matched without regard to case.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// What a [`Transport`] returns. Any HTTP status is a response; only a failure to get one is an error.
#[derive(Clone, Debug, PartialEq)]
pub struct HttpResponse {
    /// The HTTP status.
    pub status: u16,
    /// The response headers, names in lower case.
    pub headers: Vec<(String, String)>,
    /// The body text.
    pub body: String,
}

impl HttpResponse {
    /// Creates a response; header names are stored in lower case.
    pub fn new(
        status: u16,
        headers: Vec<(String, String)>,
        body: impl Into<String>,
    ) -> HttpResponse {
        HttpResponse {
            status,
            headers: headers
                .into_iter()
                .map(|(k, v)| (k.to_ascii_lowercase(), v))
                .collect(),
            body: body.into(),
        }
    }

    /// One response header, matched without regard to case.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// Why a [`Transport`] got no answer.
#[derive(Debug)]
pub enum TransportError {
    /// No answer within the request's timeout.
    Timeout(String),
    /// The connection failed (DNS, refused, reset, TLS), with the source error when there is one.
    Network(String, Option<Box<dyn std::error::Error + Send + Sync>>),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportError::Timeout(m) => write!(f, "timed out: {}", m),
            TransportError::Network(m, _) => write!(f, "{}", m),
        }
    }
}

impl std::error::Error for TransportError {}

/// The HTTP layer. Give the client your own with `ClientBuilder::transport`, to plug in another HTTP library or to
/// test your code without the network. A closure `Fn(&HttpRequest) -> Result<HttpResponse, TransportError>` is a
/// transport too.
pub trait Transport: Send + Sync {
    /// Sends one request and returns the answer, whatever its HTTP status.
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError>;
}

impl<F> Transport for F
where
    F: Fn(&HttpRequest) -> Result<HttpResponse, TransportError> + Send + Sync,
{
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self(request)
    }
}

/// The default transport: HTTP/1.1 over the standard library's `TcpStream`, with HTTPS through rustls (the default
/// `rustls` feature, Mozilla's root certificates built in) or the system's TLS library (the `native-tls` feature).
///
/// The request's timeout covers the whole exchange: connecting, sending and reading the answer. Name lookup uses the
/// system resolver, which has no timeout of its own. Each request opens its own connection.
pub struct DefaultTransport {
    #[cfg(feature = "rustls")]
    tls: std::sync::Arc<rustls::ClientConfig>,
    #[cfg(all(feature = "native-tls", not(feature = "rustls")))]
    tls: native_tls::TlsConnector,
}

impl fmt::Debug for DefaultTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DefaultTransport")
    }
}

impl DefaultTransport {
    /// Creates the transport.
    pub fn new() -> DefaultTransport {
        DefaultTransport {
            #[cfg(feature = "rustls")]
            tls: rustls_config(),
            #[cfg(all(feature = "native-tls", not(feature = "rustls")))]
            tls: native_tls::TlsConnector::new()
                .expect("the system TLS library could not be loaded"),
        }
    }
}

impl Default for DefaultTransport {
    fn default() -> Self {
        DefaultTransport::new()
    }
}

#[cfg(feature = "rustls")]
fn rustls_config() -> std::sync::Arc<rustls::ClientConfig> {
    let roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    let config = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("the ring provider supports the default TLS versions")
    .with_root_certificates(roots)
    .with_no_client_auth();
    std::sync::Arc::new(config)
}

struct Url {
    secure: bool,
    host: String,
    port: u16,
    target: String,
}

fn parse_url(url: &str) -> Result<Url, TransportError> {
    let bad = || TransportError::Network(format!("not an http or https URL: {}", url), None);
    let (secure, rest) = if let Some(r) = strip_prefix_ignore_case(url, "https://") {
        (true, r)
    } else if let Some(r) = strip_prefix_ignore_case(url, "http://") {
        (false, r)
    } else {
        return Err(bad());
    };
    let end = rest.find(&['/', '?', '#'][..]).unwrap_or(rest.len());
    let authority = &rest[..end];
    let mut target = rest[end..].split('#').next().unwrap_or("").to_string();
    if target.is_empty() || target.starts_with('?') {
        target.insert(0, '/');
    }
    let (host, port) = if let Some(v6) = authority.strip_prefix('[') {
        let close = v6.find(']').ok_or_else(bad)?;
        let port = v6[close + 1..]
            .strip_prefix(':')
            .map(|p| p.parse::<u16>().map_err(|_| bad()))
            .transpose()?;
        (v6[..close].to_string(), port)
    } else {
        match authority.rfind(':') {
            Some(i) => (
                authority[..i].to_string(),
                Some(authority[i + 1..].parse::<u16>().map_err(|_| bad())?),
            ),
            None => (authority.to_string(), None),
        }
    };
    if host.is_empty() {
        return Err(bad());
    }
    Ok(Url {
        secure,
        port: port.unwrap_or(if secure { 443 } else { 80 }),
        host,
        target,
    })
}

fn strip_prefix_ignore_case<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

fn network(message: String, source: io::Error) -> TransportError {
    TransportError::Network(message, Some(Box::new(source)))
}

fn io_error(e: io::Error, timeout: Duration) -> TransportError {
    match e.kind() {
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => {
            TransportError::Timeout(format!("no answer within {:?}", timeout))
        }
        _ => network(e.to_string(), e),
    }
}

/// Reads from a stream with an overall deadline, by setting the socket's read timeout to what is left before each read.
struct Deadline<S> {
    inner: S,
    socket: TcpStream,
    deadline: Instant,
}

impl<S: Read> Read for Deadline<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let left = self.deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "deadline passed"));
        }
        self.socket.set_read_timeout(Some(left))?;
        self.inner.read(buf)
    }
}

impl<S: Write> Write for Deadline<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let left = self.deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "deadline passed"));
        }
        self.socket.set_write_timeout(Some(left))?;
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn connect(url: &Url, deadline: Instant, timeout: Duration) -> Result<TcpStream, TransportError> {
    let addrs: Vec<SocketAddr> = (url.host.as_str(), url.port)
        .to_socket_addrs()
        .map_err(|e| network(format!("could not look up {}: {}", url.host, e), e))?
        .collect();
    let mut last: Option<io::Error> = None;
    for addr in addrs {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        match TcpStream::connect_timeout(&addr, left) {
            Ok(s) => {
                let _ = s.set_nodelay(true);
                return Ok(s);
            }
            Err(e) => last = Some(e),
        }
    }
    match last {
        Some(e) => Err(io_error(e, timeout)),
        None if Instant::now() >= deadline => Err(TransportError::Timeout(format!(
            "no connection within {:?}",
            timeout
        ))),
        None => Err(TransportError::Network(
            format!("no address found for {}", url.host),
            None,
        )),
    }
}

impl Transport for DefaultTransport {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let url = parse_url(&request.url)?;
        let deadline = Instant::now() + request.timeout;
        let tcp = connect(&url, deadline, request.timeout)?;
        let socket = tcp.try_clone().map_err(|e| network(e.to_string(), e))?;
        let head = request_head(request, &url);
        if !url.secure {
            return exchange(
                Deadline {
                    inner: tcp,
                    socket,
                    deadline,
                },
                &head,
                request,
            );
        }
        self.secure(tcp, socket, deadline, &url, &head, request)
    }
}

impl DefaultTransport {
    #[cfg(feature = "rustls")]
    fn secure(
        &self,
        tcp: TcpStream,
        socket: TcpStream,
        deadline: Instant,
        url: &Url,
        head: &str,
        request: &HttpRequest,
    ) -> Result<HttpResponse, TransportError> {
        let name = rustls::pki_types::ServerName::try_from(url.host.clone()).map_err(|e| {
            TransportError::Network(
                format!("not a valid host name: {}", url.host),
                Some(Box::new(e)),
            )
        })?;
        let connection = rustls::ClientConnection::new(self.tls.clone(), name).map_err(|e| {
            TransportError::Network(format!("TLS setup failed: {}", e), Some(Box::new(e)))
        })?;
        exchange(
            Deadline {
                inner: rustls::StreamOwned::new(connection, tcp),
                socket,
                deadline,
            },
            head,
            request,
        )
    }

    #[cfg(all(feature = "native-tls", not(feature = "rustls")))]
    fn secure(
        &self,
        tcp: TcpStream,
        socket: TcpStream,
        deadline: Instant,
        url: &Url,
        head: &str,
        request: &HttpRequest,
    ) -> Result<HttpResponse, TransportError> {
        let _ = socket.set_read_timeout(Some(
            deadline
                .saturating_duration_since(Instant::now())
                .max(Duration::from_millis(1)),
        ));
        let stream = self.tls.connect(&url.host, tcp).map_err(|e| match e {
            native_tls::HandshakeError::Failure(e) => {
                TransportError::Network(format!("TLS handshake failed: {}", e), Some(Box::new(e)))
            }
            native_tls::HandshakeError::WouldBlock(_) => {
                TransportError::Timeout(format!("no TLS handshake within {:?}", request.timeout))
            }
        })?;
        exchange(
            Deadline {
                inner: stream,
                socket,
                deadline,
            },
            head,
            request,
        )
    }

    #[cfg(not(any(feature = "rustls", feature = "native-tls")))]
    fn secure(
        &self,
        _tcp: TcpStream,
        _socket: TcpStream,
        _deadline: Instant,
        _url: &Url,
        _head: &str,
        _request: &HttpRequest,
    ) -> Result<HttpResponse, TransportError> {
        Err(TransportError::Network(
            "HTTPS needs the `rustls` or `native-tls` feature of the mygeocode crate, or your own transport".to_string(),
            None,
        ))
    }
}

fn request_head(request: &HttpRequest, url: &Url) -> String {
    let default_port = if url.secure { 443 } else { 80 };
    let host = if url.host.contains(':') {
        format!("[{}]", url.host)
    } else {
        url.host.clone()
    };
    let mut head = format!(
        "{} {} HTTP/1.1\r\nHost: {}",
        request.method, url.target, host
    );
    if url.port != default_port {
        head.push_str(&format!(":{}", url.port));
    }
    head.push_str("\r\nConnection: close\r\nAccept-Encoding: identity\r\n");
    for (name, value) in &request.headers {
        if name.eq_ignore_ascii_case("host")
            || name.eq_ignore_ascii_case("content-length")
            || name.eq_ignore_ascii_case("connection")
        {
            continue;
        }
        head.push_str(&format!("{}: {}\r\n", name, value));
    }
    if let Some(body) = &request.body {
        head.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    head.push_str("\r\n");
    head
}

fn exchange<S: Read + Write>(
    mut stream: S,
    head: &str,
    request: &HttpRequest,
) -> Result<HttpResponse, TransportError> {
    let timeout = request.timeout;
    stream
        .write_all(head.as_bytes())
        .map_err(|e| io_error(e, timeout))?;
    if let Some(body) = &request.body {
        stream
            .write_all(body.as_bytes())
            .map_err(|e| io_error(e, timeout))?;
    }
    stream.flush().map_err(|e| io_error(e, timeout))?;
    read_response(&mut stream, timeout)
}

fn read_response<S: Read>(
    stream: &mut S,
    timeout: Duration,
) -> Result<HttpResponse, TransportError> {
    let mut buf: Vec<u8> = Vec::with_capacity(8192);
    let mut chunk = [0u8; 8192];
    // Informational answers (1xx) come before the real one: read past them.
    let (status, headers, mut rest) = loop {
        let head_end = loop {
            if let Some(i) = find(&buf, b"\r\n\r\n") {
                break i;
            }
            if buf.len() > 64 * 1024 {
                return Err(TransportError::Network(
                    "the response headers are too long".to_string(),
                    None,
                ));
            }
            let n = read_some(stream, &mut chunk, timeout)?;
            if n == 0 {
                return Err(TransportError::Network(
                    "the connection closed before the response headers".to_string(),
                    None,
                ));
            }
            buf.extend_from_slice(&chunk[..n]);
        };
        let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
        let rest = buf[head_end + 4..].to_vec();
        let mut lines = head.split("\r\n");
        let status_line = lines.next().unwrap_or("");
        let status = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<u16>().ok())
            .filter(|_| status_line.starts_with("HTTP/"))
            .ok_or_else(|| {
                TransportError::Network(format!("not an HTTP response: {}", status_line), None)
            })?;
        if (100..200).contains(&status) {
            buf = rest;
            continue;
        }
        let mut headers: Vec<(String, String)> = Vec::new();
        for line in lines {
            if let Some(i) = line.find(':') {
                let name = line[..i].trim().to_ascii_lowercase();
                let value = line[i + 1..].trim().to_string();
                match headers.iter_mut().find(|(k, _)| *k == name) {
                    Some(slot) => {
                        slot.1.push_str(", ");
                        slot.1.push_str(&value);
                    }
                    None => headers.push((name, value)),
                }
            }
        }
        break (status, headers, rest);
    };
    let header = |name: &str| {
        headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
    };
    let body = if header("transfer-encoding")
        .map_or(false, |v| v.to_ascii_lowercase().contains("chunked"))
    {
        read_chunked(stream, rest, timeout)?
    } else if let Some(length) = header("content-length").and_then(|v| v.parse::<usize>().ok()) {
        while rest.len() < length {
            let n = read_some(stream, &mut chunk, timeout)?;
            if n == 0 {
                return Err(TransportError::Network(
                    "the connection closed before the whole body arrived".to_string(),
                    None,
                ));
            }
            rest.extend_from_slice(&chunk[..n]);
        }
        rest.truncate(length);
        rest
    } else {
        loop {
            let n = read_some(stream, &mut chunk, timeout)?;
            if n == 0 {
                break rest;
            }
            rest.extend_from_slice(&chunk[..n]);
        }
    };
    Ok(HttpResponse {
        status,
        headers,
        body: String::from_utf8_lossy(&body).to_string(),
    })
}

/// One read; a connection the server closed without a TLS close message counts as the end.
fn read_some<S: Read>(
    stream: &mut S,
    buf: &mut [u8],
    timeout: Duration,
) -> Result<usize, TransportError> {
    loop {
        match stream.read(buf) {
            Ok(n) => return Ok(n),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(0),
            Err(e) => return Err(io_error(e, timeout)),
        }
    }
}

fn read_chunked<S: Read>(
    stream: &mut S,
    mut pending: Vec<u8>,
    timeout: Duration,
) -> Result<Vec<u8>, TransportError> {
    let mut body = Vec::new();
    let mut chunk = [0u8; 8192];
    let truncated = || {
        TransportError::Network(
            "the connection closed inside a chunked body".to_string(),
            None,
        )
    };
    loop {
        let line_end = loop {
            if let Some(i) = find(&pending, b"\r\n") {
                break i;
            }
            let n = read_some(stream, &mut chunk, timeout)?;
            if n == 0 {
                return Err(truncated());
            }
            pending.extend_from_slice(&chunk[..n]);
        };
        let size_text = String::from_utf8_lossy(&pending[..line_end]).to_string();
        let size = usize::from_str_radix(size_text.split(';').next().unwrap_or("").trim(), 16)
            .map_err(|_| TransportError::Network(format!("bad chunk size: {}", size_text), None))?;
        pending.drain(..line_end + 2);
        if size == 0 {
            return Ok(body);
        }
        while pending.len() < size + 2 {
            let n = read_some(stream, &mut chunk, timeout)?;
            if n == 0 {
                return Err(truncated());
            }
            pending.extend_from_slice(&chunk[..n]);
        }
        body.extend_from_slice(&pending[..size]);
        pending.drain(..size + 2);
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_are_taken_apart() {
        let u = parse_url("https://api.mygeocode.com/v1/forward?q=a")
            .ok()
            .unwrap();
        assert!(
            u.secure
                && u.host == "api.mygeocode.com"
                && u.port == 443
                && u.target == "/v1/forward?q=a"
        );
        let u = parse_url("http://127.0.0.1:8080").ok().unwrap();
        assert!(!u.secure && u.port == 8080 && u.target == "/");
        let u = parse_url("http://[::1]:9000/x").ok().unwrap();
        assert!(u.host == "::1" && u.port == 9000 && u.target == "/x");
        assert!(parse_url("ftp://x").is_err());
    }

    #[test]
    fn chunked_bodies_are_joined() {
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nX-A: 1\r\n\r\n4\r\nWiki\r\n5;ext=1\r\npedia\r\n0\r\n\r\n";
        let r = read_response(&mut &raw[..], Duration::from_secs(1))
            .ok()
            .unwrap();
        assert_eq!(
            (r.status, r.body.as_str(), r.header("x-a")),
            (200, "Wikipedia", Some("1"))
        );
        let raw = b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 429 Too Many\r\nContent-Length: 2\r\n\r\n{}extra";
        let r = read_response(&mut &raw[..], Duration::from_secs(1))
            .ok()
            .unwrap();
        assert_eq!((r.status, r.body.as_str()), (429, "{}"));
    }
}
