//! The API may one day answer a field with null instead of dropping it (fields this crate reads are never removed;
//! they become null). Every method must keep working: a null field reads as `None` (never a made-up 0 or ""), a null
//! list as an empty list and a null object as `None`, with no error.
//!
//! Each recorded answer is replayed twice: once with every scalar set to null ("leaves"), once with every value set to
//! null, lists and objects included ("all"). Only the top-level "status" stays "ok".

mod common;

use common::*;
use mygeocode::blocking::Client;
use mygeocode::*;

fn nullify(v: &Value, everything: bool, top: bool) -> Value {
    match v {
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(k, item)| {
                    let n = if top && k == "status" {
                        Value::String("ok".to_string())
                    } else if everything {
                        Value::Null
                    } else {
                        match item {
                            Value::Object(_) => nullify(item, false, false),
                            Value::Array(items) => Value::Array(
                                items
                                    .iter()
                                    .map(|i| {
                                        if i.as_object().is_some() {
                                            nullify(i, false, false)
                                        } else {
                                            Value::Null
                                        }
                                    })
                                    .collect(),
                            ),
                            _ => Value::Null,
                        }
                    };
                    (k.clone(), n)
                })
                .collect(),
        ),
        _ => Value::Null,
    }
}

/// The debug text of a result, with the top-level "ok" taken out, must show no text, number or boolean anywhere.
fn check(name: &str, debug: String) {
    let cleaned = debug
        .replacen("status: Some(\"ok\")", "status: None", 1)
        .replacen("(\"status\", String(\"ok\"))", "(\"status\", Null)", 1);
    for bad in [
        "Some(\"",
        "Some(true",
        "Some(false",
        "String(",
        "Number(",
        "Bool(",
    ] {
        assert!(
            !cleaned.contains(bad),
            "{}: found {} in {}",
            name,
            bad,
            cleaned
        );
    }
    let digits = cleaned
        .match_indices("Some(")
        .any(|(i, _)| cleaned[i + 5..].starts_with(|c: char| c.is_ascii_digit() || c == '-'));
    assert!(!digits, "{}: a number survived: {}", name, cleaned);
}

type Call = Box<dyn Fn(&Client) -> String>;

