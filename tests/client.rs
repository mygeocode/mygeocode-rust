//! Client behaviour: key handling, defaults, errors, retries, timeouts and the language rules.

mod common;

use std::time::Duration;

use common::*;
use mygeocode::*;

const Q: &str = "Brandenburg Gate, Berlin";

fn fwd() -> ForwardOptions {
    ForwardOptions::new()
}

#[test]
fn keyless_sends_no_key_and_our_user_agent() {
    let rec = Recorder::of(replay("forward"));
    Setup::new().client(&rec).forward(Q, &fwd()).unwrap();
    let s = rec.last();
    assert_eq!(s.header("x-api-key"), None);
    assert_eq!(
        s.header("user-agent"),
        Some(
            format!(
                "mygeocode-rust/{} (+https://www.mygeocode.com/sdks/)",
                VERSION
            )
            .as_str()
        )
    );
    assert_eq!(
        s.header("user-agent").map(|s| s.to_string()),
        Some(user_agent())
    );
    assert_eq!(s.header("accept"), Some("application/json"));
    assert!(
        s.url.starts_with("https://api.mygeocode.com/v1/forward?"),
        "{}",
        s.url
    );
}

#[test]
fn key_goes_in_the_header_never_the_url() {
    let rec = Recorder::of(replay("forward"));
    Setup::new()
        .builder(&rec)
        .key("mg_test_key")
        .build_blocking()
        .forward(Q, &fwd())
        .unwrap();
    let s = rec.last();
    assert_eq!(s.header("x-api-key"), Some("mg_test_key"));
    assert!(!s.url.contains("mg_test_key"));
    assert!(!s.query.contains_key("key"));
}

#[test]
fn key_from_the_environment_and_empty_key_means_none() {
    let rec = Recorder::of(replay("forward"));
    let setup = Setup::new();
    setup
        .builder(&rec)
        .environment(|name| {
            if name == "MYGEOCODE_KEY" {
                Some("mg_env_key".to_string())
            } else {
                None
            }
        })
        .build_blocking()
        .forward(Q, &fwd())
        .unwrap();
    assert_eq!(rec.last().header("x-api-key"), Some("mg_env_key"));
    setup
        .builder(&rec)
        .key("")
        .build_blocking()
        .forward(Q, &fwd())
        .unwrap();
    assert_eq!(rec.last().header("x-api-key"), None);
    setup
        .builder(&rec)
        .key("  mg_padded  ")
        .build_blocking()
        .forward(Q, &fwd())
        .unwrap();
    assert_eq!(rec.last().header("x-api-key"), Some("mg_padded"));
}

#[test]
fn defaults_are_ten_seconds_and_english() {
    let rec = Recorder::of(replay("forward"));
    Setup::new().client(&rec).forward(Q, &fwd()).unwrap();
    assert_eq!(rec.last().timeout, Duration::from_secs(10));
    assert_eq!(rec.last().query["lang"], "en");
    assert_eq!(DEFAULT_BASE_URL, "https://api.mygeocode.com/v1");
}

#[test]
fn base_url_and_timeout_reach_the_transport() {
    let rec = Recorder::of(replay("forward"));
    Setup::new()
        .builder(&rec)
        .base_url("https://example.test/v1/")
        .timeout(Duration::from_secs(3))
        .build_blocking()
        .forward(Q, &fwd())
        .unwrap();
    assert!(
        rec.last()
            .url
            .starts_with("https://example.test/v1/forward?"),
        "{}",
        rec.last().url
    );
    assert_eq!(rec.last().timeout, Duration::from_secs(3));
}

#[test]
fn lang_on_the_client_and_per_call() {
    let rec = Recorder::of(replay("forward"));
    let c = Setup::new().builder(&rec).lang("de").build_blocking();
    c.forward(Q, &fwd()).unwrap();
    assert_eq!(rec.last().query["lang"], "de");
    c.forward(Q, &fwd().lang("fr")).unwrap();
    assert_eq!(rec.last().query["lang"], "fr");
}

