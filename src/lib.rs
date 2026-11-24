//! The official Rust client for the [My Geocode API](https://www.mygeocode.com/): forward and reverse geocoding,
//! address autocomplete, IPv4 and IPv6 lookup, timezones, elevation and postal codes.
//!
//! [`Client`] is async and works with any runtime (each call runs on its own thread, so there is no runtime to pick).
//! [`blocking::Client`] has the same methods and returns when the answer is in. Both are cheap to clone and safe to
//! share between threads.
//!
//! ```no_run
//! # async fn example() -> mygeocode::Result<()> {
//! use mygeocode::{Client, ReverseOptions};
//!
//! let client = Client::new();
//! let address = client.reverse(48.8584, 2.2945, &ReverseOptions::new()).await?;
//! println!("{:?}", address.result.and_then(|p| p.formatted));
//! # Ok(())
//! # }
//! ```
//!
//! A key is optional: without one, each network gets 2,500 free requests a day. Results are in English unless you
//! set `lang`.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod blocking;
mod engine;
mod error;
mod http;
mod json;
mod models;
mod options;

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use engine::Engine;
pub use engine::{user_agent, ClientBuilder, DEFAULT_BASE_URL, VERSION};
pub use error::{ApiError, Error, MyGeocodeError, Result};
pub use http::{DefaultTransport, HttpRequest, HttpResponse, Transport, TransportError};
pub use json::{Number, ParseError, Value};
pub use models::*;
pub use options::*;

/// The async client for the My Geocode API. Every method returns a [`Pending`] future that works under any runtime
/// (tokio, async-std, smol or a plain `block_on`): the request runs on a thread of its own, so it never blocks the
/// executor. The client is cheap to clone and safe to share between threads.
#[derive(Clone)]
pub struct Client {
    engine: Arc<Engine>,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("base_url", &self.engine.base_url)
            .field("key", &self.engine.key.as_ref().map(|_| "set"))
            .finish()
    }
}

impl Default for Client {
    fn default() -> Self {
        Client::new()
    }
}

impl Client {
    /// A client with every option at its default (the key from `MYGEOCODE_KEY`, if set).
    pub fn new() -> Client {
        ClientBuilder::new().build()
    }

    /// A builder for the client options; finish it with [`ClientBuilder::build`].
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    pub(crate) fn from_engine(engine: Arc<Engine>) -> Client {
        Client { engine }
    }

    /// Free text, a structured address or a place id to coordinates. For a structured search pass `""` as the query and set `street`, `city` and the rest in the options.
    pub fn forward(&self, query: &str, options: &ForwardOptions) -> Pending<ForwardResponse> {
        let query = query.to_string();
        let options = options.clone();
        run(&self.engine, move |e| e.forward(&query, &options))
    }

    /// A point to the nearest address. `result` is `None` when nothing is in range.
    pub fn reverse(
        &self,
        lat: f64,
        lon: f64,
        options: &ReverseOptions,
    ) -> Pending<ReverseResponse> {
        let options = options.clone();
        run(&self.engine, move |e| e.reverse(lat, lon, &options))
    }

    /// Suggestions for what someone has typed so far (2 to 128 characters).
    pub fn autocomplete(
        &self,
        query: &str,
        options: &AutocompleteOptions,
    ) -> Pending<AutocompleteResponse> {
        let query = query.to_string();
        let options = options.clone();
        run(&self.engine, move |e| e.autocomplete(&query, &options))
    }

    /// An IPv4 or IPv6 address; `None` for the address the request comes from.
    pub fn ip(&self, address: Option<&str>, options: &IpOptions) -> Pending<IpInfo> {
        let address = address.map(|a| a.to_string());
        let options = options.clone();
        run(&self.engine, move |e| {
            e.ip("/ip", address.as_deref(), &options)
        })
    }

    /// An IPv4 address; `None` for the address the request comes from.
    pub fn ipv4(&self, address: Option<&str>, options: &IpOptions) -> Pending<IpInfo> {
        let address = address.map(|a| a.to_string());
        let options = options.clone();
        run(&self.engine, move |e| {
            e.ip("/ipv4", address.as_deref(), &options)
        })
    }

