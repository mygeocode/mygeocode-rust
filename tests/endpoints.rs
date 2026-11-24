//! Every method sends the shared example values, the request must equal the recorded one, and the answer must be read
//! into the right fields (the recordings in tests/fixtures are real answers of the API).

mod common;

use common::*;
use mygeocode::blocking::Client;
use mygeocode::*;

fn client(rec: &Recorder) -> Client {
    Setup::new().client(rec)
}

#[test]
fn forward() {
    let rec = Recorder::of(replay("forward"));
    let r = client(&rec)
        .forward("Brandenburg Gate, Berlin", &ForwardOptions::new())
        .unwrap();
    assert_sent_like("forward", &rec.last());
    let want = body("forward");
    let want = &want.get("results").as_array().unwrap()[0];
    assert_eq!(r.status.as_deref(), Some("ok"));
    let p = &r.results[0];
    assert_eq!(
        p.formatted.as_deref(),
        Some("Brandenburg Gate, State of Berlin, Germany")
    );
    assert_eq!(p.lat, want.get("lat").as_f64());
    assert_eq!(p.lon, want.get("lon").as_f64());
    let c = p.components.as_ref().unwrap();
    assert_eq!(c.country_code.as_deref(), Some("de"));
    assert_eq!(c.poi.as_deref(), Some("Brandenburg Gate"));
    assert_eq!(c.city, None);
    assert_eq!(r.raw, body("forward"));
}

#[test]
fn forward_meta_comes_from_the_headers() {
    let r = client(&Recorder::of(replay("forward")))
        .forward("Brandenburg Gate, Berlin", &ForwardOptions::new())
        .unwrap();
    let h = recording("forward");
    let h = h.get("headers");
    let n = |name: &str| h.get(name).as_str().and_then(|v| v.parse::<i64>().ok());
    assert_eq!(r.meta.request_id.as_deref(), h.get("x-request-id").as_str());
    assert_eq!(r.meta.quota_limit, n("x-quota-limit"));
    assert_eq!(r.meta.quota_used, n("x-quota-used"));
    assert_eq!(r.meta.quota_free_remaining, n("x-quota-free-remaining"));
    assert_eq!(r.meta.quota_reset, n("x-quota-reset"));
    assert_eq!(r.meta.credits_remaining, None);
}

#[test]
fn forward_structured() {
    let rec = Recorder::of(replay("forward_structured"));
    let o = ForwardOptions::new()
        .street("10 Downing Street")
        .city("London")
        .country("GB");
    let r = client(&rec).forward("", &o).unwrap();
    assert_sent_like("forward_structured", &rec.last());
    assert_eq!(
        r.results[0].formatted.as_deref(),
        Some("Downing Street, London SW1A, United Kingdom")
    );
}

#[test]
fn reverse() {
    let rec = Recorder::of(replay("reverse"));
    let r = client(&rec)
        .reverse(EIFFEL.lat, EIFFEL.lon, &ReverseOptions::new())
        .unwrap();
    assert_sent_like("reverse", &rec.last());
    let p = r.result.unwrap();
    assert_eq!(
        p.formatted.as_deref(),
        Some("5 Avenue Anatole France, 75007 Paris, France")
    );
    assert_eq!(p.distance_m, Some(47));
    assert_eq!(p.components.unwrap().postcode.as_deref(), Some("75007"));
}

#[test]
fn autocomplete() {
    let rec = Recorder::of(replay("autocomplete"));
    let r = client(&rec)
        .autocomplete("1600 Pennsylvania", &AutocompleteOptions::new())
        .unwrap();
    assert_sent_like("autocomplete", &rec.last());
    assert_eq!(
        r.suggestions[0].text.as_deref(),
        Some("1600 Pennsylvania Avenue NW, Washington, DC 20500, United States")
    );
}

#[test]
fn ip_ipv4_ipv6() {
    let rec = Recorder::of(replay("ip"));
    let r = client(&rec).ip(Some("8.8.8.8"), &IpOptions::new()).unwrap();
    assert_sent_like("ip", &rec.last());
    assert_eq!(
        (
            r.country.as_deref(),
            r.city.as_deref(),
            r.asn,
            r.is_datacenter
        ),
        (
            Some("United States"),
            Some("Mountain View"),
            Some(15169),
            Some(true)
        )
    );
    let rec = Recorder::of(replay("ipv4"));
    assert_eq!(
        client(&rec)
            .ipv4(Some("8.8.8.8"), &IpOptions::new())
            .unwrap()
            .as_name
            .as_deref(),
        Some("Google LLC")
    );
    assert_sent_like("ipv4", &rec.last());
    let rec = Recorder::of(replay("ipv6"));
    let six = client(&rec)
        .ipv6(Some("2001:4860:4860::8888"), &IpOptions::new())
        .unwrap();
    assert_sent_like("ipv6", &rec.last());
    assert_eq!(
        (six.ip.as_deref(), six.version),
        (Some("2001:4860:4860::8888"), Some(6))
    );
}

