use warp;
use warp::{Filter};
use tokio;
use std::net;
use prometheus;
use prometheus::{Encoder};

async fn serve_metrics() {
    let reg = prometheus::default_registry();

    let routes = warp::path!("metrics").map(move || {
        let enc = prometheus::TextEncoder::new();
        let mut out: Vec<u8> = Vec::new();

        if let Err(e) = enc.encode(&reg.gather(), &mut out) {
            return e.to_string();
        }
        return String::from_utf8(out).unwrap();
    });

    let addr: net::IpAddr = "0.0.0.0".parse().unwrap();
    warp::serve(routes).run((addr, 8000)).await;
}

async fn tick_counter() {
    let ctr = prometheus::register_int_counter!(
        "ticks", "Ticks up consistently").unwrap();
    let mut every = tokio::time::interval(
        tokio::time::Duration::from_millis(500));
    loop {
        every.tick().await;
        ctr.inc();
    }
}

fn main() {
    let mut rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        tokio::join!(tick_counter(), serve_metrics());
    });
}
