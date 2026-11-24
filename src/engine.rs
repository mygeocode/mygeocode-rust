//! What both clients share: the settings, the requests of the 16 endpoint methods, retries and reading the answer.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error::{ApiError, Error, Result};
use crate::http::{DefaultTransport, HttpRequest, HttpResponse, Transport, TransportError};
use crate::json::Value;
use crate::models::*;
use crate::options::*;

/// The version of this crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The default base URL.
pub const DEFAULT_BASE_URL: &str = "https://api.mygeocode.com/v1";

/// The `User-Agent` the client sends.
pub fn user_agent() -> String {
    format!(
        "mygeocode-rust/{} (+https://www.mygeocode.com/sdks/)",
        VERSION
    )
}

pub(crate) type SleepFn = Arc<dyn Fn(Duration) + Send + Sync>;
pub(crate) type ClockFn = Arc<dyn Fn() -> f64 + Send + Sync>;
type EnvFn = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

pub(crate) struct Engine {
    pub(crate) key: Option<String>,
    pub(crate) base_url: String,
    pub(crate) timeout: Duration,
    pub(crate) max_retries: u32,
    pub(crate) max_retry_wait: Duration,
    pub(crate) lang: String,
    pub(crate) lang_set: bool,
    pub(crate) transport: Arc<dyn Transport>,
    pub(crate) sleep: SleepFn,
    pub(crate) clock: ClockFn,
}

/// Builds a client. Every option is optional.
pub struct ClientBuilder {
    key: Option<String>,
    key_set: bool,
    base_url: String,
    timeout: Duration,
    max_retries: u32,
    max_retry_wait: Duration,
    lang: Option<String>,
    transport: Option<Arc<dyn Transport>>,
    sleep: SleepFn,
    clock: ClockFn,
    environment: EnvFn,
}

impl std::fmt::Debug for ClientBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("base_url", &self.base_url)
            .field("key", &self.key.as_ref().map(|_| "set"))
            .finish()
    }
}

impl Default for ClientBuilder {
    fn default() -> Self {
        ClientBuilder {
            key: None,
            key_set: false,
            base_url: DEFAULT_BASE_URL.to_string(),
            timeout: Duration::from_secs(10),
            max_retries: 2,
            max_retry_wait: Duration::from_secs(60),
            lang: None,
            transport: None,
            sleep: Arc::new(std::thread::sleep),
            clock: Arc::new(|| {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(0.0)
            }),
            environment: Arc::new(|name| std::env::var(name).ok()),
        }
    }
}

impl ClientBuilder {
    /// A builder with every option at its default.
    pub fn new() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// The API key, sent in the `X-API-Key` header (never in the URL). Without this call the client reads the
    /// `MYGEOCODE_KEY` environment variable; with neither, no key is sent. An empty key sends none.
    pub fn key(mut self, key: impl Into<String>) -> ClientBuilder {
        self.key = Some(key.into());
        self.key_set = true;
        self
    }

    /// The base URL (default `https://api.mygeocode.com/v1`).
    pub fn base_url(mut self, url: impl Into<String>) -> ClientBuilder {
        self.base_url = url.into();
        self
    }

    /// How long to wait for each attempt, from connecting to the last byte (default 10 seconds).
    pub fn timeout(mut self, timeout: Duration) -> ClientBuilder {
        self.timeout = timeout;
        self
    }

    /// How many times a request is retried after a 429, 500, 503, timeout or connection error (default 2).
    pub fn max_retries(mut self, max_retries: u32) -> ClientBuilder {
        self.max_retries = max_retries;
        self
    }

    /// The longest wait the client accepts before a retry (default 60 seconds). A longer one, such as a 429 whose
    /// free requests reset hours from now, returns the error at once.
    pub fn max_retry_wait(mut self, wait: Duration) -> ClientBuilder {
        self.max_retry_wait = wait;
        self
    }

