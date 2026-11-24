//! The async client with no runtime at all: a ten-line `block_on` is enough, because each call runs on its own
//! thread. In a tokio, async-std or smol program, just `.await` the calls. Run: cargo run --example async_example

use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};

use mygeocode::{Client, ElevationOptions, TimezoneOptions};

struct Unpark(Thread);

impl Wake for Unpark {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let waker = Waker::from(Arc::new(Unpark(thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
            return output;
        }
        thread::park();
    }
}

fn main() -> Result<(), mygeocode::Error> {
    block_on(async {
        let client = Client::new();
        let tokyo = client
            .timezone(35.6762, 139.6503, &TimezoneOptions::new())
            .await?;
        let everest = client
            .elevation(27.9881, 86.9250, &ElevationOptions::new())
            .await?;
        println!(
            "{} {:?}",
            tokyo.timezone.unwrap_or_default(),
            everest.results[0].elevation_m
        );
        Ok(())
    })
}
