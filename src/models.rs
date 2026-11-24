//! The results. Field names are the API's own. Every field can be missing: text, numbers and flags are `Option`s,
//! lists are `Vec`s (empty when missing), and a field this crate reads is never renamed or removed by the API (at
//! most it becomes `None`). `raw` holds the decoded JSON exactly as the API sent it, fields this version does not
//! know included.

use crate::http::HttpResponse;
use crate::json::Value;

/// Figures from the response headers. Each one is `None` when its header was not sent (and on an entry inside a batch).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Meta {
    /// `X-Request-Id`. Quote it when you contact support.
    pub request_id: Option<String>,
    /// `X-Quota-Limit`: free requests a day for this key's account or network (-1 on Unlimited).
    pub quota_limit: Option<i64>,
    /// `X-Quota-Used`: free requests used today.
    pub quota_used: Option<i64>,
    /// `X-Quota-Free-Remaining`: free requests left today.
    pub quota_free_remaining: Option<i64>,
    /// `X-Quota-Reset`: Unix time in seconds when the free requests reset.
    pub quota_reset: Option<i64>,
    /// `X-Credits-Remaining`: prepaid credit left, sent only for a key with credit.
    pub credits_remaining: Option<i64>,
}

impl Meta {
    pub(crate) fn from_response(r: &HttpResponse) -> Meta {
        let number = |name: &str| {
            r.header(name)
                .and_then(|v| v.trim().parse::<f64>().ok())
                .filter(|f| f.is_finite())
                .map(|f| f as i64)
        };
        Meta {
            request_id: r
                .header("x-request-id")
                .filter(|v| !v.is_empty())
                .map(|v| v.to_string()),
            quota_limit: number("x-quota-limit"),
            quota_used: number("x-quota-used"),
            quota_free_remaining: number("x-quota-free-remaining"),
            quota_reset: number("x-quota-reset"),
            credits_remaining: number("x-credits-remaining"),
        }
    }
}

fn object(v: &Value) -> Option<Value> {
    match v {
        Value::Object(_) => Some(v.clone()),
        _ => None,
    }
}

fn texts(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|items| items.iter().filter_map(|i| i.text()).collect())
        .unwrap_or_default()
}

fn model<T>(v: &Value, read: fn(&Value) -> T) -> Option<T> {
    match v {
        Value::Object(_) => Some(read(v)),
        _ => None,
    }
}