#[test]
fn timezone_language_only_when_chosen() {
    let rec = Recorder::of(replay("timezone"));
    let setup = Setup::new();
    setup
        .client(&rec)
        .timezone(35.6762, 139.6503, &TimezoneOptions::new())
        .unwrap();
    assert!(!rec.last().query.contains_key("language"));
    setup
        .client(&rec)
        .timezone(35.6762, 139.6503, &TimezoneOptions::new().lang("ja"))
        .unwrap();
    assert_eq!(rec.last().query["language"], "ja");
    setup
        .client(&rec)
        .timezone(35.6762, 139.6503, &TimezoneOptions::new().language("de"))
        .unwrap();
    assert_eq!(rec.last().query["language"], "de");
    setup
        .builder(&rec)
        .lang("en")
        .build_blocking()
        .timezone(35.6762, 139.6503, &TimezoneOptions::new())
        .unwrap();
    assert_eq!(rec.last().query["language"], "en");
    assert!(!rec.last().query.contains_key("lang"));
}

#[test]
fn error_body_becomes_an_api_error() {
    let rec = Recorder::of(replay("error_invalid_request"));
    let e = Setup::new().client(&rec).forward("", &fwd()).unwrap_err();
    let r = recording("error_invalid_request");
    let api = e.api().expect("an API error");
    assert_eq!(api.status as i64, r.get("status").as_i64().unwrap());
    assert_eq!(
        Some(api.code.as_str()),
        r.get("body").get("error").get("code").as_str()
    );
    assert_eq!(
        Some(api.message.as_str()),
        r.get("body").get("error").get("message").as_str()
    );
    assert_eq!(
        api.param.as_deref(),
        r.get("body").get("error").get("param").as_str()
    );
    assert_eq!(
        api.request_id.as_deref(),
        r.get("headers").get("x-request-id").as_str()
    );
    assert_eq!(api.body, r.get("body").clone());
    assert_eq!(api.retry_after, None);
    assert_eq!(e.code(), Some("invalid_request"));
    assert!(std::error::Error::source(&e).is_some());
}

#[test]
fn error_without_a_json_body() {
    let rec = Recorder::of(text(502, "<html>Bad gateway</html>"));
    let e = Setup::new().client(&rec).forward(Q, &fwd()).unwrap_err();
    let api = e.api().unwrap();
    assert_eq!(
        (api.status, api.code.as_str(), api.message.as_str()),
        (502, "http_error", "HTTP 502")
    );
    assert_eq!(api.body, Value::Null);
}

#[test]
fn success_that_is_not_json_is_a_decode_error() {
    for body in ["not json", "[1,2]"] {
        let rec = Recorder::of(text(200, body));
        assert!(
            matches!(
                Setup::new().client(&rec).forward(Q, &fwd()),
                Err(Error::Decode(_))
            ),
            "{}",
            body
        );
    }
}

#[test]
fn status_error_in_a_200_is_an_api_error() {
    let rec = Recorder::of(answer(200, &error_body("invalid_request"), &[]));
    let e = Setup::new().client(&rec).forward(Q, &fwd()).unwrap_err();
    assert_eq!(
        (e.api().unwrap().status, e.code()),
        (200, Some("invalid_request"))
    );
}

#[test]
fn e429_waits_for_retry_after() {
    let setup = Setup::new();
    let rec = Recorder::new(vec![
        Item::Answer(answer(
            429,
            &error_body("rate_limited"),
            &[("Retry-After", "7")],
        )),
        Item::Answer(replay("forward")),
    ]);
    let r = setup.client(&rec).forward(Q, &fwd()).unwrap();
    assert_eq!(setup.sleeps(), vec![7.0]);
    assert_eq!(r.status.as_deref(), Some("ok"));
    assert_eq!(rec.count(), 2);
}