    /// The language of the results, an ISO 639-1 code such as `de`, `ja` or `pt-BR` (default `en`). It is sent on
    /// every endpoint that takes one, unless a call sets its own.
    pub fn lang(mut self, lang: impl Into<String>) -> ClientBuilder {
        self.lang = Some(lang.into());
        self
    }

    /// Replaces the HTTP layer (default [`DefaultTransport`]).
    pub fn transport(mut self, transport: impl Transport + 'static) -> ClientBuilder {
        self.transport = Some(Arc::new(transport));
        self
    }

    /// Replaces the wait between retries. For tests.
    pub fn sleep(mut self, sleep: impl Fn(Duration) + Send + Sync + 'static) -> ClientBuilder {
        self.sleep = Arc::new(sleep);
        self
    }

    /// Replaces the clock (Unix time in seconds) used to turn `X-Quota-Reset` into a wait. For tests.
    pub fn clock(mut self, clock: impl Fn() -> f64 + Send + Sync + 'static) -> ClientBuilder {
        self.clock = Arc::new(clock);
        self
    }

    #[doc(hidden)]
    pub fn environment(
        mut self,
        environment: impl Fn(&str) -> Option<String> + Send + Sync + 'static,
    ) -> ClientBuilder {
        self.environment = Arc::new(environment);
        self
    }

    /// Builds the async client.
    pub fn build(self) -> crate::Client {
        crate::Client::from_engine(Arc::new(self.engine()))
    }

    /// Builds the blocking client.
    pub fn build_blocking(self) -> crate::blocking::Client {
        crate::blocking::Client::from_engine(Arc::new(self.engine()))
    }

    fn engine(self) -> Engine {
        let key = if self.key_set {
            self.key
        } else {
            (self.environment)("MYGEOCODE_KEY")
        };
        let key = key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty());
        Engine {
            key,
            base_url: self.base_url.trim().trim_end_matches('/').to_string(),
            timeout: if self.timeout.is_zero() {
                Duration::from_secs(10)
            } else {
                self.timeout
            },
            max_retries: self.max_retries,
            max_retry_wait: self.max_retry_wait,
            lang_set: self.lang.is_some(),
            lang: self.lang.unwrap_or_else(|| "en".to_string()),
            transport: self
                .transport
                .unwrap_or_else(|| Arc::new(DefaultTransport::new())),
            sleep: self.sleep,
            clock: self.clock,
        }
    }
}

type Query = Vec<(String, String)>;

fn put(q: &mut Query, name: &str, value: Option<String>) {
    q.retain(|(k, _)| k != name);
    if let Some(v) = value.filter(|v| !v.is_empty()) {
        q.push((name.to_string(), v));
    }
}

fn num(v: f64) -> Option<String> {
    Some(crate::json::format_number(v))
}

impl Engine {
    /// The options, then the client's language unless the call set one (endpoints that document `lang`).
    fn with_lang(&self, mut q: Query, options: &Values) -> (Query, Vec<(String, String)>) {
        for (k, v) in &options.values {
            put(&mut q, k, Some(v.clone()));
        }
        if options.get("lang").is_none() {
            put(&mut q, "lang", Some(self.lang.clone()));
        }
        (q, options.params.clone())
    }

    /// Timezone names its parameter `language`; it is sent only when the call or the client chose one.
    fn with_language(&self, mut q: Query, options: &Values) -> (Query, Vec<(String, String)>) {
        for (k, v) in &options.values {
            if *k != "lang" && *k != "language" {
                put(&mut q, k, Some(v.clone()));
            }
        }
        let chosen = options
            .get("lang")
            .or_else(|| options.get("language"))
            .map(|s| s.to_string());
        let chosen = chosen.or_else(|| {
            if self.lang_set {
                Some(self.lang.clone())
            } else {
                None
            }
        });
        put(&mut q, "language", chosen);
        (q, options.params.clone())
    }