/// A list of results; an entry that is not an object reads as an empty result, so batch entries keep their order.
fn models<T: Default>(v: &Value, read: fn(&Value) -> T) -> Vec<T> {
    v.as_array()
        .map(|items| {
            items
                .iter()
                .map(|i| {
                    if i.as_object().is_some() {
                        read(i)
                    } else {
                        T::default()
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The box around a place, in degrees.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct Bounds {
    /// Northern edge.
    pub north: Option<f64>,
    /// Southern edge.
    pub south: Option<f64>,
    /// Eastern edge.
    pub east: Option<f64>,
    /// Western edge.
    pub west: Option<f64>,
    /// The decoded JSON exactly as the API sent it.
    pub raw: Value,
}

impl Bounds {
    pub(crate) fn from_value(v: &Value) -> Bounds {
        Bounds {
            north: v.get("north").as_f64(),
            south: v.get("south").as_f64(),
            east: v.get("east").as_f64(),
            west: v.get("west").as_f64(),
            raw: v.clone(),
        }
    }
}

/// The parts of an address. Each one is text (a number the API sends in one of these fields is read as text), and `None` when the address has no such part.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct Components {
    /// Point of interest name.
    pub poi: Option<String>,
    /// Building name.
    pub building: Option<String>,
    /// Flat, suite or unit.
    pub unit: Option<String>,
    /// Floor.
    pub level: Option<String>,
    /// House number.
    pub house_number: Option<String>,
    /// Street.
    pub road: Option<String>,
    /// Lot or plot.
    pub lot: Option<String>,
    /// Suburb or neighbourhood.
    pub suburb: Option<String>,
    /// Municipality.
    pub municipality: Option<String>,
    /// City, town or village.
    pub city: Option<String>,
    /// County or district.
    pub county: Option<String>,
    /// State, province or region.
    pub state: Option<String>,
    /// State code.
    pub state_code: Option<String>,
    /// Postal code.
    pub postcode: Option<String>,
    /// Country name.
    pub country: Option<String>,
    /// ISO 3166-1 alpha-2 country code, lower case.
    pub country_code: Option<String>,
    /// Official name of the place.
    pub official_name: Option<String>,
    /// The decoded JSON exactly as the API sent it.
    pub raw: Value,
}

impl Components {
    pub(crate) fn from_value(v: &Value) -> Components {
        Components {
            poi: v.get("poi").text(),
            building: v.get("building").text(),
            unit: v.get("unit").text(),
            level: v.get("level").text(),
            house_number: v.get("house_number").text(),
            road: v.get("road").text(),
            lot: v.get("lot").text(),
            suburb: v.get("suburb").text(),
            municipality: v.get("municipality").text(),
            city: v.get("city").text(),
            county: v.get("county").text(),
            state: v.get("state").text(),
            state_code: v.get("state_code").text(),
            postcode: v.get("postcode").text(),
            country: v.get("country").text(),
            country_code: v.get("country_code").text(),
            official_name: v.get("official_name").text(),
            raw: v.clone(),
        }
    }
}

/// One forward or reverse result. The extra blocks you ask for with the annotation flags (`timezone`, `codes` and the rest) are plain JSON objects, `None` when not asked for.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct Place {
    /// The whole address on one line.
    pub formatted: Option<String>,
    /// Latitude.
    pub lat: Option<f64>,
    /// Longitude.
    pub lon: Option<f64>,
    /// The kind of result: address, street, postcode, city, suburb, region, country or poi.
    pub r#type: Option<String>,
    /// How precise the match is: house, street, postcode, city, region or country.
    pub precision: Option<String>,
    /// How sure the match is, 0 to 1.
    pub confidence: Option<f64>,
    /// An id you can look up again with `ForwardOptions::place_id`.
    pub place_id: Option<String>,
    /// The address parts.
    pub components: Option<Components>,
    /// The box around the place.
    pub bounds: Option<Bounds>,
    /// Reverse only: metres from the point you gave.
    pub distance_m: Option<i64>,
    /// The place name on one line, without street and number.
    pub formatted_place: Option<String>,
    /// The place block.
    pub place: Option<Value>,
    /// The land parcel block, where the country publishes one.
    pub parcel: Option<Value>,
    /// The codes block (plus code, geohash, MGRS ...), with the `codes` flag.
    pub codes: Option<Value>,
    /// The population of a city or region result.
    pub population: Option<i64>,
    /// The UN/LOCODE block.
    pub un_locode: Option<Value>,
    /// The timezone block, with the `timezone` flag.
    pub timezone: Option<Value>,
    /// Metres above sea level, with the `elevation` flag.
    pub elevation_m: Option<f64>,
    /// The Wikidata id, with the `wikidata` flag.
    pub wikidata: Option<String>,
    /// The country block, with the `country_info` flag.
    pub country_info: Option<Value>,
    /// Sunrise and sunset, with the `sun` flag.
    pub sun: Option<Value>,
    /// The regions block, with the `regions` flag.
    pub regions: Option<Value>,
    /// The districts block, with the `districts` flag.
    pub districts: Option<Value>,
    /// The census block, with the `census` flag.
    pub census: Option<Value>,
    /// The demographics block, with the `demographics` flag.
    pub demographics: Option<Value>,
    /// The road block, with the `road` flag.
    pub road: Option<Value>,
    /// The nearest intersection.
    pub intersection: Option<Value>,
    /// The site block, with the `site` flag.
    pub site: Option<Value>,
    /// The decoded JSON exactly as the API sent it.
    pub raw: Value,
}

impl Place {
    pub(crate) fn from_value(v: &Value) -> Place {
        Place {
            formatted: v.get("formatted").text(),
            lat: v.get("lat").as_f64(),
            lon: v.get("lon").as_f64(),
            r#type: v.get("type").text(),
            precision: v.get("precision").text(),
            confidence: v.get("confidence").as_f64(),
            place_id: v.get("place_id").text(),
            components: model(v.get("components"), Components::from_value),
            bounds: model(v.get("bounds"), Bounds::from_value),
            distance_m: v.get("distance_m").as_i64(),
            formatted_place: v.get("formatted_place").text(),
            place: object(v.get("place")),
            parcel: object(v.get("parcel")),
            codes: object(v.get("codes")),
            population: v.get("population").as_i64(),
            un_locode: object(v.get("un_locode")),
            timezone: object(v.get("timezone")),
            elevation_m: v.get("elevation_m").as_f64(),
            wikidata: v.get("wikidata").text(),
            country_info: object(v.get("country_info")),
            sun: object(v.get("sun")),
            regions: object(v.get("regions")),
            districts: object(v.get("districts")),
            census: object(v.get("census")),
            demographics: object(v.get("demographics")),
            road: object(v.get("road")),
            intersection: object(v.get("intersection")),
            site: object(v.get("site")),
            raw: v.clone(),
        }
    }
}

/// The answer of `forward`: the matches, best first.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ForwardResponse {
    /// `ok`.
    pub status: Option<String>,
    /// The text that was searched.
    pub query: Option<String>,
    /// The matches, best first; empty when nothing matched.
    pub results: Vec<Place>,
    /// Request id and quota figures from the response headers (empty on an entry inside a batch).
    pub meta: Meta,
    /// The decoded JSON exactly as the API sent it.
    pub raw: Value,
}

impl ForwardResponse {
    pub(crate) fn from_value(v: &Value) -> ForwardResponse {
        ForwardResponse {
            status: v.get("status").text(),
            query: v.get("query").text(),
            results: models(v.get("results"), Place::from_value),
            meta: Meta::default(),
            raw: v.clone(),
        }
    }
}

/// One search of a forward batch.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ForwardBatchEntry {
    /// The text that was searched.
    pub query: Option<String>,
    /// Its matches, best first.
    pub results: Vec<Place>,
    /// The decoded JSON exactly as the API sent it.
    pub raw: Value,
}

impl ForwardBatchEntry {
    pub(crate) fn from_value(v: &Value) -> ForwardBatchEntry {
        ForwardBatchEntry {
            query: v.get("query").text(),
            results: models(v.get("results"), Place::from_value),
            raw: v.clone(),
        }
    }
}

/// The answer of `forward_batch`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ForwardBatchResponse {
    /// `ok`.
    pub status: Option<String>,
    /// One entry per search, in the order sent.
    pub results: Vec<ForwardBatchEntry>,
    /// Request id and quota figures from the response headers (empty on an entry inside a batch).
    pub meta: Meta,
    /// The decoded JSON exactly as the API sent it.
    pub raw: Value,
}

impl ForwardBatchResponse {
    pub(crate) fn from_value(v: &Value) -> ForwardBatchResponse {
        ForwardBatchResponse {
            status: v.get("status").text(),
            results: models(v.get("results"), ForwardBatchEntry::from_value),
            meta: Meta::default(),
            raw: v.clone(),
        }
    }
}

/// The answer of `reverse`, and one entry of a reverse batch.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ReverseResponse {
    /// `ok`.
    pub status: Option<String>,
    /// The latitude you sent.
    pub lat: Option<f64>,
    /// The longitude you sent.
    pub lon: Option<f64>,
    /// The nearest address, or `None` when nothing is in range.
    pub result: Option<Place>,
    /// The Australian boundaries, with the `boundaries` option.
    pub boundaries: Option<Value>,
    /// Request id and quota figures from the response headers (empty on an entry inside a batch).
    pub meta: Meta,
    /// The decoded JSON exactly as the API sent it.
    pub raw: Value,
}

