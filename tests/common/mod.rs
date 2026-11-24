//! Shared test helpers: the recorded answers in tests/fixtures, a recording transport, and request checks.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;

use mygeocode::{
    ClientBuilder, HttpRequest, HttpResponse, Point, Transport, TransportError, Value,
};

pub const EIFFEL: Point = Point {
    lat: 48.8584,
    lon: 2.2945,
};
pub const LIBERTY: Point = Point {
    lat: 40.6892,
    lon: -74.0445,
};
pub const OPERA: Point = Point {
    lat: -33.8568,
    lon: 151.2153,
};

/// A recorded answer: {"request": {...}, "status": ..., "headers": {...}, "body": {...}}.
pub fn recording(name: &str) -> Value {
    let path = format!(
        "{}/tests/fixtures/{}.json",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    Value::parse(&std::fs::read_to_string(&path).expect("recording")).expect("recording JSON")
}

pub fn body(name: &str) -> Value {
    recording(name).get("body").clone()
}

pub fn answer(status: u16, body: &Value, headers: &[(&str, &str)]) -> HttpResponse {
    HttpResponse::new(
        status,
        headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        body.to_json(),
    )
}

pub fn text(status: u16, body: &str) -> HttpResponse {
    HttpResponse::new(status, vec![], body)
}

pub fn replay(name: &str) -> HttpResponse {
    let rec = recording(name);
    let headers = rec
        .get("headers")
        .as_object()
        .unwrap_or(&[])
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
        .collect();
    HttpResponse::new(
        rec.get("status").as_i64().unwrap() as u16,
        headers,
        rec.get("body").to_json(),
    )
}

pub fn error_body(code: &str) -> Value {
    Value::parse(&format!(
        r#"{{"status":"error","error":{{"code":"{}","message":"x"}}}}"#,
        code
    ))
    .unwrap()
}

pub fn json(text: &str) -> Value {
    Value::parse(text).unwrap()
}

/// A request as the transport received it, with the URL taken apart.
#[derive(Clone, Debug)]
pub struct Sent {
    pub method: String,
    pub url: String,
    pub path: String,
    pub query: BTreeMap<String, String>,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
    pub timeout: Duration,
}

impl Sent {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn json(&self) -> Value {
        self.body
            .as_deref()
            .map(|b| Value::parse(b).unwrap())
            .unwrap_or(Value::Null)
    }
}

pub fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' => {
                out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
                i += 2;
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8(out).unwrap()
}

fn sent(r: &HttpRequest) -> Sent {
    let (before, query) = match r.url.find('?') {
        Some(i) => (&r.url[..i], &r.url[i + 1..]),
        None => (&r.url[..], ""),
    };
    let path = match before.find("://") {
        Some(i) => before[i + 3..]
            .find('/')
            .map(|j| before[i + 3 + j..].to_string())
            .unwrap_or_default(),
        None => before.to_string(),
    };
    let mut q = BTreeMap::new();
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let mut it = pair.splitn(2, '=');
        q.insert(decode(it.next().unwrap()), decode(it.next().unwrap_or("")));
    }
    Sent {
        method: r.method.clone(),
        url: r.url.clone(),
        path,
        query: q,
        headers: r.headers.clone(),
        body: r.body.clone(),
        timeout: r.timeout,
    }
}

/// What the recorder answers with.
#[derive(Clone)]
pub enum Item {
    Answer(HttpResponse),
    Timeout,
    Refused,
}

/// Collects requests and answers with the queued items in turn (the last one repeats).
#[derive(Clone)]
pub struct Recorder {
    pub sent: Arc<Mutex<Vec<Sent>>>,
    queue: Arc<Mutex<Vec<Item>>>,
}

impl Recorder {
    pub fn new(items: Vec<Item>) -> Recorder {
        Recorder {
            sent: Arc::new(Mutex::new(Vec::new())),
            queue: Arc::new(Mutex::new(items)),
        }
    }

    pub fn of(response: HttpResponse) -> Recorder {
        Recorder::new(vec![Item::Answer(response)])
    }

    pub fn last(&self) -> Sent {
        self.sent
            .lock()
            .unwrap()
            .last()
            .cloned()
            .expect("a request was sent")
    }

    pub fn count(&self) -> usize {
        self.sent.lock().unwrap().len()
    }
}

impl Transport for Recorder {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.sent.lock().unwrap().push(sent(request));
        let mut queue = self.queue.lock().unwrap();
        let item = if queue.len() > 1 {
            queue.remove(0)
        } else {
            queue[0].clone()
        };
        match item {
            Item::Answer(r) => Ok(r),
            Item::Timeout => Err(TransportError::Timeout("slow".to_string())),
            Item::Refused => Err(TransportError::Network("refused".to_string(), None)),
        }
    }
}

/// A client builder with the recorder, no real waits, the clock at Unix time 1000 and no environment key.
pub struct Setup {
    pub sleeps: Arc<Mutex<Vec<f64>>>,
}

impl Setup {
    pub fn new() -> Setup {
        Setup {
            sleeps: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn builder(&self, rec: &Recorder) -> ClientBuilder {
        let sleeps = self.sleeps.clone();
        ClientBuilder::new()
            .transport(rec.clone())
            .sleep(move |d: Duration| sleeps.lock().unwrap().push(d.as_secs_f64()))
            .clock(|| 1000.0)
            .environment(|_| None)
    }

    pub fn client(&self, rec: &Recorder) -> mygeocode::blocking::Client {
        self.builder(rec).build_blocking()
    }

    pub fn sleeps(&self) -> Vec<f64> {
        self.sleeps.lock().unwrap().clone()
    }
}

fn same(a: &str, b: &str) -> bool {
    a == b || matches!((a.parse::<f64>(), b.parse::<f64>()), (Ok(x), Ok(y)) if x == y)
}

/// The request equals the recorded one: method, path, every query parameter and the JSON body.
pub fn assert_sent_like(name: &str, sent: &Sent) {
    let want = recording(name);
    let want = want.get("request");
    assert_eq!(
        Some(sent.method.as_str()),
        want.get("method").as_str(),
        "{} method",
        name
    );
    assert_eq!(
        Some(sent.path.as_str()),
        want.get("path").as_str(),
        "{} path",
        name
    );
    let fields = want.get("query").as_object().unwrap_or(&[]);
    let names: Vec<&str> = fields.iter().map(|(k, _)| k.as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(
        sent.query.keys().map(|k| k.as_str()).collect::<Vec<_>>(),
        sorted,
        "{} query names",
        name
    );
    for (k, v) in fields {
        let expected = v
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| v.to_json());
        assert!(
            same(&sent.query[k], &expected),
            "{}: {}: {} != {}",
            name,
            k,
            sent.query[k],
            expected
        );
    }
    assert_eq!(sent.json(), want.get("body").clone(), "{} body", name);
}

/// A minimal executor: the async client needs no runtime, so the tests use none.
pub fn block_on<F: Future>(future: F) -> F::Output {
    struct Unpark(std::thread::Thread);
    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let mut future = Box::pin(future);
    let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(out) = future.as_mut().poll(&mut cx) {
            return out;
        }
        std::thread::park();
    }
}
