//! The blocking client: the same methods as the async [`Client`](crate::Client), each one returning when the
//! answer is in.

use std::sync::Arc;

use crate::engine::{ClientBuilder, Engine};
use crate::error::Result;
use crate::models::*;
use crate::options::*;

/// The blocking client for the My Geocode API. It is cheap to clone and safe to share between threads.
///
/// ```no_run
/// use mygeocode::blocking::Client;
/// use mygeocode::ForwardOptions;
///
/// let client = Client::new();
/// let found = client.forward("Brandenburg Gate, Berlin", &ForwardOptions::new())?;
/// println!("{:?}", found.results[0].formatted);
/// # Ok::<(), mygeocode::Error>(())
/// ```
#[derive(Clone)]
pub struct Client {
    engine: Arc<Engine>,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("blocking::Client")
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
        ClientBuilder::new().build_blocking()
    }

    /// A builder for the client options; finish it with [`ClientBuilder::build_blocking`].
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    pub(crate) fn from_engine(engine: Arc<Engine>) -> Client {
        Client { engine }
    }

    /// Free text, a structured address or a place id to coordinates. For a structured search pass `""` as the query and set `street`, `city` and the rest in the options.
    pub fn forward(&self, query: &str, options: &ForwardOptions) -> Result<ForwardResponse> {
        self.engine.forward(query, options)
    }

    /// A point to the nearest address. `result` is `None` when nothing is in range.
    pub fn reverse(&self, lat: f64, lon: f64, options: &ReverseOptions) -> Result<ReverseResponse> {
        self.engine.reverse(lat, lon, options)
    }

    /// Suggestions for what someone has typed so far (2 to 128 characters).
    pub fn autocomplete(
        &self,
        query: &str,
        options: &AutocompleteOptions,
    ) -> Result<AutocompleteResponse> {
        self.engine.autocomplete(query, options)
    }

    /// An IPv4 or IPv6 address; `None` for the address the request comes from.
    pub fn ip(&self, address: Option<&str>, options: &IpOptions) -> Result<IpInfo> {
        self.engine.ip("/ip", address, options)
    }

    /// An IPv4 address; `None` for the address the request comes from.
    pub fn ipv4(&self, address: Option<&str>, options: &IpOptions) -> Result<IpInfo> {
        self.engine.ip("/ipv4", address, options)
    }

    /// An IPv6 address; `None` for the address the request comes from.
    pub fn ipv6(&self, address: Option<&str>, options: &IpOptions) -> Result<IpInfo> {
        self.engine.ip("/ipv6", address, options)
    }

    /// Zone, offset and local time of a point, now or at `timestamp`.
    pub fn timezone(&self, lat: f64, lon: f64, options: &TimezoneOptions) -> Result<TimezoneInfo> {
        self.engine.timezone(Some((lat, lon)), options)
    }

    /// A timezone without a single point: a zone id (`tz`), or several points (`locations`, `path`, `bounds`). For several points the answer also has them in `raw`; `timezone_batch` reads them for you.
    pub fn timezone_with(&self, options: &TimezoneOptions) -> Result<TimezoneInfo> {
        self.engine.timezone(None, options)
    }

    /// Height of one point. The answer has one entry in `results`.
    pub fn elevation(
        &self,
        lat: f64,
        lon: f64,
        options: &ElevationOptions,
    ) -> Result<ElevationResponse> {
        self.engine.elevation(Some((lat, lon)), options)
    }

    /// Heights without a single point: several points (`locations`), a path (`path` with `samples`) or a grid (`bounds` with `rows` and `cols`).
    pub fn elevation_with(&self, options: &ElevationOptions) -> Result<ElevationResponse> {
        self.engine.elevation(None, options)
    }

    /// A postal code to every place that uses it. `country` in the options narrows it to one country.
    pub fn postcode(&self, code: &str, options: &PostcodeOptions) -> Result<PostcodeResponse> {
        self.engine.postcode(code, options)
    }

    /// Up to 100 searches in one request (each counts as one). The options apply to every search.
    pub fn forward_batch<S: AsRef<str>>(
        &self,
        queries: &[S],
        options: &ForwardBatchOptions,
    ) -> Result<ForwardBatchResponse> {
        self.engine.forward_batch(
            queries.iter().map(|s| s.as_ref().to_string()).collect(),
            options,
        )
    }

    /// Up to 100 points in one request (each counts as one). The options apply to every point.
    pub fn reverse_batch<P: Into<Point> + Copy>(
        &self,
        points: &[P],
        options: &ReverseOptions,
    ) -> Result<ReverseBatchResponse> {
        self.engine
            .reverse_batch(points.iter().map(|p| (*p).into()).collect(), options)
    }

    /// Up to 100 IPv4 and IPv6 addresses, mixed, in one request.
    pub fn ip_batch<S: AsRef<str>>(
        &self,
        addresses: &[S],
        options: &IpOptions,
    ) -> Result<IpBatchResponse> {
        self.engine.ip_batch(
            "/ip",
            addresses.iter().map(|s| s.as_ref().to_string()).collect(),
            options,
        )
    }

    /// Up to 100 IPv4 addresses in one request.
    pub fn ipv4_batch<S: AsRef<str>>(
        &self,
        addresses: &[S],
        options: &IpOptions,
    ) -> Result<IpBatchResponse> {
        self.engine.ip_batch(
            "/ipv4",
            addresses.iter().map(|s| s.as_ref().to_string()).collect(),
            options,
        )
    }

    /// Up to 100 IPv6 addresses in one request.
    pub fn ipv6_batch<S: AsRef<str>>(
        &self,
        addresses: &[S],
        options: &IpOptions,
    ) -> Result<IpBatchResponse> {
        self.engine.ip_batch(
            "/ipv6",
            addresses.iter().map(|s| s.as_ref().to_string()).collect(),
            options,
        )
    }

    /// Timezones of up to 100 points.
    pub fn timezone_batch<P: Into<Point> + Copy>(
        &self,
        points: &[P],
        options: &TimezoneBatchOptions,
    ) -> Result<TimezoneBatchResponse> {
        self.engine
            .timezone_batch(crate::options::points(points), options)
    }

    /// Timezones along an encoded polyline (`"enc:..."`) or a `"lat,lon|lat,lon"` list.
    pub fn timezone_batch_text(
        &self,
        locations: &str,
        options: &TimezoneBatchOptions,
    ) -> Result<TimezoneBatchResponse> {
        self.engine.timezone_batch(locations.to_string(), options)
    }

    /// Heights of up to 100 points, sent as a POST body.
    pub fn elevation_batch<P: Into<Point> + Copy>(
        &self,
        points: &[P],
        options: &ElevationBatchOptions,
    ) -> Result<ElevationResponse> {
        self.engine
            .elevation_batch(points.iter().map(|p| (*p).into()).collect(), options)
    }
}