impl ReverseResponse {
    pub(crate) fn from_value(v: &Value) -> ReverseResponse {
        ReverseResponse {
            status: v.get("status").text(),
            lat: v.get("lat").as_f64(),
            lon: v.get("lon").as_f64(),
            result: model(v.get("result"), Place::from_value),
            boundaries: object(v.get("boundaries")),
            meta: Meta::default(),
            raw: v.clone(),
        }
    }
}

/// The answer of `reverse_batch`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ReverseBatchResponse {
    /// `ok`.
    pub status: Option<String>,
    /// One entry per point, in the order sent.
    pub results: Vec<ReverseResponse>,
    /// Request id and quota figures from the response headers (empty on an entry inside a batch).
    pub meta: Meta,
    /// The decoded JSON exactly as the API sent it.
    pub raw: Value,
}

impl ReverseBatchResponse {
    pub(crate) fn from_value(v: &Value) -> ReverseBatchResponse {
        ReverseBatchResponse {
            status: v.get("status").text(),
            results: models(v.get("results"), ReverseResponse::from_value),
            meta: Meta::default(),
            raw: v.clone(),
        }
    }
}

/// One autocomplete suggestion.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct Suggestion {
    /// The suggestion as shown to the user.
    pub text: Option<String>,
    /// Latitude.
    pub lat: Option<f64>,
    /// Longitude.
    pub lon: Option<f64>,
    /// The kind of result: address, street, postcode, city, region, country or poi.
    pub r#type: Option<String>,
    /// How precise the match is: house, street, postcode, city, region or country.
    pub precision: Option<String>,
    /// Look it up with `ForwardOptions::place_id` for the full result.
    pub place_id: Option<String>,
    /// The decoded JSON exactly as the API sent it.
    pub raw: Value,
}

