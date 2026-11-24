//! Every documented parameter is an option, sent under the API's own name.

mod common;

use std::collections::BTreeMap;

use common::*;
use mygeocode::*;

fn q(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn forward_every_parameter() {
    let rec = Recorder::of(replay("forward"));
    let o = ForwardOptions::new()
        .street("Pariser Platz")
        .city("Berlin")
        .state("Berlin")
        .postcode("10117")
        .country("DE")
        .place_id("abc")
        .limit(3)
        .countries(["de", "at"])
        .bounds(52.3, 13.0, 52.7, 13.8)
        .strict(true)
        .proximity((52.5, 13.4))
        .types(["poi", "address"])
        .lang("de")
        .annotations(true)
        .pretty(true);
    Setup::new()
        .client(&rec)
        .forward("Brandenburg Gate, Berlin", &o)
        .unwrap();
    assert_eq!(
        rec.last().query,
        q(&[
            ("q", "Brandenburg Gate, Berlin"),
            ("street", "Pariser Platz"),
            ("city", "Berlin"),
            ("state", "Berlin"),
            ("postcode", "10117"),
            ("country", "DE"),
            ("place_id", "abc"),
            ("limit", "3"),
            ("countries", "de,at"),
            ("bounds", "52.3,13,52.7,13.8"),
            ("strict", "1"),
            ("proximity", "52.5,13.4"),
            ("types", "poi,address"),
            ("lang", "de"),
            ("annotations", "1"),
            ("pretty", "1"),
        ])
    );
}

macro_rules! flag {
    ($o:expr, $name:expr) => {
        match $name {
            "timezone" => $o.timezone(true),
            "elevation" => $o.elevation(true),
            "wikidata" => $o.wikidata(true),
            "country_info" => $o.country_info(true),
            "codes" => $o.codes(true),
            "sun" => $o.sun(true),
            "regions" => $o.regions(true),
            "districts" => $o.districts(true),
            "census" => $o.census(true),
            "demographics" => $o.demographics(true),
            "road" => $o.road(true),
            "site" => $o.site(true),
            _ => $o.annotations(true),
        }
    };
}

#[test]
fn flags_on_forward_reverse_and_their_batches() {
    for name in [
        "timezone",
        "elevation",
        "wikidata",
        "country_info",
        "codes",
        "sun",
        "regions",
        "districts",
        "census",
        "demographics",
        "road",
        "site",
        "annotations",
    ] {
        let rec = Recorder::of(replay("forward"));
        let c = Setup::new().client(&rec);
        c.forward(
            "Brandenburg Gate, Berlin",
            &flag!(ForwardOptions::new(), name),
        )
        .unwrap();
        assert_eq!(rec.last().query[name], "1", "forward {}", name);
        c.reverse(EIFFEL.lat, EIFFEL.lon, &flag!(ReverseOptions::new(), name))
            .unwrap();
        assert_eq!(rec.last().query[name], "1", "reverse {}", name);
        c.forward_batch(
            &["Brandenburg Gate, Berlin"],
            &flag!(ForwardBatchOptions::new(), name),
        )
        .unwrap();
        assert_eq!(rec.last().query[name], "1", "forward_batch {}", name);
        c.reverse_batch(&[EIFFEL], &flag!(ReverseOptions::new(), name))
            .unwrap();
        assert_eq!(rec.last().query[name], "1", "reverse_batch {}", name);
    }
    let rec = Recorder::of(replay("forward"));
    Setup::new()
        .client(&rec)
        .forward(
            "Brandenburg Gate, Berlin",
            &ForwardOptions::new().timezone(true).timezone(false),
        )
        .unwrap();
    assert!(
        !rec.last().query.contains_key("timezone"),
        "a flag set back to false is left out"
    );
}

#[test]
fn reverse_every_parameter() {
    let rec = Recorder::of(replay("reverse"));
    let o = ReverseOptions::new()
        .level("city")
        .radius(500)
        .lang("fr")
        .boundaries(true)
        .annotations(true)
        .pretty(true);
    Setup::new()
        .client(&rec)
        .reverse(EIFFEL.lat, EIFFEL.lon, &o)
        .unwrap();
    assert_eq!(
        rec.last().query,
        q(&[
            ("lat", "48.8584"),
            ("lon", "2.2945"),
            ("level", "city"),
            ("radius", "500"),
            ("lang", "fr"),
            ("boundaries", "1"),
            ("annotations", "1"),
            ("pretty", "1")
        ])
    );
}

#[test]
fn autocomplete_every_parameter() {
    let rec = Recorder::of(replay("autocomplete"));
    let o = AutocompleteOptions::new()
        .limit(5)
        .countries(["us"])
        .proximity(Point::new(38.9, -77.0))
        .bounds(38.0, -78.0, 39.0, -76.0)
        .strict(true)
        .types(["address"])
        .lang("en")
        .pretty(true);
    Setup::new()
        .client(&rec)
        .autocomplete("1600 Pennsylvania", &o)
        .unwrap();
    assert_eq!(
        rec.last().query,
        q(&[
            ("q", "1600 Pennsylvania"),
            ("limit", "5"),
            ("countries", "us"),
            ("proximity", "38.9,-77"),
            ("bounds", "38,-78,39,-76"),
            ("strict", "1"),
            ("types", "address"),
            ("lang", "en"),
            ("pretty", "1")
        ])
    );
}

#[test]
fn ip_parameters() {
    let rec = Recorder::of(replay("ip"));
    let c = Setup::new().client(&rec);
    c.ipv4(
        Some("8.8.8.8"),
        &IpOptions::new().elevation(true).pretty(true),
    )
    .unwrap();
    assert_eq!(
        rec.last().query,
        q(&[("ip", "8.8.8.8"), ("elevation", "1"), ("pretty", "1")])
    );
    c.ipv6_batch(
        &["2001:4860:4860::8888", "2606:4700:4700::1111"],
        &IpOptions::new().elevation(true),
    )
    .unwrap();
    assert_eq!(
        rec.last().query,
        q(&[
            ("ips", "2001:4860:4860::8888,2606:4700:4700::1111"),
            ("elevation", "1")
        ])
    );
}

#[test]
fn timezone_parameters() {
    let rec = Recorder::of(replay("timezone"));
    let c = Setup::new().client(&rec);
    c.timezone(
        35.6762,
        139.6503,
        &TimezoneOptions::new()
            .timestamp(1767225600)
            .lang("ja")
            .pretty(true),
    )
    .unwrap();
    assert_eq!(
        rec.last().query,
        q(&[
            ("lat", "35.6762"),
            ("lon", "139.6503"),
            ("timestamp", "1767225600"),
            ("language", "ja"),
            ("pretty", "1")
        ])
    );
    c.timezone_with(&TimezoneOptions::new().locations(&[EIFFEL, LIBERTY]))
        .unwrap();
    assert_eq!(
        rec.last().query,
        q(&[("locations", "48.8584,2.2945|40.6892,-74.0445")])
    );
    c.timezone_with(&TimezoneOptions::new().path_text("enc:abc").samples(10))
        .unwrap();
    assert_eq!(
        rec.last().query,
        q(&[("path", "enc:abc"), ("samples", "10")])
    );
    c.timezone_with(
        &TimezoneOptions::new()
            .bounds(40.0, -75.0, 41.0, -73.0)
            .rows(3)
            .cols(4),
    )
    .unwrap();
    assert_eq!(
        rec.last().query,
        q(&[("bounds", "40,-75,41,-73"), ("rows", "3"), ("cols", "4")])
    );
    c.timezone_batch(
        &[EIFFEL],
        &TimezoneBatchOptions::new().timestamp(1767225600).lang("de"),
    )
    .unwrap();
    assert_eq!(
        rec.last().query,
        q(&[
            ("locations", "48.8584,2.2945"),
            ("timestamp", "1767225600"),
            ("language", "de")
        ])
    );
    c.timezone_batch_text("enc:abc", &TimezoneBatchOptions::new())
        .unwrap();
    assert_eq!(rec.last().query, q(&[("locations", "enc:abc")]));
}

#[test]
fn elevation_parameters() {
    let rec = Recorder::of(replay("elevation"));
    let c = Setup::new().client(&rec);
    c.elevation_with(
        &ElevationOptions::new()
            .locations(&[EIFFEL, LIBERTY])
            .pretty(true),
    )
    .unwrap();
    assert_eq!(
        rec.last().query,
        q(&[
            ("locations", "48.8584,2.2945|40.6892,-74.0445"),
            ("pretty", "1")
        ])
    );
    c.elevation_with(&ElevationOptions::new().path(&[EIFFEL, LIBERTY]).samples(5))
        .unwrap();
    assert_eq!(
        rec.last().query,
        q(&[
            ("path", "48.8584,2.2945|40.6892,-74.0445"),
            ("samples", "5")
        ])
    );
    c.elevation_with(
        &ElevationOptions::new()
            .bounds(27.0, 86.0, 28.0, 87.0)
            .rows(2)
            .cols(2),
    )
    .unwrap();
    assert_eq!(
        rec.last().query,
        q(&[("bounds", "27,86,28,87"), ("rows", "2"), ("cols", "2")])
    );
}

#[test]
fn postcode_parameters() {
    let rec = Recorder::of(replay("postcode"));
    Setup::new()
        .client(&rec)
        .postcode("75008", &PostcodeOptions::new().country("FR").pretty(true))
        .unwrap();
    assert_eq!(
        rec.last().query,
        q(&[("code", "75008"), ("country", "FR"), ("pretty", "1")])
    );
}

#[test]
fn batch_options_go_in_the_query_string() {
    let rec = Recorder::of(replay("forward_batch"));
    let c = Setup::new().client(&rec);
    let o = ForwardBatchOptions::new()
        .limit(1)
        .countries(vec!["de".to_string()])
        .bounds(52.0, 13.0, 53.0, 14.0)
        .strict(true)
        .proximity([52.5, 13.4])
        .types(["poi"])
        .lang("fr")
        .annotations(true);
    c.forward_batch(&["Brandenburg Gate, Berlin"], &o).unwrap();
    assert_eq!(
        rec.last().json(),
        json(r#"{"queries":["Brandenburg Gate, Berlin"]}"#)
    );
    assert_eq!(
        rec.last().query,
        q(&[
            ("limit", "1"),
            ("countries", "de"),
            ("bounds", "52,13,53,14"),
            ("strict", "1"),
            ("proximity", "52.5,13.4"),
            ("types", "poi"),
            ("lang", "fr"),
            ("annotations", "1")
        ])
    );
    assert_eq!(rec.last().header("content-type"), Some("application/json"));
    c.reverse_batch(
        &[EIFFEL],
        &ReverseOptions::new()
            .level("street")
            .radius(100)
            .boundaries(true),
    )
    .unwrap();
    assert_eq!(
        rec.last().json(),
        json("[[48.8584,2.2945]]")
            .as_array()
            .map(|a| Value::Object(vec![("coords".to_string(), Value::Array(a.to_vec()))]))
            .unwrap()
    );
    assert_eq!(
        rec.last().query,
        q(&[
            ("level", "street"),
            ("radius", "100"),
            ("lang", "en"),
            ("boundaries", "1")
        ])
    );
    c.elevation_batch(&[EIFFEL], &ElevationBatchOptions::new().pretty(true))
        .unwrap();
    assert_eq!(
        rec.last()
            .json()
            .get("locations")
            .as_array()
            .map(|a| a.len()),
        Some(1)
    );
    assert_eq!(rec.last().query, q(&[("pretty", "1")]));
}

#[test]
fn params_pass_through_and_win() {
    let rec = Recorder::of(replay("forward"));
    let c = Setup::new().client(&rec);
    c.forward(
        "Brandenburg Gate, Berlin",
        &ForwardOptions::new()
            .limit(2)
            .param("limit", 5)
            .params(vec![("brand_new", "x")]),
    )
    .unwrap();
    assert_eq!(rec.last().query["limit"], "5");
    assert_eq!(rec.last().query["brand_new"], "x");
    c.forward(
        "Brandenburg Gate, Berlin",
        &ForwardOptions::new().param("lang", "ja"),
    )
    .unwrap();
    assert_eq!(
        rec.last().query["lang"],
        "ja",
        "params win over the client language"
    );
}

#[test]
fn text_is_url_encoded() {
    let rec = Recorder::of(replay("forward"));
    Setup::new()
        .client(&rec)
        .forward(
            "Brandenburg Gate, Berlin & more/\u{fc}",
            &ForwardOptions::new(),
        )
        .unwrap();
    assert!(
        rec.last()
            .url
            .contains("q=Brandenburg+Gate%2C+Berlin+%26+more%2F%C3%BC"),
        "{}",
        rec.last().url
    );
    assert_eq!(
        rec.last().query["q"],
        "Brandenburg Gate, Berlin & more/\u{fc}"
    );
}