    fn plain(&self, mut q: Query, options: &Values) -> (Query, Vec<(String, String)>) {
        for (k, v) in &options.values {
            put(&mut q, k, Some(v.clone()));
        }
        (q, options.params.clone())
    }

    pub(crate) fn forward(&self, query: &str, o: &ForwardOptions) -> Result<ForwardResponse> {
        let mut q = Query::new();
        put(&mut q, "q", Some(query.to_string()));
        let (q, extra) = self.with_lang(q, o.values());
        self.get("/forward", q, extra, ForwardResponse::from_value, |r, m| {
            r.meta = m
        })
    }

    pub(crate) fn reverse(
        &self,
        lat: f64,
        lon: f64,
        o: &ReverseOptions,
    ) -> Result<ReverseResponse> {
        let mut q = Query::new();
        put(&mut q, "lat", num(lat));
        put(&mut q, "lon", num(lon));
        let (q, extra) = self.with_lang(q, o.values());
        self.get("/reverse", q, extra, ReverseResponse::from_value, |r, m| {
            r.meta = m
        })
    }

    pub(crate) fn autocomplete(
        &self,
        query: &str,
        o: &AutocompleteOptions,
    ) -> Result<AutocompleteResponse> {
        let mut q = Query::new();
        put(&mut q, "q", Some(query.to_string()));
        let (q, extra) = self.with_lang(q, o.values());
        self.get(
            "/autocomplete",
            q,
            extra,
            AutocompleteResponse::from_value,
            |r, m| r.meta = m,
        )
    }

    pub(crate) fn ip(&self, path: &str, address: Option<&str>, o: &IpOptions) -> Result<IpInfo> {
        let mut q = Query::new();
        put(&mut q, "ip", address.map(|a| a.to_string()));
        let (q, extra) = self.plain(q, o.values());
        self.get(path, q, extra, IpInfo::from_value, |r, m| r.meta = m)
    }

    pub(crate) fn timezone(
        &self,
        point: Option<(f64, f64)>,
        o: &TimezoneOptions,
    ) -> Result<TimezoneInfo> {
        let mut q = Query::new();
        if let Some((lat, lon)) = point {
            put(&mut q, "lat", num(lat));
            put(&mut q, "lon", num(lon));
        }
        let (q, extra) = self.with_language(q, o.values());
        self.get("/timezone", q, extra, TimezoneInfo::from_value, |r, m| {
            r.meta = m
        })
    }

    pub(crate) fn elevation(
        &self,
        point: Option<(f64, f64)>,
        o: &ElevationOptions,
    ) -> Result<ElevationResponse> {
        let mut q = Query::new();
        if let Some((lat, lon)) = point {
            put(&mut q, "lat", num(lat));
            put(&mut q, "lon", num(lon));
        }
        let (q, extra) = self.plain(q, o.values());
        self.get(
            "/elevation",
            q,
            extra,
            ElevationResponse::from_value,
            |r, m| r.meta = m,
        )
    }

    pub(crate) fn postcode(&self, code: &str, o: &PostcodeOptions) -> Result<PostcodeResponse> {
        let mut q = Query::new();
        put(&mut q, "code", Some(code.to_string()));
        let (q, extra) = self.plain(q, o.values());
        self.get(
            "/postcode",
            q,
            extra,
            PostcodeResponse::from_value,
            |r, m| r.meta = m,
        )
    }

    pub(crate) fn forward_batch(
        &self,
        queries: Vec<String>,
        o: &ForwardBatchOptions,
    ) -> Result<ForwardBatchResponse> {
        let body = Value::Object(vec![(
            "queries".to_string(),
            Value::Array(queries.into_iter().map(Value::String).collect()),
        )]);
        let (q, extra) = self.with_lang(Query::new(), o.values());
        self.send(
            "POST",
            "/forward",
            q,
            extra,
            Some(body.to_json()),
            ForwardBatchResponse::from_value,
            |r, m| r.meta = m,
        )
    }