impl Suggestion {
    pub(crate) fn from_value(v: &Value) -> Suggestion {
        Suggestion {
            text: v.get("text").text(),
            lat: v.get("lat").as_f64(),
            lon: v.get("lon").as_f64(),
            r#type: v.get("type").text(),
            precision: v.get("precision").text(),
            place_id: v.get("place_id").text(),
            raw: v.clone(),
        }
    }
}

/// The answer of `autocomplete`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct AutocompleteResponse {
    /// `ok`.
    pub status: Option<String>,
    /// The text that was looked up.
    pub query: Option<String>,
    /// The suggestions, best first.
    pub suggestions: Vec<Suggestion>,
    /// Request id and quota figures from the response headers (empty on an entry inside a batch).
    pub meta: Meta,
    /// The decoded JSON exactly as the API sent it.
    pub raw: Value,
}

impl AutocompleteResponse {
    pub(crate) fn from_value(v: &Value) -> AutocompleteResponse {
        AutocompleteResponse {
            status: v.get("status").text(),
            query: v.get("query").text(),
            suggestions: models(v.get("suggestions"), Suggestion::from_value),
            meta: Meta::default(),
            raw: v.clone(),
        }
    }
}

/// An IPv4 or IPv6 lookup, and one entry of an IP batch. In a batch, an address the API cannot read has the status `error` and an `error` block.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct IpInfo {
    /// `ok`, or `error` on a batch entry the API could not read.
    pub status: Option<String>,
    /// The address.
    pub ip: Option<String>,
    /// The IP version, 4 or 6.
    pub version: Option<i64>,
    /// Whether the address is in the data.
    pub found: Option<bool>,
    /// The network the address belongs to.
    pub prefix: Option<String>,
    /// Country name.
    pub country: Option<String>,
    /// ISO 3166-1 alpha-2 country code.
    pub country_code: Option<String>,
    /// Continent name.
    pub continent: Option<String>,
    /// Continent code.
    pub continent_code: Option<String>,
    /// Region name.
    pub region: Option<String>,
    /// Region code.
    pub region_code: Option<String>,
    /// City.
    pub city: Option<String>,
    /// Postal code.
    pub postcode: Option<String>,
    /// Latitude.
    pub lat: Option<f64>,
    /// Longitude.
    pub lon: Option<f64>,
    /// Metres above sea level, with the `elevation` option.
    pub elevation_m: Option<f64>,
    /// IANA timezone id.
    pub timezone: Option<String>,
    /// UTC offset, such as `-07:00`.
    pub utc_offset: Option<String>,
    /// UTC offset in seconds.
    pub utc_offset_seconds: Option<i64>,
    /// Whether the country is in the European Union.
    pub is_eu: Option<bool>,
    /// Autonomous system number of the network owner.
    pub asn: Option<i64>,
    /// Name of the network owner.
    pub as_name: Option<String>,
    /// Organisation.
    pub org: Option<String>,
    /// What the network is used for.
    pub usage_type: Option<String>,
    /// The regional internet registry.
    pub registry: Option<String>,
    /// When the block was allocated.
    pub allocated: Option<String>,
    /// Tags such as `anycast` or `datacenter`.
    pub tags: Vec<String>,
    /// Anycast network.
    pub is_anycast: Option<bool>,
    /// Satellite network.
    pub is_satellite: Option<bool>,
    /// Data centre network.
    pub is_datacenter: Option<bool>,
    /// Known proxy.
    pub is_proxy: Option<bool>,
    /// Network of a VPN provider.
    pub is_vpn_provider: Option<bool>,
    /// Tor node.
    pub is_tor: Option<bool>,
    /// The crawler block, when the address belongs to a known crawler.
    pub crawler: Option<Value>,
    /// Private or reserved address.
    pub is_private: Option<bool>,
    /// Why a private or reserved address has no data.
    pub reason: Option<String>,
    /// Abuse contact of the network.
    pub abuse_email: Option<String>,
    /// The threat block.
    pub threat: Option<Value>,
    /// When the record was last updated.
    pub updated_at: Option<String>,
    /// On a batch entry the API rejected: `code` and `message`.
    pub error: Option<Value>,
    /// Request id and quota figures from the response headers (empty on an entry inside a batch).
    pub meta: Meta,
    /// The decoded JSON exactly as the API sent it.
    pub raw: Value,
}