    /// An IPv6 address; `None` for the address the request comes from.
    pub fn ipv6(&self, address: Option<&str>, options: &IpOptions) -> Pending<IpInfo> {
        let address = address.map(|a| a.to_string());
        let options = options.clone();
        run(&self.engine, move |e| {
            e.ip("/ipv6", address.as_deref(), &options)
        })
    }

    /// Zone, offset and local time of a point, now or at `timestamp`.
    pub fn timezone(&self, lat: f64, lon: f64, options: &TimezoneOptions) -> Pending<TimezoneInfo> {
        let options = options.clone();
        run(&self.engine, move |e| {
            e.timezone(Some((lat, lon)), &options)
        })
    }

    /// A timezone without a single point: a zone id (`tz`), or several points (`locations`, `path`, `bounds`). For several points the answer also has them in `raw`; `timezone_batch` reads them for you.
    pub fn timezone_with(&self, options: &TimezoneOptions) -> Pending<TimezoneInfo> {
        let options = options.clone();
        run(&self.engine, move |e| e.timezone(None, &options))
    }

    /// Height of one point. The answer has one entry in `results`.
    pub fn elevation(
        &self,
        lat: f64,
        lon: f64,
        options: &ElevationOptions,
    ) -> Pending<ElevationResponse> {
        let options = options.clone();
        run(&self.engine, move |e| {
            e.elevation(Some((lat, lon)), &options)
        })
    }

    /// Heights without a single point: several points (`locations`), a path (`path` with `samples`) or a grid (`bounds` with `rows` and `cols`).
    pub fn elevation_with(&self, options: &ElevationOptions) -> Pending<ElevationResponse> {
        let options = options.clone();
        run(&self.engine, move |e| e.elevation(None, &options))
    }

    /// A postal code to every place that uses it. `country` in the options narrows it to one country.
    pub fn postcode(&self, code: &str, options: &PostcodeOptions) -> Pending<PostcodeResponse> {
        let code = code.to_string();
        let options = options.clone();
        run(&self.engine, move |e| e.postcode(&code, &options))
    }

    /// Up to 100 searches in one request (each counts as one). The options apply to every search.
    pub fn forward_batch<S: AsRef<str>>(
        &self,
        queries: &[S],
        options: &ForwardBatchOptions,
    ) -> Pending<ForwardBatchResponse> {
        let queries: Vec<String> = queries.iter().map(|s| s.as_ref().to_string()).collect();
        let options = options.clone();
        run(&self.engine, move |e| e.forward_batch(queries, &options))
    }

    /// Up to 100 points in one request (each counts as one). The options apply to every point.
    pub fn reverse_batch<P: Into<Point> + Copy>(
        &self,
        points: &[P],
        options: &ReverseOptions,
    ) -> Pending<ReverseBatchResponse> {
        let points: Vec<Point> = points.iter().map(|p| (*p).into()).collect();
        let options = options.clone();
        run(&self.engine, move |e| e.reverse_batch(points, &options))
    }

    /// Up to 100 IPv4 and IPv6 addresses, mixed, in one request.
    pub fn ip_batch<S: AsRef<str>>(
        &self,
        addresses: &[S],
        options: &IpOptions,
    ) -> Pending<IpBatchResponse> {
        let addresses: Vec<String> = addresses.iter().map(|s| s.as_ref().to_string()).collect();
        let options = options.clone();
        run(&self.engine, move |e| {
            e.ip_batch("/ip", addresses, &options)
        })
    }

    /// Up to 100 IPv4 addresses in one request.
    pub fn ipv4_batch<S: AsRef<str>>(
        &self,
        addresses: &[S],
        options: &IpOptions,
    ) -> Pending<IpBatchResponse> {
        let addresses: Vec<String> = addresses.iter().map(|s| s.as_ref().to_string()).collect();
        let options = options.clone();
        run(&self.engine, move |e| {
            e.ip_batch("/ipv4", addresses, &options)
        })
    }