fn run_all(everything: bool) {
    let calls: Vec<(&str, Call)> = vec![
        (
            "forward",
            Box::new(|c| {
                format!(
                    "{:?}",
                    c.forward("Brandenburg Gate, Berlin", &ForwardOptions::new())
                        .unwrap()
                )
            }),
        ),
        (
            "reverse",
            Box::new(|c| {
                format!(
                    "{:?}",
                    c.reverse(EIFFEL.lat, EIFFEL.lon, &ReverseOptions::new())
                        .unwrap()
                )
            }),
        ),
        (
            "autocomplete",
            Box::new(|c| {
                format!(
                    "{:?}",
                    c.autocomplete("1600 Pennsylvania", &AutocompleteOptions::new())
                        .unwrap()
                )
            }),
        ),
        (
            "ip",
            Box::new(|c| format!("{:?}", c.ip(Some("8.8.8.8"), &IpOptions::new()).unwrap())),
        ),
        (
            "ipv4",
            Box::new(|c| format!("{:?}", c.ipv4(Some("8.8.8.8"), &IpOptions::new()).unwrap())),
        ),
        (
            "ipv6",
            Box::new(|c| {
                format!(
                    "{:?}",
                    c.ipv6(Some("2001:4860:4860::8888"), &IpOptions::new())
                        .unwrap()
                )
            }),
        ),
        (
            "timezone",
            Box::new(|c| {
                format!(
                    "{:?}",
                    c.timezone(35.6762, 139.6503, &TimezoneOptions::new())
                        .unwrap()
                )
            }),
        ),
        (
            "elevation",
            Box::new(|c| {
                format!(
                    "{:?}",
                    c.elevation(27.9881, 86.925, &ElevationOptions::new())
                        .unwrap()
                )
            }),
        ),
        (
            "postcode",
            Box::new(|c| {
                format!(
                    "{:?}",
                    c.postcode("10001", &PostcodeOptions::new().country("US"))
                        .unwrap()
                )
            }),
        ),
        (
            "forward_batch",
            Box::new(|c| {
                format!(
                    "{:?}",
                    c.forward_batch(
                        &["Brandenburg Gate, Berlin", "Sydney Opera House"],
                        &ForwardBatchOptions::new()
                    )
                    .unwrap()
                )
            }),
        ),
        (
            "reverse_batch",
            Box::new(|c| {
                format!(
                    "{:?}",
                    c.reverse_batch(&[EIFFEL, LIBERTY], &ReverseOptions::new())
                        .unwrap()
                )
            }),
        ),
        (
            "ip_batch",
            Box::new(|c| {
                format!(
                    "{:?}",
                    c.ip_batch(&["8.8.8.8", "2606:4700:4700::1111"], &IpOptions::new())
                        .unwrap()
                )
            }),
        ),
        (
            "ipv4_batch",
            Box::new(|c| {
                format!(
                    "{:?}",
                    c.ipv4_batch(&["8.8.8.8", "1.1.1.1", "9.9.9.9"], &IpOptions::new())
                        .unwrap()
                )
            }),
        ),
        (
            "ipv6_batch",
            Box::new(|c| {
                format!(
                    "{:?}",
                    c.ipv6_batch(&["2001:4860:4860::8888"], &IpOptions::new())
                        .unwrap()
                )
            }),
        ),
        (
            "timezone_batch",
            Box::new(|c| {
                format!(
                    "{:?}",
                    c.timezone_batch(&[EIFFEL, LIBERTY], &TimezoneBatchOptions::new())
                        .unwrap()
                )
            }),
        ),
        (
            "elevation_batch",
            Box::new(|c| {
                format!(
                    "{:?}",
                    c.elevation_batch(&[EIFFEL, LIBERTY], &ElevationBatchOptions::new())
                        .unwrap()
                )
            }),
        ),
    ];
    for (name, call) in calls {
        let b = nullify(&body(name), everything, true);
        let client = Setup::new().client(&Recorder::of(answer(200, &b, &[])));
        check(name, call(&client));
    }
}

#[test]
fn every_scalar_null_stays_none() {
    run_all(false);
}

#[test]
fn every_value_null_including_lists_and_objects() {
    run_all(true);
}

#[test]
fn null_lists_read_as_empty_lists() {
    let c = Setup::new().client(&Recorder::of(answer(
        200,
        &json(r#"{"status":"ok","query":null,"results":null}"#),
        &[],
    )));
    assert!(c
        .forward("x", &ForwardOptions::new())
        .unwrap()
        .results
        .is_empty());
    let c = Setup::new().client(&Recorder::of(answer(
        200,
        &json(r#"{"status":"ok","lat":null,"lon":null,"result":null}"#),
        &[],
    )));
    let r = c.reverse(1.0, 2.0, &ReverseOptions::new()).unwrap();
    assert!(r.result.is_none() && r.lat.is_none());
    let c = Setup::new().client(&Recorder::of(answer(
        200,
        &json(r#"{"status":"ok","ip":"8.8.8.8","tags":null}"#),
        &[],
    )));
    let ip = c.ip(Some("8.8.8.8"), &IpOptions::new()).unwrap();
    assert!(ip.tags.is_empty() && ip.country.is_none());
}

#[test]
fn wrong_types_read_as_none() {
    let b = json(
        r#"{"status":"ok","lat":"not a number","result":{"formatted":[1],"components":"x","distance_m":"far"}}"#,
    );
    let r = Setup::new()
        .client(&Recorder::of(answer(200, &b, &[])))
        .reverse(1.0, 2.0, &ReverseOptions::new())
        .unwrap();
    assert_eq!(r.lat, None);
    let p = r.result.unwrap();
    assert_eq!(
        (p.formatted, p.components, p.distance_m),
        (None, None, None)
    );
}

#[test]
#[should_panic(expected = "found")]
fn the_check_catches_a_real_value() {
    let c = Setup::new().client(&Recorder::of(replay("reverse")));
    check(
        "reverse",
        format!(
            "{:?}",
            c.reverse(EIFFEL.lat, EIFFEL.lon, &ReverseOptions::new())
                .unwrap()
        ),
    );
}