impl IpInfo {
    pub(crate) fn from_value(v: &Value) -> IpInfo {
        IpInfo {
            status: v.get("status").text(),
            ip: v.get("ip").text(),
            version: v.get("version").as_i64(),
            found: v.get("found").as_bool(),
            prefix: v.get("prefix").text(),
            country: v.get("country").text(),
            country_code: v.get("country_code").text(),
            continent: v.get("continent").text(),
            continent_code: v.get("continent_code").text(),
            region: v.get("region").text(),
            region_code: v.get("region_code").text(),
            city: v.get("city").text(),
            postcode: v.get("postcode").text(),
            lat: v.get("lat").as_f64(),
            lon: v.get("lon").as_f64(),
            elevation_m: v.get("elevation_m").as_f64(),
            timezone: v.get("timezone").text(),
            utc_offset: v.get("utc_offset").text(),
            utc_offset_seconds: v.get("utc_offset_seconds").as_i64(),
            is_eu: v.get("is_eu").as_bool(),
            asn: v.get("asn").as_i64(),
            as_name: v.get("as_name").text(),
            org: v.get("org").text(),
            usage_type: v.get("usage_type").text(),
            registry: v.get("registry").text(),
            allocated: v.get("allocated").text(),
            tags: texts(v.get("tags")),
            is_anycast: v.get("is_anycast").as_bool(),
            is_satellite: v.get("is_satellite").as_bool(),
            is_datacenter: v.get("is_datacenter").as_bool(),
            is_proxy: v.get("is_proxy").as_bool(),
            is_vpn_provider: v.get("is_vpn_provider").as_bool(),
            is_tor: v.get("is_tor").as_bool(),
            crawler: object(v.get("crawler")),
            is_private: v.get("is_private").as_bool(),
            reason: v.get("reason").text(),
            abuse_email: v.get("abuse_email").text(),
            threat: object(v.get("threat")),
            updated_at: v.get("updated_at").text(),
            error: object(v.get("error")),
            meta: Meta::default(),
            raw: v.clone(),
        }
    }
}

/// The answer of the IP batch methods.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct IpBatchResponse {
    /// `ok`.
    pub status: Option<String>,
    /// One entry per address, in the order sent.
    pub results: Vec<IpInfo>,
    /// Request id and quota figures from the response headers (empty on an entry inside a batch).
    pub meta: Meta,
    /// The decoded JSON exactly as the API sent it.
    pub raw: Value,
}

impl IpBatchResponse {
    pub(crate) fn from_value(v: &Value) -> IpBatchResponse {
        IpBatchResponse {
            status: v.get("status").text(),
            results: models(v.get("results"), IpInfo::from_value),
            meta: Meta::default(),
            raw: v.clone(),
        }
    }
}

