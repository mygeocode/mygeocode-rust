# My Geocode Rust SDK

The official Rust client for the [My Geocode API](https://www.mygeocode.com/): forward and reverse geocoding, address
autocomplete, IPv4 and IPv6 lookup, timezones, elevation and postal codes.

![Rust 1.63 and newer](https://img.shields.io/badge/rust-1.63%20and%20newer-dea584)
![Dependencies: TLS only](https://img.shields.io/badge/dependencies-TLS%20only-brightgreen)
![License: MIT](https://img.shields.io/badge/license-MIT-blue)

- An async `Client` that works under any runtime (tokio, async-std, smol or none at all), and a `blocking::Client`
  with the same methods.
- Its own small HTTP client and JSON reader, so the only dependency is the TLS library for HTTPS: rustls by default,
  or the system's own with the `native-tls` feature.
- Works on Rust 1.63 and newer (1.71 with the default TLS, see [Requirements](#requirements)), with no warnings, no
  `unsafe` code and nothing global: no logger, no runtime, no environment changes.
- 2,500 free requests a day without a key or an account.

## Install

```bash
cargo add mygeocode
```

or in `Cargo.toml`:

```toml
[dependencies]
mygeocode = "1.1"
```

## Quick start (no key)

```rust
use mygeocode::blocking::Client;
use mygeocode::{ForwardOptions, ReverseOptions};

let client = Client::new();

let found = client.forward("Brandenburg Gate, Berlin", &ForwardOptions::new())?;
let place = &found.results[0];
println!("{}", place.formatted.as_deref().unwrap_or_default());      // Brandenburg Gate, State of Berlin, Germany
println!("{:?} {:?}", place.lat, place.lon);                          // Some(52.516268) Some(13.377692)

let address = client.reverse(48.8584, 2.2945, &ReverseOptions::new())?;
println!("{}", address.result.unwrap().formatted.unwrap_or_default()); // 5 Avenue Anatole France, 75007 Paris, France
```

Without a key, each network gets 2,500 free requests a day. `found.meta.quota_free_remaining` tells you how many are
left. Make one client and share it: it is cheap to clone and safe to use from many threads.

The async client has the same methods; await them under any runtime:

```rust
use mygeocode::{Client, ReverseOptions};

let client = Client::new();
let address = client.reverse(48.8584, 2.2945, &ReverseOptions::new()).await?;
```

## Using a key

A key adds your account's prepaid credit or Unlimited package on top of the free requests. Create one in the
[dashboard](https://www.mygeocode.com/dashboard).

```rust
let client = Client::builder().key("YOUR_KEY").build_blocking(); // or .build() for the async client
```

If you leave out the key, the client reads the `MYGEOCODE_KEY` environment variable. Pass `key("")` to make sure no
key is sent. The key always travels in the `X-API-Key` header, never in the URL, the SDK never writes it anywhere, and
the clients' `Debug` output hides it.

## Language

Results are in English unless you set `lang`. Set it once on the client, or per call:

```rust
let client = Client::builder().lang("de").build_blocking();

client.reverse(48.8584, 2.2945, &ReverseOptions::new().lang("fr"))?; // this call only
```

`lang` takes an ISO 639-1 code such as `de`, `ja` or `pt-BR`. The whole address comes back in that language and
script. On its own, the API answers each result in the language of its area; the SDK sends `lang=en` by default so
that every result reads the same.

## Methods

Every method takes its main arguments first, then a reference to an options value named after the method, such as
`&ForwardOptions::new()`. The options' setters carry the API's own parameter names and chain:

```rust
ForwardOptions::new().limit(3).countries(["de", "at"]).timezone(true)
```

Each method has its own options type, so an option that does not belong to a method does not compile. Every options
type also has:

- `param(name, value)` and `params(pairs)`: more query parameters, sent as they are (and winning over the named
  options), so a parameter the API adds later works before the SDK knows it.
- `pretty(true)` for indented JSON.

Points are `Point::new(lat, lon)`, a `(lat, lon)` tuple or a `[lat, lon]` array.

| Method | Endpoint |
|---|---|
| [`forward`](#forward) | `GET /v1/forward` |
| [`reverse`](#reverse) | `GET /v1/reverse` |
| [`autocomplete`](#autocomplete) | `GET /v1/autocomplete` |
| [`ip`, `ipv4`, `ipv6`](#ip-ipv4-ipv6) | `GET /v1/ip`, `/v1/ipv4`, `/v1/ipv6` |
| [`timezone`, `timezone_with`](#timezone) | `GET /v1/timezone` |
| [`elevation`, `elevation_with`](#elevation) | `GET /v1/elevation` |
| [`postcode`](#postcode) | `GET /v1/postcode` |
| [`forward_batch`](#forward_batch) | `POST /v1/forward` |
| [`reverse_batch`](#reverse_batch) | `POST /v1/reverse` |
| [`ip_batch`, `ipv4_batch`, `ipv6_batch`](#ip_batch-ipv4_batch-ipv6_batch) | `GET /v1/ip`, `/v1/ipv4`, `/v1/ipv6` with `ips` |
| [`timezone_batch`](#timezone_batch) | `GET /v1/timezone` with `locations` |
| [`elevation_batch`](#elevation_batch) | `POST /v1/elevation` |

The examples below use the blocking client; the async one is the same with `.await`.

### forward

Text, structured address fields or a place id to coordinates. Returns a `ForwardResponse` whose `results` holds up to
`limit` `Place` values, best first. An empty list means nothing matched.

```rust
let found = client.forward("Brandenburg Gate, Berlin", &ForwardOptions::new())?;
println!("{}", found.results[0].formatted.as_deref().unwrap_or_default()); // Brandenburg Gate, State of Berlin, Germany

client.forward("Brandenburg Gate, Berlin", &ForwardOptions::new().limit(3).countries(["de"]))?;
client.forward("", &ForwardOptions::new().street("10 Downing Street").city("London").country("GB"))?;
```

`ForwardOptions`:

| Option | What it does |
|---|---|
| `street`, `city`, `state`, `postcode`, `country` | Structured search instead of (or as well as) free text. Pass `""` as the query to search by these alone. |
| `place_id` | Look up a `place_id` from an earlier result. |
| `limit` | 1 to 10 results. |
| `countries` | ISO 3166-1 alpha-2 codes to search in, such as `countries(["de", "at"])`. Takes anything that iterates over strings. |
| `bounds` | A box that results should fall in: `bounds(south, west, north, east)`. |
| `strict` | Only return results inside `bounds`. |
| `proximity` | Prefer results near this point. |
| `types` | Kinds of result to keep, such as `address` or `poi`. |
| `lang` | Language of the result. |
| `timezone`, `elevation`, `wikidata`, `country_info`, `codes`, `sun`, `regions`, `districts`, `census`, `demographics`, `road`, `site` | Each adds its extra block to every result. |
| `annotations` | Adds every extra block at once. |

### reverse

A point to the nearest address. Returns a `ReverseResponse` whose `result` is a `Place`, or `None` when nothing is in
range.

```rust
let address = client.reverse(48.8584, 2.2945, &ReverseOptions::new())?;
let place = address.result.unwrap();
println!("{}", place.formatted.as_deref().unwrap_or_default()); // 5 Avenue Anatole France, 75007 Paris, France
println!("{:?}", place.distance_m);                              // Some(47) (metres from the point you gave)

client.reverse(48.8584, 2.2945, &ReverseOptions::new().level("city").timezone(true))?;
```

`ReverseOptions`:

| Option | What it does |
|---|---|
| `level` | `address`, `street`, `postcode`, `city`, `region` or `country`. |
| `radius` | Search radius in metres, 1 to 5000. |
| `lang` | Language of the result. |
| `boundaries` | Add the administrative boundaries the point falls in (Australia). |
| `timezone`, `elevation`, `wikidata`, `country_info`, `codes`, `sun`, `regions`, `districts`, `census`, `demographics`, `road`, `site` | Each adds its extra block, as for `forward`. |
| `annotations` | Adds every extra block at once. |

### autocomplete

Suggestions while someone types an address. Returns an `AutocompleteResponse` with a `suggestions` list; each
`Suggestion` has `text`, `lat`, `lon`, `type`, `precision` and `place_id`.

```rust
let typed = client.autocomplete("1600 Pennsylvania", &AutocompleteOptions::new())?;
println!("{}", typed.suggestions[0].text.as_deref().unwrap_or_default()); // 1600 Pennsylvania Avenue NW, Washington, DC 20500, United States

client.autocomplete("1600 Pennsylvania", &AutocompleteOptions::new().limit(5).countries(["us"]))?;
```

`AutocompleteOptions`: `limit`, `countries`, `proximity`, `bounds`, `strict`, `types` and `lang`, as for `forward`.

### ip, ipv4, ipv6

Country, region, city, coordinates, timezone, network owner (ASN) and proxy, VPN and data centre flags for an IP
address. `ip` takes either version; `ipv4` and `ipv6` accept only their own. `None` instead of an address looks up the
address the request comes from. Returns an `IpInfo`.

```rust
let google = client.ip(Some("8.8.8.8"), &IpOptions::new())?;
println!("{:?} {:?}", google.country, google.city);  // Some("United States") Some("Mountain View")
println!("{:?} {:?}", google.asn, google.as_name);   // Some(15169) Some("Google LLC")
println!("{:?}", google.is_datacenter);              // Some(true)

client.ipv6(Some("2001:4860:4860::8888"), &IpOptions::new())?;
client.ip(None, &IpOptions::new())?;                              // the address the request came from
client.ipv4(Some("8.8.8.8"), &IpOptions::new().elevation(true))?; // adds elevation_m
```

`IpOptions`: `elevation`. The full list of `IpInfo` fields is under [Results and meta](#results-and-meta).

### timezone

The IANA zone, UTC offset, daylight saving state and local time for a point, now or at a given moment. Returns a
`TimezoneInfo`. `timezone_with` takes only options: a zone id, or several points.

```rust
let tokyo = client.timezone(35.6762, 139.6503, &TimezoneOptions::new())?;
println!("{:?} {:?}", tokyo.timezone, tokyo.utc_offset);   // Some("Asia/Tokyo") Some("+09:00")
println!("{:?} {:?}", tokyo.name, tokyo.abbreviation);     // Some("Japan Standard Time") Some("JST")
println!("{:?}", tokyo.local_time);                        // Some("2026-11-24T18:07:15+09:00")

client.timezone(35.6762, 139.6503, &TimezoneOptions::new().timestamp(1767225600))?;
client.timezone_with(&TimezoneOptions::new().tz("Europe/Paris"))?;
```

`TimezoneOptions`:

| Option | What it does |
|---|---|
| `tz` | Look up a zone by its IANA or Windows id instead of a point (with `timezone_with`). |
| `timestamp` | Unix time to answer for. Default: now. |
| `lang` | Language of the zone name. Sent as the API's `language` parameter, only when you set it here or on the client. `language` works as another name for this option. |
| `locations`, `locations_text` | Several points: a slice of points, or `"lat,lon\|lat,lon"` or an `enc:` polyline (with `timezone_with`). |
| `path`, `path_text`, `samples` | Points along a path, sampled `samples` times (2 to 100). |
| `bounds`, `rows`, `cols` | A grid of points over a box. |

For a plain list of points, `timezone_batch` is simpler.

### elevation

Height in metres above sea level (negative for the sea floor), from a global 15 arc-second grid. Returns an
`ElevationResponse` whose `results` holds `ElevationPoint` values with `lat`, `lon`, `elevation_m` and
`resolution_m`. `elevation_with` takes only options: several points, a path or a grid.

```rust
let everest = client.elevation(27.9881, 86.9250, &ElevationOptions::new())?;
println!("{:?}", everest.results[0].elevation_m); // Some(8627.0)

client.elevation_with(&ElevationOptions::new().path(&[(27.9881, 86.9250), (28.0, 86.85)]).samples(10))?;
```

`ElevationOptions`: `locations`, `path` and `samples`, or `bounds`, `rows` and `cols`, as for `timezone`.

### postcode

A postal code to its place, region and coordinates. Returns a `PostcodeResponse` whose `results` lists every place that
uses the code.

```rust
let nyc = client.postcode("10001", &PostcodeOptions::new().country("US"))?;
let first = &nyc.results[0];
println!("{:?} {:?}", first.place, first.region_code);  // Some("New York") Some("NY")
println!("{:?} {:?}", first.lat, first.lon);            // Some(40.748401) Some(-73.996702)

client.postcode("75008", &PostcodeOptions::new().country("FR"))?;
```

`PostcodeOptions`: `country` (ISO alpha-2). Without it, the code is matched in every country.

### forward_batch

Up to 100 searches in one request (each counts as one request against your quota). Returns a `ForwardBatchResponse`
whose `results` holds one `ForwardBatchEntry` per search, in the order you sent them; each entry has the `query` and
its own `results` list of `Place` values.

```rust
let batch = client.forward_batch(&["Brandenburg Gate, Berlin", "Sydney Opera House", "Statue of Liberty, New York"], &ForwardBatchOptions::new())?;
for entry in &batch.results {
    println!("{}: {}", entry.query.as_deref().unwrap_or_default(), entry.results[0].formatted.as_deref().unwrap_or_default());
}
// Brandenburg Gate, Berlin: Brandenburg Gate, State of Berlin, Germany
// Sydney Opera House: Sydney Opera House, Sydney, New South Wales, Australia
// Statue of Liberty, New York: Statue of Liberty, New York, New York, United States

client.forward_batch(&["Brandenburg Gate, Berlin", "Sydney Opera House"], &ForwardBatchOptions::new().limit(1).timezone(true))?;
```

`ForwardBatchOptions` apply to every search in the batch: `limit`, `countries`, `bounds`, `strict`, `proximity`,
`types`, `lang` and the extra blocks, as for `forward`. Structured fields and `place_id` are not available in a batch;
use `forward` for those.

### reverse_batch

Up to 100 points in one request (each counts as one request). Returns a `ReverseBatchResponse` whose `results` holds
one `ReverseResponse` per point, in the order you sent them; its `result` is a `Place`, or `None` when nothing is in
range.

```rust
let batch = client.reverse_batch(&[(48.8584, 2.2945), (40.6892, -74.0445), (-33.8568, 151.2153)], &ReverseOptions::new())?;
for point in &batch.results {
    println!("{}", point.result.as_ref().and_then(|p| p.formatted.as_deref()).unwrap_or("nothing in range"));
}
// 5 Avenue Anatole France, 75007 Paris, France
// 1 Liberty Island, New York, NY 10004, United States
// 2B Macquarie Street, Sydney NSW 2000, Australia
```

It takes the same `ReverseOptions` as `reverse`, applied to every point in the batch.

### ip_batch, ipv4_batch, ipv6_batch

Up to 100 addresses in one request. `ip_batch` takes both versions mixed; `ipv4_batch` and `ipv6_batch` accept only
their own. Returns an `IpBatchResponse` whose `results` holds one `IpInfo` per address, in the order you sent them. An
address the API cannot read gets an entry whose `status` is `error` and whose `error` explains why, while the rest
still answer.

```rust
let batch = client.ipv4_batch(&["8.8.8.8", "1.1.1.1", "9.9.9.9"], &IpOptions::new())?;
for info in &batch.results {
    println!("{}: {}", info.ip.as_deref().unwrap_or_default(), info.as_name.as_deref().unwrap_or("unknown"));
}
// 8.8.8.8: Google LLC
// 1.1.1.1: Cloudflare, Inc.
// 9.9.9.9: Quad9

client.ipv6_batch(&["2001:4860:4860::8888", "2606:4700:4700::1111"], &IpOptions::new())?;
client.ip_batch(&["8.8.8.8", "2606:4700:4700::1111"], &IpOptions::new().elevation(true))?;
```

`IpOptions`: `elevation`, adds `elevation_m` to every address.

### timezone_batch

Timezones for up to 100 points. Returns a `TimezoneBatchResponse` whose `results` holds one `TimezoneInfo` per point,
in the order you sent them. `timezone_batch_text` takes the points as an `enc:` polyline instead.

```rust
let batch = client.timezone_batch(&[(48.8584, 2.2945), (40.6892, -74.0445), (-33.8568, 151.2153)], &TimezoneBatchOptions::new())?;
for zone in &batch.results {
    println!("{}", zone.timezone.as_deref().unwrap_or_default());
}
// Europe/Paris
// America/New_York
// Australia/Sydney
```

`TimezoneBatchOptions`: `timestamp` (Unix time, default now) and `lang` (also accepted as `language`), as for
`timezone`.

### elevation_batch

Heights for up to 100 points, sent as a POST body. Returns an `ElevationResponse`.

```rust
let batch = client.elevation_batch(&[(48.8584, 2.2945), (40.6892, -74.0445), (-33.8568, 151.2153)], &ElevationBatchOptions::new())?;
for point in &batch.results {
    println!("{:?}", point.elevation_m);
}
// Some(37.0)
// Some(0.0)
// Some(10.0)
```

## Async

The async `Client` has every method above with the same arguments. Each returns a `Pending` future that owns
everything the call needs, so you can store it, move it to another task, or start several and await them together.
The request starts when the future is first polled and runs on a thread of its own, so it never blocks the executor and
works under any runtime:

```rust
use mygeocode::{Client, ElevationOptions, TimezoneOptions};

let client = Client::new();
let tokyo = client.timezone(35.6762, 139.6503, &TimezoneOptions::new());
let everest = client.elevation(27.9881, 86.9250, &ElevationOptions::new());
println!("{:?} {:?}", tokyo.await?.timezone, everest.await?.results[0].elevation_m); // Some("Asia/Tokyo") Some(8627.0)
```

`examples/async_example.rs` runs the same calls with no runtime at all, through a ten-line `block_on`.

## Results and meta

Results are plain structs whose public fields carry the API's own field names, so the
[API documentation](https://www.mygeocode.com/docs/) describes them field by field:

```rust
let found = client.forward("Brandenburg Gate, Berlin", &ForwardOptions::new())?;
let place = &found.results[0];
let parts = place.components.as_ref().unwrap();

println!("{:?}", place.formatted);     // Some("Brandenburg Gate, State of Berlin, Germany")
println!("{:?}", place.precision);     // Some("house")
println!("{:?}", parts.poi);           // Some("Brandenburg Gate")
println!("{:?}", parts.country_code);  // Some("de")
println!("{:?}", parts.city);          // None (not every address has every part)
```

Every field is an `Option` (the field `type` is `r#type`), so a field the API did not send is `None`, never a made-up
`0` or `""`. The fields this SDK reads are never renamed or removed from the API: if one ever has to go, it keeps
coming back as `None`, and your code keeps working. Lists are `Vec`s, empty when the API sent none, and the address
parts in `Components` are always text.

Every result also has `raw`, the decoded JSON as a `mygeocode::Value`, so fields added to the API later are readable
before the SDK knows them: `place.raw.get("new_field").as_str()`. The result structs are `#[non_exhaustive]`, so new
fields can be added without breaking your code.

What each method returns, and its fields:

| Type | Returned by | Fields |
|---|---|---|
| `ForwardResponse` | `forward` | `status`, `query`, `results` (`Vec<Place>`), `meta`, `raw` |
| `ReverseResponse` | `reverse`, each entry of `reverse_batch` | `status`, `lat`, `lon`, `result` (`Option<Place>`), `boundaries`, `meta`, `raw` |
| `Place` | inside forward and reverse results | `formatted`, `lat`, `lon`, `type`, `precision`, `confidence`, `place_id`, `components`, `bounds`, `distance_m`, `formatted_place`, `place`, `parcel`, `codes`, `population`, `un_locode`, `timezone`, `elevation_m`, `wikidata`, `country_info`, `sun`, `regions`, `districts`, `census`, `demographics`, `road`, `intersection`, `site`, `raw` |
| `Components` | `Place::components` | `poi`, `building`, `unit`, `level`, `house_number`, `road`, `lot`, `suburb`, `municipality`, `city`, `county`, `state`, `state_code`, `postcode`, `country`, `country_code`, `official_name`, `raw` |
| `Bounds` | `Place::bounds` | `north`, `south`, `east`, `west`, `raw` |
| `AutocompleteResponse` | `autocomplete` | `status`, `query`, `suggestions` (`Vec<Suggestion>`), `meta`, `raw` |
| `Suggestion` | inside autocomplete results | `text`, `lat`, `lon`, `type`, `precision`, `place_id`, `raw` |
| `IpInfo` | `ip`, `ipv4`, `ipv6`, each entry of the IP batches | `status`, `ip`, `version`, `found`, `prefix`, `country`, `country_code`, `continent`, `continent_code`, `region`, `region_code`, `city`, `postcode`, `lat`, `lon`, `elevation_m`, `timezone`, `utc_offset`, `utc_offset_seconds`, `is_eu`, `asn`, `as_name`, `org`, `usage_type`, `registry`, `allocated`, `tags`, `is_anycast`, `is_satellite`, `is_datacenter`, `is_proxy`, `is_vpn_provider`, `is_tor`, `crawler`, `is_private`, `reason`, `abuse_email`, `threat`, `updated_at`, `error`, `meta`, `raw` |
| `TimezoneInfo` | `timezone`, each entry of `timezone_batch` | `status`, `lat`, `lon`, `timezone`, `name`, `abbreviation`, `utc_offset`, `utc_offset_seconds`, `raw_offset_seconds`, `dst`, `dst_offset_seconds`, `timestamp`, `local_time`, `country_code`, `is_ocean`, `windows_timezone`, `next_transition` (`Option<TimezoneTransition>`), `meta`, `raw` |
| `TimezoneTransition` | `TimezoneInfo::next_transition` | `timestamp`, `utc_offset`, `utc_offset_seconds`, `dst`, `abbreviation`, `raw` |
| `ElevationResponse` | `elevation`, `elevation_batch` | `status`, `results` (`Vec<ElevationPoint>`), `meta`, `raw` |
| `ElevationPoint` | inside elevation results | `lat`, `lon`, `elevation_m`, `resolution_m`, `raw` |
| `PostcodeResponse` | `postcode` | `status`, `postcode`, `country_code`, `results` (`Vec<PostcodePlace>`), `meta`, `raw` |
| `PostcodePlace` | inside postcode results | `postcode`, `place`, `region`, `region_code`, `country`, `country_code`, `lat`, `lon`, `raw` |
| `ForwardBatchResponse` | `forward_batch` | `status`, `results` (`Vec<ForwardBatchEntry>`), `meta`, `raw` |
| `ForwardBatchEntry` | inside forward batch results | `query`, `results` (`Vec<Place>`), `raw` |
| `ReverseBatchResponse` | `reverse_batch` | `status`, `results` (`Vec<ReverseResponse>`), `meta`, `raw` |
| `IpBatchResponse` | `ip_batch`, `ipv4_batch`, `ipv6_batch` | `status`, `results` (`Vec<IpInfo>`), `meta`, `raw` |
| `TimezoneBatchResponse` | `timezone_batch` | `status`, `timestamp`, `results` (`Vec<TimezoneInfo>`), `meta`, `raw` |

The extra blocks you ask for with the flags (`timezone`, `elevation`, `wikidata` and the rest) are fields of `Place`
holding a `Value`. On an entry inside a batch, `meta` is empty; the batch response has the meta.

Every response has `meta`, filled from the response headers (`None` when a header was not sent):

| Field | Header | Meaning |
|---|---|---|
| `request_id` | `X-Request-Id` | Quote it when you contact support. |
| `quota_limit` | `X-Quota-Limit` | Free requests a day for this key's account or network. |
| `quota_used` | `X-Quota-Used` | Free requests used today. |
| `quota_free_remaining` | `X-Quota-Free-Remaining` | Free requests left today. |
| `quota_reset` | `X-Quota-Reset` | Unix time when the free requests reset. |
| `credits_remaining` | `X-Credits-Remaining` | Prepaid credit left, when a key with credit is used. |

## Errors

Every method returns `mygeocode::Result<T>`. The error is a `mygeocode::Error` (also named `MyGeocodeError`):

| Variant | When |
|---|---|
| `Error::Api` | The API answered with an error. The `ApiError` inside has `status` (HTTP), `code` (the API's error code, such as `invalid_request`, `invalid_key` or `quota_exceeded`), `message`, `param` (the parameter at fault), `request_id`, `retry_after` (a `Duration`) and `body` (the decoded answer). |
| `Error::RequestTimeout` | No answer within `timeout`. |
| `Error::Network` | The connection failed: DNS, refused, reset or TLS. The original error is its `source()`. |
| `Error::Decode` | The answer could not be read as JSON. |

```rust
match client.forward("Brandenburg Gate, Berlin", &ForwardOptions::new()) {
    Ok(found) => println!("{} results", found.results.len()), // 5 results
    Err(mygeocode::Error::Api(e)) => println!("{} {} {}", e.status, e.code, e.message),
    Err(e) => println!("could not reach the API: {}", e),
}
```

`error.api()` and `error.code()` read the API error without a `match`, and `error.is_timeout()` tells a timeout
apart. The [errors page](https://www.mygeocode.com/docs/errors/) lists every error code.

## Retries

The client retries on its own, up to `max_retries` times (default 2):

- **429** (too many requests): waits for `Retry-After`, or until `X-Quota-Reset`. If that is longer than
  `max_retry_wait` (default 60 seconds), for example when the day's free requests are gone, it returns the error
  straight away instead of waiting.
- **500**: tries once more after one second.
- **503**: waits for `Retry-After`, or 1, 2 and then 4 seconds.
- **Timeouts and connection errors**: retried with the same 1, 2, 4 second pattern for GET requests. The POST batch
  calls are not retried after a connection error, so a batch is never charged twice.

Other errors are returned at once.

## Configuration

All options are optional:

```rust
use std::time::Duration;

let client = mygeocode::Client::builder()
    .key("YOUR_KEY")                          // default: MYGEOCODE_KEY, else no key
    .base_url("https://api.mygeocode.com/v1")
    .timeout(Duration::from_secs(10))         // for each attempt, from connecting to the last byte
    .max_retries(2)
    .max_retry_wait(Duration::from_secs(60))
    .lang("en")
    .build();                                 // or .build_blocking()
```

`transport`, `sleep` and `clock` replace the HTTP layer, the wait and the clock the retry logic uses; they are meant
for tests and for plugging in another HTTP library, see below.

### Features

| Feature | What it does |
|---|---|
| `rustls` (default) | HTTPS through rustls, with Mozilla's root certificates built in. Pure Rust; its `ring` crate needs a C compiler to build. |
| `native-tls` | HTTPS through the system's TLS library (when both features are on, rustls is used): OpenSSL on Linux (needs its development files, such as `libssl-dev` and `pkg-config`), Secure Transport on macOS, SChannel on Windows. Uses the system's certificate store. |
| none | `default-features = false`: no dependencies at all. Plain `http://` works, and HTTPS goes through your own transport. |

```toml
mygeocode = { version = "1.1", default-features = false, features = ["native-tls"] }
```

The default transport opens one connection per request, follows no redirects and uses no proxy. For a proxy or
connection pooling, plug in your own HTTP library through `transport`.

## Testing your code (transport hook)

The client sends every request through its `Transport`: a trait with one method that takes an `HttpRequest`
(`method`, `url`, `headers`, `body`, `timeout`) and returns an `HttpResponse` (status, headers, body text), or a
`TransportError`. A closure is a transport too. Give it your own to test code that uses the SDK without touching the
network:

```rust
use mygeocode::{HttpRequest, HttpResponse, ReverseOptions, TransportError};

let canned = |_request: &HttpRequest| -> Result<HttpResponse, TransportError> {
    Ok(HttpResponse::new(200, vec![], r#"{"status":"ok","lat":48.8584,"lon":2.2945,"result":{"formatted":"5 Avenue Anatole France, 75007 Paris, France","lat":48.858821,"lon":2.294596}}"#))
};

let client = mygeocode::Client::builder().transport(canned).sleep(|_| {}).build_blocking(); // no real waiting in retry tests
let address = client.reverse(48.8584, 2.2945, &ReverseOptions::new())?;
println!("{}", address.result.unwrap().formatted.unwrap_or_default()); // 5 Avenue Anatole France, 75007 Paris, France
```

The same hook plugs in any HTTP library, such as reqwest or ureq with your own proxy and pooling settings. Return every
HTTP status as an `HttpResponse`, and an error only when no answer arrived: `TransportError::Timeout` for a timeout,
`TransportError::Network` for anything else.

### Running the SDK's own tests

The tests use only the standard library, so the same suite runs on every Rust from 1.63. They answer from real
recorded API responses (and a small local server for the transport) and never call the API:

```bash
cargo test
cargo test --no-default-features
```

## Requirements

Tested on every stable Rust from 1.63 to 1.100, on beta and on nightly, with every warning turned into an error.

| What you build | Rust |
|---|---|
| Without TLS (`default-features = false`): no dependencies | 1.63 and newer |
| Default (`rustls`) | 1.85 and newer as it is. Down to 1.71 when your lockfile picks dependency versions that support it: cargo 1.84 and newer does that with `resolver = "3"` (or `CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback`); with an older cargo, run `cargo update -p zeroize --precise 1.8.2` once |
| `native-tls` | 1.80 and newer |

The figures for the TLS features come from their dependencies; the SDK's own code builds on 1.63. Nothing else is
needed at run time: the default TLS brings its own root certificates, and `native-tls` uses the system's.

## Links

- [API documentation](https://www.mygeocode.com/docs/)
- [Pricing and free tier](https://www.mygeocode.com/pricing/)
- [Dashboard and API keys](https://www.mygeocode.com/dashboard)
- [Issues](https://github.com/mygeocode/mygeocode-rust/issues)
- Support: [support@mygeocode.com](mailto:support@mygeocode.com)

## Licence

MIT. See [LICENSE](LICENSE).