    pub(crate) fn reverse_batch(
        &self,
        points: Vec<Point>,
        o: &ReverseOptions,
    ) -> Result<ReverseBatchResponse> {
        let body = Value::Object(vec![("coords".to_string(), pairs(&points))]);
        let (q, extra) = self.with_lang(Query::new(), o.values());
        self.send(
            "POST",
            "/reverse",
            q,
            extra,
            Some(body.to_json()),
            ReverseBatchResponse::from_value,
            |r, m| r.meta = m,
        )
    }

    pub(crate) fn ip_batch(
        &self,
        path: &str,
        addresses: Vec<String>,
        o: &IpOptions,
    ) -> Result<IpBatchResponse> {
        let (q, extra) = self.plain(vec![("ips".to_string(), addresses.join(","))], o.values());
        self.get(path, q, extra, IpBatchResponse::from_value, |r, m| {
            r.meta = m
        })
    }

    pub(crate) fn timezone_batch(
        &self,
        locations: String,
        o: &TimezoneBatchOptions,
    ) -> Result<TimezoneBatchResponse> {
        let (q, extra) = self.with_language(vec![("locations".to_string(), locations)], o.values());
        self.get(
            "/timezone",
            q,
            extra,
            TimezoneBatchResponse::from_value,
            |r, m| r.meta = m,
        )
    }