/// The next change of UTC offset, such as the start or end of daylight saving time.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct TimezoneTransition {
    /// Unix time of the change.
    pub timestamp: Option<i64>,
    /// The offset after the change.
    pub utc_offset: Option<String>,
    /// The offset after the change, in seconds.
    pub utc_offset_seconds: Option<i64>,
    /// Whether daylight saving time applies after the change.
    pub dst: Option<bool>,
    /// The abbreviation after the change.
    pub abbreviation: Option<String>,
    /// The decoded JSON exactly as the API sent it.
    pub raw: Value,
}

impl TimezoneTransition {
    pub(crate) fn from_value(v: &Value) -> TimezoneTransition {
        TimezoneTransition {
            timestamp: v.get("timestamp").as_i64(),
            utc_offset: v.get("utc_offset").text(),
            utc_offset_seconds: v.get("utc_offset_seconds").as_i64(),
            dst: v.get("dst").as_bool(),
            abbreviation: v.get("abbreviation").text(),
            raw: v.clone(),
        }
    }
}

/// The answer of `timezone`, and one entry of a timezone batch.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct TimezoneInfo {
    /// `ok`.
    pub status: Option<String>,
    /// Latitude.
    pub lat: Option<f64>,
    /// Longitude.
    pub lon: Option<f64>,
    /// IANA timezone id, such as `Asia/Tokyo`.
    pub timezone: Option<String>,
    /// The zone name, such as `Japan Standard Time`.
    pub name: Option<String>,
    /// The abbreviation, such as `JST`.
    pub abbreviation: Option<String>,
    /// The UTC offset, such as `+09:00`.
    pub utc_offset: Option<String>,
    /// The offset in seconds.
    pub utc_offset_seconds: Option<i64>,
    /// The offset without daylight saving time.
    pub raw_offset_seconds: Option<i64>,
    /// Whether daylight saving time applies.
    pub dst: Option<bool>,
    /// The daylight saving part of the offset.
    pub dst_offset_seconds: Option<i64>,
    /// The Unix time answered for.
    pub timestamp: Option<i64>,
    /// Local time, ISO 8601.
    pub local_time: Option<String>,
    /// ISO 3166-1 alpha-2 country code.
    pub country_code: Option<String>,
    /// Whether the point is at sea.
    pub is_ocean: Option<bool>,
    /// The Windows zone id.
    pub windows_timezone: Option<String>,
    /// The next change of offset.
    pub next_transition: Option<TimezoneTransition>,
    /// Request id and quota figures from the response headers (empty on an entry inside a batch).
    pub meta: Meta,
    /// The decoded JSON exactly as the API sent it.
    pub raw: Value,
}

impl TimezoneInfo {
    pub(crate) fn from_value(v: &Value) -> TimezoneInfo {
        TimezoneInfo {
            status: v.get("status").text(),
            lat: v.get("lat").as_f64(),
            lon: v.get("lon").as_f64(),
            timezone: v.get("timezone").text(),
            name: v.get("name").text(),
            abbreviation: v.get("abbreviation").text(),
            utc_offset: v.get("utc_offset").text(),
            utc_offset_seconds: v.get("utc_offset_seconds").as_i64(),
            raw_offset_seconds: v.get("raw_offset_seconds").as_i64(),
            dst: v.get("dst").as_bool(),
            dst_offset_seconds: v.get("dst_offset_seconds").as_i64(),
            timestamp: v.get("timestamp").as_i64(),
            local_time: v.get("local_time").text(),
            country_code: v.get("country_code").text(),
            is_ocean: v.get("is_ocean").as_bool(),
            windows_timezone: v.get("windows_timezone").text(),
            next_transition: model(v.get("next_transition"), TimezoneTransition::from_value),
            meta: Meta::default(),
            raw: v.clone(),
        }
    }
}

/// The answer of `timezone_batch`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct TimezoneBatchResponse {
    /// `ok`.
    pub status: Option<String>,
    /// The Unix time answered for.
    pub timestamp: Option<i64>,
    /// One entry per point, in the order sent.
    pub results: Vec<TimezoneInfo>,
    /// Request id and quota figures from the response headers (empty on an entry inside a batch).
    pub meta: Meta,
    /// The decoded JSON exactly as the API sent it.
    pub raw: Value,
}