#[test]
fn e429_waits_until_the_quota_reset() {
    let setup = Setup::new();
    let rec = Recorder::new(vec![
        Item::Answer(answer(
            429,
            &error_body("quota_exceeded"),
            &[("X-Quota-Reset", "1030")],
        )),
        Item::Answer(replay("forward")),
    ]);
    setup.client(&rec).forward(Q, &fwd()).unwrap();
    assert_eq!(setup.sleeps(), vec![30.0]);
}

#[test]
fn e429_without_a_wait_returns_at_once() {
    let setup = Setup::new();
    let rec = Recorder::of(answer(429, &error_body("rate_limited"), &[]));
    assert!(setup.client(&rec).forward(Q, &fwd()).is_err());
    assert!(setup.sleeps().is_empty());
    assert_eq!(rec.count(), 1);
}

#[test]
fn quota_gone_for_the_day_returns_at_once() {
    let setup = Setup::new();
    let rec = Recorder::of(answer(
        429,
        &error_body("quota_exceeded"),
        &[("X-Quota-Reset", "90000")],
    ));
    let e = setup.client(&rec).forward(Q, &fwd()).unwrap_err();
    assert_eq!(e.code(), Some("quota_exceeded"));
    assert_eq!(
        e.api().unwrap().retry_after,
        Some(Duration::from_secs(89000))
    );
    assert!(setup.sleeps().is_empty());
    assert_eq!(rec.count(), 1);
}

#[test]
fn max_retry_wait_is_configurable() {
    let setup = Setup::new();
    let rec = Recorder::new(vec![
        Item::Answer(answer(
            429,
            &error_body("rate_limited"),
            &[("Retry-After", "90")],
        )),
        Item::Answer(replay("forward")),
    ]);
    setup
        .builder(&rec)
        .max_retry_wait(Duration::from_secs(120))
        .build_blocking()
        .forward(Q, &fwd())
        .unwrap();
    assert_eq!(setup.sleeps(), vec![90.0]);
}

#[test]
fn retries_stop_at_max_retries() {
    let setup = Setup::new();
    let rec = Recorder::of(answer(503, &error_body("unavailable"), &[]));
    assert!(setup.client(&rec).forward(Q, &fwd()).is_err());
    assert_eq!(setup.sleeps(), vec![1.0, 2.0]);
    assert_eq!(rec.count(), 3);
}

#[test]
fn max_retries_zero_never_retries() {
    let rec = Recorder::of(answer(503, &error_body("unavailable"), &[]));
    assert!(Setup::new()
        .builder(&rec)
        .max_retries(0)
        .build_blocking()
        .forward(Q, &fwd())
        .is_err());
    assert_eq!(rec.count(), 1);
}

#[test]
fn e503_uses_retry_after_and_a_long_one_returns() {
    let setup = Setup::new();
    let rec = Recorder::new(vec![
        Item::Answer(answer(
            503,
            &error_body("maintenance"),
            &[("Retry-After", "5")],
        )),
        Item::Answer(replay("forward")),
    ]);
    setup.client(&rec).forward(Q, &fwd()).unwrap();
    assert_eq!(setup.sleeps(), vec![5.0]);
    let rec = Recorder::of(answer(
        503,
        &error_body("maintenance"),
        &[("Retry-After", "900")],
    ));
    let e = Setup::new().client(&rec).forward(Q, &fwd()).unwrap_err();
    assert_eq!(e.api().unwrap().retry_after, Some(Duration::from_secs(900)));
}

#[test]
fn e500_is_retried_once_after_one_second() {
    let broken = Item::Answer(answer(500, &error_body("server_error"), &[]));
    let setup = Setup::new();
    setup
        .client(&Recorder::new(vec![
            broken.clone(),
            Item::Answer(replay("forward")),
        ]))
        .forward(Q, &fwd())
        .unwrap();
    assert_eq!(setup.sleeps(), vec![1.0]);
    let setup = Setup::new();
    let rec = Recorder::new(vec![broken]);
    assert!(setup
        .builder(&rec)
        .max_retries(5)
        .build_blocking()
        .forward(Q, &fwd())
        .is_err());
    assert_eq!(rec.count(), 2);
    assert_eq!(setup.sleeps(), vec![1.0]);
}