#[test]
fn ip_of_the_caller_sends_no_address() {
    let rec = Recorder::of(replay("ip"));
    let c = client(&rec);
    c.ip(None, &IpOptions::new()).unwrap();
    assert!(!rec.last().query.contains_key("ip"));
    assert_eq!(rec.last().path, "/v1/ip");
    c.ipv4(None, &IpOptions::new()).unwrap();
    assert_eq!(rec.last().path, "/v1/ipv4");
    c.ipv6(None, &IpOptions::new()).unwrap();
    assert_eq!(rec.last().path, "/v1/ipv6");
}

#[test]
fn timezone() {
    let rec = Recorder::of(replay("timezone"));
    let r = client(&rec)
        .timezone(35.6762, 139.6503, &TimezoneOptions::new())
        .unwrap();
    assert_sent_like("timezone", &rec.last());
    assert_eq!(
        (
            r.timezone.as_deref(),
            r.utc_offset.as_deref(),
            r.name.as_deref(),
            r.abbreviation.as_deref()
        ),
        (
            Some("Asia/Tokyo"),
            Some("+09:00"),
            Some("Japan Standard Time"),
            Some("JST")
        )
    );
}

#[test]
fn timezone_by_zone_id() {
    let rec = Recorder::of(replay("timezone_tz"));
    let r = client(&rec)
        .timezone_with(&TimezoneOptions::new().tz("Europe/Paris"))
        .unwrap();
    assert_sent_like("timezone_tz", &rec.last());
    assert_eq!(r.timezone.as_deref(), Some("Europe/Paris"));
}

#[test]
fn elevation() {
    let rec = Recorder::of(replay("elevation"));
    let r = client(&rec)
        .elevation(27.9881, 86.9250, &ElevationOptions::new())
        .unwrap();
    assert_sent_like("elevation", &rec.last());
    assert_eq!(r.results[0].elevation_m, Some(8627.0));
}