impl TimezoneBatchResponse {
    pub(crate) fn from_value(v: &Value) -> TimezoneBatchResponse {
        TimezoneBatchResponse {
            status: v.get("status").text(),
            timestamp: v.get("timestamp").as_i64(),
            results: models(v.get("results"), TimezoneInfo::from_value),
            meta: Meta::default(),
            raw: v.clone(),
        }
    }
}

/// The height of one point.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ElevationPoint {
    /// Latitude.
    pub lat: Option<f64>,
    /// Longitude.
    pub lon: Option<f64>,
    /// Metres above sea level (negative for the sea floor), `None` where there is no data.
    pub elevation_m: Option<f64>,
    /// The size of the grid cell, in metres.
    pub resolution_m: Option<f64>,
    /// The decoded JSON exactly as the API sent it.
    pub raw: Value,
}

impl ElevationPoint {
    pub(crate) fn from_value(v: &Value) -> ElevationPoint {
        ElevationPoint {
            lat: v.get("lat").as_f64(),
            lon: v.get("lon").as_f64(),
            elevation_m: v.get("elevation_m").as_f64(),
            resolution_m: v.get("resolution_m").as_f64(),
            raw: v.clone(),
        }
    }
}

/// The answer of the elevation methods. It always has `results`, one entry for a single point.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ElevationResponse {
    /// `ok`.
    pub status: Option<String>,
    /// One entry per point, in order.
    pub results: Vec<ElevationPoint>,
    /// Request id and quota figures from the response headers (empty on an entry inside a batch).
    pub meta: Meta,
    /// The decoded JSON exactly as the API sent it.
    pub raw: Value,
}

impl ElevationResponse {
    pub(crate) fn from_value(v: &Value) -> ElevationResponse {
        ElevationResponse {
            status: v.get("status").text(),
            results: models(v.get("results"), ElevationPoint::from_value),
            meta: Meta::default(),
            raw: v.clone(),
        }
    }
}

/// One place that uses a postal code.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct PostcodePlace {
    /// The postal code, as text.
    pub postcode: Option<String>,
    /// The place name.
    pub place: Option<String>,
    /// Region name.
    pub region: Option<String>,
    /// Region code.
    pub region_code: Option<String>,
    /// Country name.
    pub country: Option<String>,
    /// ISO 3166-1 alpha-2 country code.
    pub country_code: Option<String>,
    /// Latitude.
    pub lat: Option<f64>,
    /// Longitude.
    pub lon: Option<f64>,
    /// The decoded JSON exactly as the API sent it.
    pub raw: Value,
}

impl PostcodePlace {
    pub(crate) fn from_value(v: &Value) -> PostcodePlace {
        PostcodePlace {
            postcode: v.get("postcode").text(),
            place: v.get("place").text(),
            region: v.get("region").text(),
            region_code: v.get("region_code").text(),
            country: v.get("country").text(),
            country_code: v.get("country_code").text(),
            lat: v.get("lat").as_f64(),
            lon: v.get("lon").as_f64(),
            raw: v.clone(),
        }
    }
}

/// The answer of `postcode`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct PostcodeResponse {
    /// `ok`.
    pub status: Option<String>,
    /// The code that was looked up, as text.
    pub postcode: Option<String>,
    /// The country it was matched in.
    pub country_code: Option<String>,
    /// Every place that uses the code.
    pub results: Vec<PostcodePlace>,
    /// Request id and quota figures from the response headers (empty on an entry inside a batch).
    pub meta: Meta,
    /// The decoded JSON exactly as the API sent it.
    pub raw: Value,
}

impl PostcodeResponse {
    pub(crate) fn from_value(v: &Value) -> PostcodeResponse {
        PostcodeResponse {
            status: v.get("status").text(),
            postcode: v.get("postcode").text(),
            country_code: v.get("country_code").text(),
            results: models(v.get("results"), PostcodePlace::from_value),
            meta: Meta::default(),
            raw: v.clone(),
        }
    }
}