#[test]
fn e500_on_post_and_max_retries() {
    let broken = Item::Answer(answer(500, &error_body("server_error"), &[]));
    let rec = Recorder::new(vec![broken.clone(), Item::Answer(replay("forward_batch"))]);
    Setup::new()
        .client(&rec)
        .forward_batch(&[Q], &ForwardBatchOptions::new())
        .unwrap();
    assert_eq!(rec.count(), 2);
    let rec = Recorder::new(vec![broken]);
    assert!(Setup::new()
        .builder(&rec)
        .max_retries(0)
        .build_blocking()
        .forward_batch(&[Q], &ForwardBatchOptions::new())
        .is_err());
    assert_eq!(rec.count(), 1);
}

#[test]
fn connection_errors_are_retried_on_get() {
    let setup = Setup::new();
    let rec = Recorder::new(vec![
        Item::Refused,
        Item::Refused,
        Item::Answer(replay("forward")),
    ]);
    setup.client(&rec).forward(Q, &fwd()).unwrap();
    assert_eq!(setup.sleeps(), vec![1.0, 2.0]);
}

#[test]
fn network_error_after_the_retries() {
    let rec = Recorder::new(vec![Item::Refused]);
    assert!(matches!(
        Setup::new().client(&rec).forward(Q, &fwd()),
        Err(Error::Network(..))
    ));
    assert_eq!(rec.count(), 3);
}

#[test]
fn timeouts_are_retried_then_returned() {
    let rec = Recorder::new(vec![Item::Timeout]);
    let e = Setup::new().client(&rec).forward(Q, &fwd()).unwrap_err();
    assert!(e.is_timeout());
    assert_eq!(rec.count(), 3);
}

#[test]
fn post_is_not_retried_after_a_connection_error() {
    let rec = Recorder::new(vec![Item::Refused, Item::Answer(replay("forward_batch"))]);
    assert!(matches!(
        Setup::new()
            .client(&rec)
            .forward_batch(&[Q], &ForwardBatchOptions::new()),
        Err(Error::Network(..))
    ));
    assert_eq!(rec.count(), 1);
}

#[test]
fn post_is_retried_on_429() {
    let setup = Setup::new();
    let rec = Recorder::new(vec![
        Item::Answer(answer(
            429,
            &error_body("rate_limited"),
            &[("Retry-After", "2")],
        )),
        Item::Answer(replay("forward_batch")),
    ]);
    setup
        .client(&rec)
        .forward_batch(&[Q], &ForwardBatchOptions::new())
        .unwrap();
    assert_eq!(setup.sleeps(), vec![2.0]);
}

#[test]
fn a_closure_is_a_transport() {
    let client = ClientBuilder::new()
        .transport(
            |_r: &HttpRequest| -> std::result::Result<HttpResponse, TransportError> {
                Ok(replay("reverse"))
            },
        )
        .build_blocking();
    let r = client
        .reverse(48.8584, 2.2945, &ReverseOptions::new())
        .unwrap();
    assert_eq!(
        r.result.unwrap().formatted.as_deref(),
        Some("5 Avenue Anatole France, 75007 Paris, France")
    );
}

#[test]
fn debug_output_hides_the_key() {
    let rec = Recorder::of(replay("forward"));
    let c = Setup::new().builder(&rec).key("mg_secret").build_blocking();
    assert!(!format!("{:?}", c).contains("mg_secret"));
    let c = Setup::new().builder(&rec).key("mg_secret").build();
    assert!(!format!("{:?}", c).contains("mg_secret"));
}