    pub(crate) fn elevation_batch(
        &self,
        points: Vec<Point>,
        o: &ElevationBatchOptions,
    ) -> Result<ElevationResponse> {
        let body = Value::Object(vec![("locations".to_string(), pairs(&points))]);
        let (q, extra) = self.plain(Query::new(), o.values());
        self.send(
            "POST",
            "/elevation",
            q,
            extra,
            Some(body.to_json()),
            ElevationResponse::from_value,
            |r, m| r.meta = m,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn get<T>(
        &self,
        path: &str,
        q: Query,
        extra: Vec<(String, String)>,
        read: fn(&Value) -> T,
        meta: fn(&mut T, Meta),
    ) -> Result<T> {
        self.send("GET", path, q, extra, None, read, meta)
    }

    #[allow(clippy::too_many_arguments)]
    fn send<T>(
        &self,
        method: &str,
        path: &str,
        mut q: Query,
        extra: Vec<(String, String)>,
        body: Option<String>,
        read: fn(&Value) -> T,
        meta: fn(&mut T, Meta),
    ) -> Result<T> {
        for (k, v) in extra {
            q.retain(|(name, _)| *name != k);
            q.push((k, v));
        }
        let query = q
            .iter()
            .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        let mut headers = vec![
            ("Accept".to_string(), "application/json".to_string()),
            ("User-Agent".to_string(), user_agent()),
        ];
        if let Some(key) = &self.key {
            headers.push(("X-API-Key".to_string(), key.clone()));
        }
        if body.is_some() {
            headers.push(("Content-Type".to_string(), "application/json".to_string()));
        }
        let url = if query.is_empty() {
            format!("{}{}", self.base_url, path)
        } else {
            format!("{}{}?{}", self.base_url, path, query)
        };
        let request = HttpRequest {
            method: method.to_string(),
            url,
            headers,
            body,
            timeout: self.timeout,
        };

        let mut retried_500 = false;
        let mut attempt: u32 = 0;
        loop {
            let response = match self.transport.send(&request) {
                Ok(r) => r,
                Err(e) => {
                    if method == "GET" && attempt < self.max_retries {
                        (self.sleep)(backoff(attempt));
                        attempt += 1;
                        continue;
                    }
                    return Err(match e {
                        TransportError::Timeout(m) => Error::RequestTimeout(m),
                        TransportError::Network(m, source) => Error::Network(m, source),
                    });
                }
            };
            if matches!(response.status, 429 | 500 | 503) {
                if let Some(wait) = self.retry_wait(&response, attempt, retried_500) {
                    retried_500 = retried_500 || response.status == 500;
                    (self.sleep)(wait);
                    attempt += 1;
                    continue;
                }
            }
            return self.parse(&response, path, read, meta);
        }
    }

    /// How long to wait before trying again, or `None` to return the error now.
    fn retry_wait(
        &self,
        response: &HttpResponse,
        attempt: u32,
        retried_500: bool,
    ) -> Option<Duration> {
        if attempt >= self.max_retries {
            return None;
        }
        if response.status == 500 {
            return if retried_500 {
                None
            } else {
                Some(Duration::from_secs(1))
            };
        }
        let mut wait = self.server_wait(response);
        if response.status == 503 && wait.is_none() {
            wait = Some(backoff(attempt));
        }
        wait.filter(|w| *w <= self.max_retry_wait)
    }

    /// The wait the server asks for: `Retry-After`, or on a 429 the time until `X-Quota-Reset`.
    fn server_wait(&self, response: &HttpResponse) -> Option<Duration> {
        let seconds = |name: &str| {
            response
                .header(name)
                .and_then(|v| v.trim().parse::<f64>().ok())
                .filter(|f| f.is_finite())
        };
        if let Some(after) = seconds("retry-after") {
            return Some(Duration::from_millis(
                (after.max(0.0) * 1000.0).round() as u64
            ));
        }
        if response.status == 429 {
            if let Some(reset) = seconds("x-quota-reset") {
                return Some(Duration::from_millis(
                    ((reset - (self.clock)()).max(0.0) * 1000.0).round() as u64,
                ));
            }
        }
        None
    }

    fn parse<T>(
        &self,
        response: &HttpResponse,
        path: &str,
        read: fn(&Value) -> T,
        meta: fn(&mut T, Meta),
    ) -> Result<T> {
        let parsed = Value::parse(&response.body).ok();
        let object = parsed.filter(|v| v.as_object().is_some());
        if response.status >= 400
            || object
                .as_ref()
                .map_or(false, |o| o.get("status").as_str() == Some("error"))
        {
            return Err(Error::Api(Box::new(self.api_error(response, object))));
        }
        let mut object = match object {
            Some(o) => o,
            None => {
                return Err(Error::Decode(format!(
                    "expected a JSON object from the API, got HTTP {} with a body that is not one",
                    response.status
                )))
            }
        };
        if path == "/elevation"
            && object.get("results").is_null()
            && !object.get("elevation_m").is_null()
        {
            let single = object.clone();
            if let Value::Object(fields) = &mut object {
                fields.retain(|(k, _)| k != "results");
                fields.push(("results".to_string(), Value::Array(vec![single])));
            }
        }
        let mut result = read(&object);
        meta(&mut result, Meta::from_response(response));
        Ok(result)
    }

    fn api_error(&self, response: &HttpResponse, object: Option<Value>) -> ApiError {
        let body = object.unwrap_or(Value::Null);
        let error = body.get("error");
        let text = |name: &str| {
            error
                .get(name)
                .as_str()
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        };
        ApiError {
            status: response.status,
            code: text("code").unwrap_or_else(|| "http_error".to_string()),
            message: text("message").unwrap_or_else(|| format!("HTTP {}", response.status)),
            param: error.get("param").as_str().map(|s| s.to_string()),
            request_id: response
                .header("x-request-id")
                .filter(|v| !v.is_empty())
                .map(|v| v.to_string()),
            retry_after: self.server_wait(response),
            body,
        }
    }
}

fn backoff(attempt: u32) -> Duration {
    Duration::from_secs(1u64 << attempt.min(20))
}

fn pairs(points: &[Point]) -> Value {
    Value::Array(
        points
            .iter()
            .map(|p| {
                Value::Array(vec![
                    Value::Number(crate::json::Number::Float(p.lat)),
                    Value::Number(crate::json::Number::Float(p.lon)),
                ])
            })
            .collect(),
    )
}

/// Query-string encoding: letters, digits and `_.-~` as they are, a space as `+`, everything else as `%XX`.
fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