    /// Up to 100 IPv6 addresses in one request.
    pub fn ipv6_batch<S: AsRef<str>>(
        &self,
        addresses: &[S],
        options: &IpOptions,
    ) -> Pending<IpBatchResponse> {
        let addresses: Vec<String> = addresses.iter().map(|s| s.as_ref().to_string()).collect();
        let options = options.clone();
        run(&self.engine, move |e| {
            e.ip_batch("/ipv6", addresses, &options)
        })
    }

    /// Timezones of up to 100 points.
    pub fn timezone_batch<P: Into<Point> + Copy>(
        &self,
        points: &[P],
        options: &TimezoneBatchOptions,
    ) -> Pending<TimezoneBatchResponse> {
        let locations = crate::options::points(points);
        let options = options.clone();
        run(&self.engine, move |e| e.timezone_batch(locations, &options))
    }

    /// Timezones along an encoded polyline (`"enc:..."`) or a `"lat,lon|lat,lon"` list.
    pub fn timezone_batch_text(
        &self,
        locations: &str,
        options: &TimezoneBatchOptions,
    ) -> Pending<TimezoneBatchResponse> {
        let locations = locations.to_string();
        let options = options.clone();
        run(&self.engine, move |e| e.timezone_batch(locations, &options))
    }

    /// Heights of up to 100 points, sent as a POST body.
    pub fn elevation_batch<P: Into<Point> + Copy>(
        &self,
        points: &[P],
        options: &ElevationBatchOptions,
    ) -> Pending<ElevationResponse> {
        let points: Vec<Point> = points.iter().map(|p| (*p).into()).collect();
        let options = options.clone();
        run(&self.engine, move |e| e.elevation_batch(points, &options))
    }
}

struct Shared<T> {
    result: Option<Result<T>>,
    waker: Option<Waker>,
}

type Start<T> = Box<dyn FnOnce() -> Result<T> + Send>;

/// The future of one call of the async [`Client`]. It owns everything the call needs, so it can be stored, moved to
/// another task or awaited later. The request starts on the first poll and runs on a thread of its own, so it never
/// blocks the executor. Dropping it before the answer is in lets the request finish and throws the answer away.
#[must_use = "a request is sent only when the future is awaited"]
pub struct Pending<T> {
    start: Option<Start<T>>,
    shared: Arc<Mutex<Shared<T>>>,
}

impl<T> std::fmt::Debug for Pending<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.start.is_some() {
            "Pending(not started)"
        } else {
            "Pending(running)"
        })
    }
}

impl<T: Send + 'static> Future for Pending<T> {
    type Output = Result<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<T>> {
        let this = self.get_mut();
        if let Some(start) = this.start.take() {
            this.shared.lock().unwrap_or_else(|e| e.into_inner()).waker = Some(cx.waker().clone());
            let done = Arc::clone(&this.shared);
            let started = std::thread::Builder::new()
                .name("mygeocode-request".to_string())
                .spawn(move || {
                    let result = start();
                    let mut shared = done.lock().unwrap_or_else(|e| e.into_inner());
                    shared.result = Some(result);
                    if let Some(waker) = shared.waker.take() {
                        waker.wake();
                    }
                });
            if let Err(e) = started {
                return Poll::Ready(Err(Error::Network(
                    format!("could not start a thread for the request: {}", e),
                    Some(Box::new(e)),
                )));
            }
        }
        let mut shared = this.shared.lock().unwrap_or_else(|e| e.into_inner());
        match shared.result.take() {
            Some(result) => Poll::Ready(result),
            None => {
                shared.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

fn run<T, F>(engine: &Arc<Engine>, call: F) -> Pending<T>
where
    T: Send + 'static,
    F: FnOnce(&Engine) -> Result<T> + Send + 'static,
{
    let engine = Arc::clone(engine);
    Pending {
        start: Some(Box::new(move || call(&engine))),
        shared: Arc::new(Mutex::new(Shared {
            result: None,
            waker: None,
        })),
    }
}
