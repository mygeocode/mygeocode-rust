//! One options type per method. Every setter takes the options by value and returns them, so they chain:
//! `ForwardOptions::new().limit(3).countries(["de", "at"])`. An option a method does not take does not compile.

use crate::json::format_number;

/// A latitude and longitude in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    /// Latitude, -90 to 90.
    pub lat: f64,
    /// Longitude, -180 to 180.
    pub lon: f64,
}

impl Point {
    /// Creates a point.
    pub fn new(lat: f64, lon: f64) -> Point {
        Point { lat, lon }
    }
}

impl From<(f64, f64)> for Point {
    fn from(p: (f64, f64)) -> Point {
        Point::new(p.0, p.1)
    }
}

impl From<[f64; 2]> for Point {
    fn from(p: [f64; 2]) -> Point {
        Point::new(p[0], p[1])
    }
}

pub(crate) fn pair(p: Point) -> String {
    format!("{},{}", format_number(p.lat), format_number(p.lon))
}

pub(crate) fn points<P: Into<Point> + Copy>(list: &[P]) -> String {
    list.iter()
        .map(|p| pair((*p).into()))
        .collect::<Vec<_>>()
        .join("|")
}

fn join<I, S>(items: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    items
        .into_iter()
        .map(|s| s.as_ref().to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn bounds(south: f64, west: f64, north: f64, east: f64) -> String {
    [south, west, north, east]
        .iter()
        .map(|v| format_number(*v))
        .collect::<Vec<_>>()
        .join(",")
}

/// The values of an options type, in the order they were set, under the API's names.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Values {
    pub(crate) values: Vec<(&'static str, String)>,
    pub(crate) params: Vec<(String, String)>,
}

impl Values {
    fn set(&mut self, name: &'static str, value: Option<String>) {
        self.values.retain(|(k, _)| *k != name);
        if let Some(v) = value {
            self.values.push((name, v));
        }
    }

    fn flag(&mut self, name: &'static str, on: bool) {
        self.set(name, if on { Some("1".to_string()) } else { None });
    }

    fn param(&mut self, name: String, value: String) {
        self.params.retain(|(k, _)| *k != name);
        self.params.push((name, value));
    }

    pub(crate) fn get(&self, name: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v.as_str())
    }
}

macro_rules! common {
    ($t:ident) => {
        impl $t {
            /// Empty options.
            pub fn new() -> $t {
                $t::default()
            }

            /// Asks the API for indented JSON. It changes nothing in the result you get.
            pub fn pretty(mut self, on: bool) -> $t {
                self.0.flag("pretty", on);
                self
            }

            /// One more query parameter, sent as given and winning over the options above, so a parameter the API
            /// adds later works before this crate knows it.
            pub fn param(mut self, name: impl Into<String>, value: impl ToString) -> $t {
                self.0.param(name.into(), value.to_string());
                self
            }

            /// More query parameters, as [`param`](Self::param) does for each pair.
            pub fn params<I, K, V>(mut self, pairs: I) -> $t
            where
                I: IntoIterator<Item = (K, V)>,
                K: Into<String>,
                V: ToString,
            {
                for (k, v) in pairs {
                    self.0.param(k.into(), v.to_string());
                }
                self
            }

            pub(crate) fn values(&self) -> &Values {
                &self.0
            }
        }
    };
}

macro_rules! search {
    ($t:ident) => {
        impl $t {
            /// How many results, 1 to 10 (default 5).
            pub fn limit(mut self, limit: u32) -> $t {
                self.0.set("limit", Some(limit.to_string()));
                self
            }

            /// Only search in these countries: ISO 3166-1 alpha-2 codes, at most 10.
            pub fn countries<I, S>(mut self, codes: I) -> $t
            where
                I: IntoIterator<Item = S>,
                S: AsRef<str>,
            {
                let text = join(codes);
                self.0
                    .set("countries", if text.is_empty() { None } else { Some(text) });
                self
            }

            /// Prefer results inside this box (with [`strict`](Self::strict), only results inside it).
            pub fn bounds(mut self, south: f64, west: f64, north: f64, east: f64) -> $t {
                self.0.set("bounds", Some(bounds(south, west, north, east)));
                self
            }

            /// Leave out results outside `bounds`.
            pub fn strict(mut self, on: bool) -> $t {
                self.0.flag("strict", on);
                self
            }

            /// Rank results near this point first.
            pub fn proximity(mut self, point: impl Into<Point>) -> $t {
                self.0.set("proximity", Some(pair(point.into())));
                self
            }

            /// Only keep these kinds of result: address, street, postcode, city, region, country, poi.
            pub fn types<I, S>(mut self, types: I) -> $t
            where
                I: IntoIterator<Item = S>,
                S: AsRef<str>,
            {
                let text = join(types);
                self.0
                    .set("types", if text.is_empty() { None } else { Some(text) });
                self
            }

            /// Language of the results for this call (default: the client's, English unless set).
            pub fn lang(mut self, lang: impl Into<String>) -> $t {
                self.0.set("lang", Some(lang.into()));
                self
            }
        }
    };
}

macro_rules! annotations {
    ($t:ident) => {
        impl $t {
            /// Adds every extra data block at once.
            pub fn annotations(mut self, on: bool) -> $t {
                self.0.flag("annotations", on);
                self
            }

            /// Adds the timezone block.
            pub fn timezone(mut self, on: bool) -> $t {
                self.0.flag("timezone", on);
                self
            }

            /// Adds `elevation_m`.
            pub fn elevation(mut self, on: bool) -> $t {
                self.0.flag("elevation", on);
                self
            }

            /// Adds the Wikidata id.
            pub fn wikidata(mut self, on: bool) -> $t {
                self.0.flag("wikidata", on);
                self
            }

            /// Adds the country block.
            pub fn country_info(mut self, on: bool) -> $t {
                self.0.flag("country_info", on);
                self
            }

            /// Adds the codes block: plus code, geohash, MGRS and more.
            pub fn codes(mut self, on: bool) -> $t {
                self.0.flag("codes", on);
                self
            }

            /// Adds sunrise and sunset.
            pub fn sun(mut self, on: bool) -> $t {
                self.0.flag("sun", on);
                self
            }

            /// Adds the regions block.
            pub fn regions(mut self, on: bool) -> $t {
                self.0.flag("regions", on);
                self
            }

            /// Adds the districts block.
            pub fn districts(mut self, on: bool) -> $t {
                self.0.flag("districts", on);
                self
            }

            /// Adds the census block.
            pub fn census(mut self, on: bool) -> $t {
                self.0.flag("census", on);
                self
            }

            /// Adds the demographics block.
            pub fn demographics(mut self, on: bool) -> $t {
                self.0.flag("demographics", on);
                self
            }

            /// Adds the road block.
            pub fn road(mut self, on: bool) -> $t {
                self.0.flag("road", on);
                self
            }

            /// Adds the site block.
            pub fn site(mut self, on: bool) -> $t {
                self.0.flag("site", on);
                self
            }
        }
    };
}

macro_rules! points_and_grid {
    ($t:ident) => {
        impl $t {
            /// Several points at once.
            pub fn locations<P: Into<Point> + Copy>(mut self, list: &[P]) -> $t {
                self.0.set("locations", Some(points(list)));
                self
            }

            /// Several points at once, as the API's own text: `"lat,lon|lat,lon"` or an encoded polyline `"enc:..."`.
            pub fn locations_text(mut self, text: impl Into<String>) -> $t {
                self.0.set("locations", Some(text.into()));
                self
            }

            /// Points along a path, sampled [`samples`](Self::samples) times.
            pub fn path<P: Into<Point> + Copy>(mut self, list: &[P]) -> $t {
                self.0.set("path", Some(points(list)));
                self
            }

            /// A path as the API's own text: `"lat,lon|lat,lon"` or an encoded polyline `"enc:..."`.
            pub fn path_text(mut self, text: impl Into<String>) -> $t {
                self.0.set("path", Some(text.into()));
                self
            }

            /// How many points to sample along the path, 2 to 100.
            pub fn samples(mut self, samples: u32) -> $t {
                self.0.set("samples", Some(samples.to_string()));
                self
            }

            /// A grid over this box, with [`rows`](Self::rows) and [`cols`](Self::cols).
            pub fn bounds(mut self, south: f64, west: f64, north: f64, east: f64) -> $t {
                self.0.set("bounds", Some(bounds(south, west, north, east)));
                self
            }

            /// Rows of the grid.
            pub fn rows(mut self, rows: u32) -> $t {
                self.0.set("rows", Some(rows.to_string()));
                self
            }

            /// Columns of the grid.
            pub fn cols(mut self, cols: u32) -> $t {
                self.0.set("cols", Some(cols.to_string()));
                self
            }
        }
    };
}

/// Options of `forward`: the search filters, the extra data blocks, and a structured address or a place id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ForwardOptions(Values);
common!(ForwardOptions);
search!(ForwardOptions);
annotations!(ForwardOptions);

impl ForwardOptions {
    /// Structured search: the street with the house number, such as `"10 Downing Street"`.
    pub fn street(mut self, street: impl Into<String>) -> ForwardOptions {
        self.0.set("street", Some(street.into()));
        self
    }

    /// Structured search: the city.
    pub fn city(mut self, city: impl Into<String>) -> ForwardOptions {
        self.0.set("city", Some(city.into()));
        self
    }

    /// Structured search: the state, province or region.
    pub fn state(mut self, state: impl Into<String>) -> ForwardOptions {
        self.0.set("state", Some(state.into()));
        self
    }

    /// Structured search: the postal code.
    pub fn postcode(mut self, postcode: impl Into<String>) -> ForwardOptions {
        self.0.set("postcode", Some(postcode.into()));
        self
    }

    /// Structured search: the country, an ISO 3166-1 alpha-2 code.
    pub fn country(mut self, country: impl Into<String>) -> ForwardOptions {
        self.0.set("country", Some(country.into()));
        self
    }

    /// Look up a `place_id` from an earlier result.
    pub fn place_id(mut self, place_id: impl Into<String>) -> ForwardOptions {
        self.0.set("place_id", Some(place_id.into()));
        self
    }
}

/// Options of `forward_batch`. They apply to every search in the batch.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ForwardBatchOptions(Values);
common!(ForwardBatchOptions);
search!(ForwardBatchOptions);
annotations!(ForwardBatchOptions);

/// Options of `reverse` and `reverse_batch`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReverseOptions(Values);
common!(ReverseOptions);
annotations!(ReverseOptions);

impl ReverseOptions {
    /// How fine the answer is: address (default), street, postcode, city, region or country.
    pub fn level(mut self, level: impl Into<String>) -> ReverseOptions {
        self.0.set("level", Some(level.into()));
        self
    }

    /// How far to look for an address, 1 to 5000 metres.
    pub fn radius(mut self, metres: u32) -> ReverseOptions {
        self.0.set("radius", Some(metres.to_string()));
        self
    }

    /// Language of the result for this call (default: the client's, English unless set).
    pub fn lang(mut self, lang: impl Into<String>) -> ReverseOptions {
        self.0.set("lang", Some(lang.into()));
        self
    }

    /// Adds the administrative boundaries the point falls in (Australia).
    pub fn boundaries(mut self, on: bool) -> ReverseOptions {
        self.0.flag("boundaries", on);
        self
    }
}

/// Options of `autocomplete`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AutocompleteOptions(Values);
common!(AutocompleteOptions);
search!(AutocompleteOptions);

/// Options of the IP methods and their batch forms.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IpOptions(Values);
common!(IpOptions);

impl IpOptions {
    /// Adds `elevation_m`.
    pub fn elevation(mut self, on: bool) -> IpOptions {
        self.0.flag("elevation", on);
        self
    }
}

/// Options of `timezone` and `timezone_with`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TimezoneOptions(Values);
common!(TimezoneOptions);
points_and_grid!(TimezoneOptions);

impl TimezoneOptions {
    /// Look up a zone by its IANA or Windows id instead of a point (with `timezone_with`).
    pub fn tz(mut self, zone: impl Into<String>) -> TimezoneOptions {
        self.0.set("tz", Some(zone.into()));
        self
    }

    /// The moment to answer for, Unix time in seconds (default: now).
    pub fn timestamp(mut self, unix_seconds: i64) -> TimezoneOptions {
        self.0.set("timestamp", Some(unix_seconds.to_string()));
        self
    }

    /// Language of the zone name. Sent as the API's `language`, and only when set here or on the client.
    pub fn lang(mut self, lang: impl Into<String>) -> TimezoneOptions {
        self.0.set("lang", Some(lang.into()));
        self
    }

    /// The same as [`lang`](Self::lang), under the API's own name.
    pub fn language(mut self, language: impl Into<String>) -> TimezoneOptions {
        self.0.set("language", Some(language.into()));
        self
    }
}

/// Options of `timezone_batch`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TimezoneBatchOptions(Values);
common!(TimezoneBatchOptions);

impl TimezoneBatchOptions {
    /// The moment to answer for, Unix time in seconds (default: now).
    pub fn timestamp(mut self, unix_seconds: i64) -> TimezoneBatchOptions {
        self.0.set("timestamp", Some(unix_seconds.to_string()));
        self
    }

    /// Language of the zone names. Sent as the API's `language`, and only when set here or on the client.
    pub fn lang(mut self, lang: impl Into<String>) -> TimezoneBatchOptions {
        self.0.set("lang", Some(lang.into()));
        self
    }

    /// The same as [`lang`](Self::lang), under the API's own name.
    pub fn language(mut self, language: impl Into<String>) -> TimezoneBatchOptions {
        self.0.set("language", Some(language.into()));
        self
    }
}

/// Options of `elevation` and `elevation_with`: several points, a path or a grid.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ElevationOptions(Values);
common!(ElevationOptions);
points_and_grid!(ElevationOptions);

/// Options of `elevation_batch`: only `pretty` and `params`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ElevationBatchOptions(Values);
common!(ElevationBatchOptions);

/// Options of `postcode`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PostcodeOptions(Values);
common!(PostcodeOptions);

impl PostcodeOptions {
    /// Only match the code in this country (ISO 3166-1 alpha-2). Send it whenever you know it.
    pub fn country(mut self, country: impl Into<String>) -> PostcodeOptions {
        self.0.set("country", Some(country.into()));
        self
    }
}
