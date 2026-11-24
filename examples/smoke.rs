//! Live smoke test: 7 requests in total. Uses MYGEOCODE_KEY if set, else the free requests.
//! Run: cargo run --example smoke

use mygeocode::blocking::Client;
use mygeocode::*;

fn main() -> Result<()> {
    let c = Client::new();

    let fwd = c.forward("Brandenburg Gate, Berlin", &ForwardOptions::new().limit(1))?;
    let code = fwd
        .results
        .first()
        .and_then(|p| p.components.as_ref())
        .and_then(|x| x.country_code.clone());
    assert_eq!(code.as_deref(), Some("de"), "forward");

    let rev = c.reverse(48.8584, 2.2945, &ReverseOptions::new())?;
    let code = rev
        .result
        .as_ref()
        .and_then(|p| p.components.as_ref())
        .and_then(|x| x.country_code.clone());
    assert_eq!(code.as_deref(), Some("fr"), "reverse");

    assert!(
        !c.autocomplete("1600 Pennsylvania", &AutocompleteOptions::new().limit(3))?
            .suggestions
            .is_empty(),
        "autocomplete"
    );

    let ip = c.ipv4(Some("8.8.8.8"), &IpOptions::new())?;
    assert_eq!(
        (ip.country_code.as_deref(), ip.asn),
        (Some("US"), Some(15169)),
        "ipv4"
    );

    assert_eq!(
        c.timezone(35.6762, 139.6503, &TimezoneOptions::new())?
            .timezone
            .as_deref(),
        Some("Asia/Tokyo")
    );

    let el = c.elevation(27.9881, 86.9250, &ElevationOptions::new())?;
    assert!(
        el.results
            .first()
            .and_then(|p| p.elevation_m)
            .map_or(false, |m| m > 8000.0),
        "elevation"
    );

    let pc = c.postcode("10001", &PostcodeOptions::new().country("US"))?;
    assert!(!pc.results.is_empty(), "postcode");

    println!(
        "ok; request id {:?} free left {:?}",
        pc.meta.request_id, pc.meta.quota_free_remaining
    );
    Ok(())
}