#[test]
fn elevation_answered_flat_still_has_results() {
    let b =
        json(r#"{"status":"ok","lat":27.9881,"lon":86.925,"elevation_m":8627,"resolution_m":30}"#);
    let r = client(&Recorder::of(answer(200, &b, &[])))
        .elevation(27.9881, 86.925, &ElevationOptions::new())
        .unwrap();
    assert_eq!(r.results.len(), 1);
    assert_eq!(r.results[0].elevation_m, Some(8627.0));
}

#[test]
fn postcode() {
    let rec = Recorder::of(replay("postcode"));
    let r = client(&rec)
        .postcode("10001", &PostcodeOptions::new().country("US"))
        .unwrap();
    assert_sent_like("postcode", &rec.last());
    assert_eq!(
        (
            r.results[0].place.as_deref(),
            r.results[0].region_code.as_deref()
        ),
        (Some("New York"), Some("NY"))
    );
}

#[test]
fn forward_batch() {
    let rec = Recorder::of(replay("forward_batch"));
    let r = client(&rec)
        .forward_batch(
            &[
                "Brandenburg Gate, Berlin",
                "Sydney Opera House",
                "Statue of Liberty, New York",
            ],
            &ForwardBatchOptions::new(),
        )
        .unwrap();
    assert_sent_like("forward_batch", &rec.last());
    let got: Vec<String> = r
        .results
        .iter()
        .map(|e| {
            format!(
                "{}: {}",
                e.query.as_deref().unwrap(),
                e.results[0].formatted.as_deref().unwrap()
            )
        })
        .collect();
    assert_eq!(
        got,
        vec![
            "Brandenburg Gate, Berlin: Brandenburg Gate, State of Berlin, Germany",
            "Sydney Opera House: Sydney Opera House, Sydney, New South Wales, Australia",
            "Statue of Liberty, New York: Statue of Liberty, New York, New York, United States",
        ]
    );
}

#[test]
fn forward_batch_reads_a_numeric_postcode_as_text() {
    let r = client(&Recorder::of(replay("forward_batch")))
        .forward_batch(
            &[
                "Brandenburg Gate, Berlin",
                "Sydney Opera House",
                "Statue of Liberty, New York",
            ],
            &ForwardBatchOptions::new(),
        )
        .unwrap();
    let york = &r.results[2].results[3];
    assert_eq!(
        york.raw.get("components").get("postcode").as_i64(),
        Some(68467)
    );
    assert_eq!(
        york.components.as_ref().unwrap().postcode.as_deref(),
        Some("68467")
    );
}

#[test]
fn reverse_batch() {
    let rec = Recorder::of(replay("reverse_batch"));
    let r = client(&rec)
        .reverse_batch(&[EIFFEL, LIBERTY, OPERA], &ReverseOptions::new())
        .unwrap();
    assert_sent_like("reverse_batch", &rec.last());
    let got: Vec<&str> = r
        .results
        .iter()
        .map(|p| p.result.as_ref().unwrap().formatted.as_deref().unwrap())
        .collect();
    assert_eq!(
        got,
        vec![
            "5 Avenue Anatole France, 75007 Paris, France",
            "1 Liberty Island, New York, NY 10004, United States",
            "2B Macquarie Street, Sydney NSW 2000, Australia"
        ]
    );
    assert_eq!(r.results[0].meta.request_id, None);
    let tuples = client(&Recorder::of(replay("reverse_batch"))).reverse_batch(
        &[(48.8584, 2.2945), (40.6892, -74.0445), (-33.8568, 151.2153)],
        &ReverseOptions::new(),
    );
    assert!(tuples.is_ok());
}

#[test]
fn ip_batches() {
    let cases: [(&str, Vec<&str>); 3] = [
        ("ip_batch", vec!["8.8.8.8", "2606:4700:4700::1111"]),
        ("ipv4_batch", vec!["8.8.8.8", "1.1.1.1", "9.9.9.9"]),
        (
            "ipv6_batch",
            vec!["2001:4860:4860::8888", "2606:4700:4700::1111"],
        ),
    ];
    for (name, addresses) in cases.iter() {
        let rec = Recorder::of(replay(name));
        let c = client(&rec);
        let r = match *name {
            "ip_batch" => c.ip_batch(addresses, &IpOptions::new()),
            "ipv4_batch" => c.ipv4_batch(addresses, &IpOptions::new()),
            _ => c.ipv6_batch(addresses, &IpOptions::new()),
        }
        .unwrap();
        assert_sent_like(name, &rec.last());
        let got: Vec<&str> = r.results.iter().map(|i| i.ip.as_deref().unwrap()).collect();
        assert_eq!(&got, addresses, "{}", name);
    }
    let r = client(&Recorder::of(replay("ipv4_batch")))
        .ipv4_batch(&["8.8.8.8", "1.1.1.1", "9.9.9.9"], &IpOptions::new())
        .unwrap();
    let owners: Vec<&str> = r
        .results
        .iter()
        .map(|i| i.as_name.as_deref().unwrap())
        .collect();
    assert_eq!(owners, vec!["Google LLC", "Cloudflare, Inc.", "Quad9"]);
}

#[test]
fn ip_batch_entry_the_api_rejects() {
    let b = json(&format!(
        r#"{{"status":"ok","results":[{},{{"status":"error","ip":"not-an-ip","error":{{"code":"invalid_ip","message":"Not an IP address."}}}}]}}"#,
        body("ipv4").to_json()
    ));
    let r = client(&Recorder::of(answer(200, &b, &[])))
        .ip_batch(&["8.8.8.8", "not-an-ip"], &IpOptions::new())
        .unwrap();
    assert_eq!(r.results[0].status.as_deref(), Some("ok"));
    assert_eq!(r.results[1].status.as_deref(), Some("error"));
    assert_eq!(
        r.results[1].error.as_ref().unwrap().get("code").as_str(),
        Some("invalid_ip")
    );
    assert_eq!(r.results[1].country, None);
}

#[test]
fn timezone_batch() {
    let rec = Recorder::of(replay("timezone_batch"));
    let r = client(&rec)
        .timezone_batch(&[EIFFEL, LIBERTY, OPERA], &TimezoneBatchOptions::new())
        .unwrap();
    assert_sent_like("timezone_batch", &rec.last());
    let got: Vec<&str> = r
        .results
        .iter()
        .map(|z| z.timezone.as_deref().unwrap())
        .collect();
    assert_eq!(
        got,
        vec!["Europe/Paris", "America/New_York", "Australia/Sydney"]
    );
    assert_eq!(
        r.timestamp,
        body("timezone_batch").get("timestamp").as_i64()
    );
}

#[test]
fn elevation_batch() {
    let rec = Recorder::of(replay("elevation_batch"));
    let r = client(&rec)
        .elevation_batch(&[EIFFEL, LIBERTY, OPERA], &ElevationBatchOptions::new())
        .unwrap();
    assert_sent_like("elevation_batch", &rec.last());
    let got: Vec<f64> = r.results.iter().map(|p| p.elevation_m.unwrap()).collect();
    assert_eq!(got, vec![37.0, 0.0, 10.0]);
}

#[test]
fn unknown_fields_do_not_break_reading() {
    let text = body("reverse")
        .to_json()
        .replacen("{", r#"{"brand_new_field":{"x":1},"#, 1)
        .replacen(
            r#""result":{"#,
            r#""result":{"another_new_field":[1,2],"#,
            1,
        );
    let r = client(&Recorder::of(answer(200, &json(&text), &[])))
        .reverse(EIFFEL.lat, EIFFEL.lon, &ReverseOptions::new())
        .unwrap();
    assert_eq!(r.raw.get("brand_new_field").get("x").as_i64(), Some(1));
    let p = r.result.unwrap();
    assert_eq!(
        p.raw.get("another_new_field").as_array().map(|a| a.len()),
        Some(2)
    );
    assert_eq!(
        p.formatted.as_deref(),
        Some("5 Avenue Anatole France, 75007 Paris, France")
    );
}
