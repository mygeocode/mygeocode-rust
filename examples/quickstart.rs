//! Keyless example: the network's free requests apply. Run: cargo run --example quickstart

use mygeocode::blocking::Client;
use mygeocode::{ForwardOptions, ReverseOptions};

fn main() -> Result<(), mygeocode::Error> {
    let client = Client::new();

    let found = client.forward("Brandenburg Gate, Berlin", &ForwardOptions::new().limit(1))?;
    let best = &found.results[0];
    println!(
        "{} {:?} {:?} {}",
        best.formatted.as_deref().unwrap_or("?"),
        best.lat,
        best.lon,
        best.precision.as_deref().unwrap_or("?")
    );

    let back = client.reverse(48.8584, 2.2945, &ReverseOptions::new())?;
    match back.result.and_then(|p| p.formatted) {
        Some(address) => println!("{}", address),
        None => println!("nothing nearby"),
    }

    println!(
        "free requests left today: {:?}",
        found.meta.quota_free_remaining
    );
    Ok(())
}
