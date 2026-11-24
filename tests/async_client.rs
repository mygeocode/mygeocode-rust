//! The async client: the same methods as the blocking one, the same requests and retries, under any executor.

mod common;

use common::*;
use mygeocode::*;

#[test]
fn every_method_against_its_recording() {
    let setup = Setup::new();
    macro_rules! check {
        ($name:expr, $call:expr) => {{
            let rec = Recorder::of(replay($name));
            let c = setup.builder(&rec).build();
            let r = block_on($call(c)).unwrap();
            assert_sent_like($name, &rec.last());
            assert_eq!(r.raw, body($name), "{}", $name);
        }};
    }
    check!("forward", |c: Client| async move {
        c.forward("Brandenburg Gate, Berlin", &ForwardOptions::new())
            .await
    });
    check!("forward_structured", |c: Client| async move {
        c.forward(
            "",
            &ForwardOptions::new()
                .street("10 Downing Street")
                .city("London")
                .country("GB"),
        )
        .await
    });
    check!("reverse", |c: Client| async move {
        c.reverse(48.8584, 2.2945, &ReverseOptions::new()).await
    });
    check!("autocomplete", |c: Client| async move {
        c.autocomplete("1600 Pennsylvania", &AutocompleteOptions::new())
            .await
    });
    check!("ip", |c: Client| async move {
        c.ip(Some("8.8.8.8"), &IpOptions::new()).await
    });
    check!("ipv4", |c: Client| async move {
        c.ipv4(Some("8.8.8.8"), &IpOptions::new()).await
    });
    check!("ipv6", |c: Client| async move {
        c.ipv6(Some("2001:4860:4860::8888"), &IpOptions::new())
            .await
    });
    check!("timezone", |c: Client| async move {
        c.timezone(35.6762, 139.6503, &TimezoneOptions::new()).await
    });
    check!("timezone_tz", |c: Client| async move {
        c.timezone_with(&TimezoneOptions::new().tz("Europe/Paris"))
            .await
    });
    check!("elevation", |c: Client| async move {
        c.elevation(27.9881, 86.925, &ElevationOptions::new()).await
    });
    check!("postcode", |c: Client| async move {
        c.postcode("10001", &PostcodeOptions::new().country("US"))
            .await
    });
    check!("forward_batch", |c: Client| async move {
        c.forward_batch(
            &[
                "Brandenburg Gate, Berlin",
                "Sydney Opera House",
                "Statue of Liberty, New York",
            ],
            &ForwardBatchOptions::new(),
        )
        .await
    });
    check!("reverse_batch", |c: Client| async move {
        c.reverse_batch(&[EIFFEL, LIBERTY, OPERA], &ReverseOptions::new())
            .await
    });
    check!("ip_batch", |c: Client| async move {
        c.ip_batch(&["8.8.8.8", "2606:4700:4700::1111"], &IpOptions::new())
            .await
    });
    check!("ipv4_batch", |c: Client| async move {
        c.ipv4_batch(&["8.8.8.8", "1.1.1.1", "9.9.9.9"], &IpOptions::new())
            .await
    });
    check!("ipv6_batch", |c: Client| async move {
        c.ipv6_batch(
            &["2001:4860:4860::8888", "2606:4700:4700::1111"],
            &IpOptions::new(),
        )
        .await
    });
    check!("timezone_batch", |c: Client| async move {
        c.timezone_batch(&[EIFFEL, LIBERTY, OPERA], &TimezoneBatchOptions::new())
            .await
    });
    check!("elevation_batch", |c: Client| async move {
        c.elevation_batch(&[EIFFEL, LIBERTY, OPERA], &ElevationBatchOptions::new())
            .await
    });
}

#[test]
fn retries_and_errors_come_through_the_future() {
    let setup = Setup::new();
    let rec = Recorder::new(vec![
        Item::Answer(answer(
            429,
            &error_body("rate_limited"),
            &[("Retry-After", "3")],
        )),
        Item::Answer(replay("forward")),
    ]);
    block_on(
        setup
            .builder(&rec)
            .build()
            .forward("Brandenburg Gate, Berlin", &ForwardOptions::new()),
    )
    .unwrap();
    assert_eq!(setup.sleeps(), vec![3.0]);
    let rec = Recorder::of(replay("error_invalid_request"));
    let e = block_on(
        Setup::new()
            .builder(&rec)
            .build()
            .forward("", &ForwardOptions::new()),
    )
    .unwrap_err();
    assert_eq!(e.code(), Some("invalid_request"));
}

#[test]
fn many_calls_at_once() {
    let rec = Recorder::of(replay("ipv4"));
    let c = Setup::new().builder(&rec).build();
    let futures: Vec<_> = (0..20)
        .map(|_| c.ipv4(Some("8.8.8.8"), &IpOptions::new()))
        .collect();
    for f in futures {
        assert_eq!(block_on(f).unwrap().as_name.as_deref(), Some("Google LLC"));
    }
    assert_eq!(rec.count(), 20);
}

#[test]
fn the_futures_are_send() {
    fn is_send<T: Send>(_: &T) {}
    let c = Client::new();
    let f = c.reverse(48.8584, 2.2945, &ReverseOptions::new());
    is_send(&f);
    let _ = (std::mem::size_of::<blocking::Client>(), &c);
    drop(f);
}
