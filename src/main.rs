use warp;
use warp::{Filter};
use tokio;
use std::net;
use prometheus;
use prometheus::{Encoder};
use std::env;
use std::process;
mod device;
mod wire;
use device::Device;
use std::thread;
use std::time;

async fn serve_metrics(addr: &str) {
    let reg = prometheus::default_registry();

    let routes = warp::path!("metrics").map(move || {
        let enc = prometheus::TextEncoder::new();
        let mut out: Vec<u8> = Vec::new();

        if let Err(e) = enc.encode(&reg.gather(), &mut out) {
            return e.to_string();
        }
        return String::from_utf8(out).unwrap();
    });

    println!("serving metrics on {}:8000", addr);
    let addr: net::IpAddr = addr.parse().unwrap();
    warp::serve(routes).run((addr, 8000)).await;
}

async fn measure(mut sensor: device::T6615) {
    let gague = prometheus::register_gauge!(
        "co2_ppm", "The current concentration of CO2 in parts per million.")
        .expect("unable to setup CO2 gauge");

    // Update every interval.
    let mut every = tokio::time::interval(
        tokio::time::Duration::from_secs(25));
    loop {
        every.tick().await;

        println!("Measuring...");
        match sensor.read_co2() {
            Ok(c) => gague.set(c.ppm() as f64),
            Err(e) => eprintln!("Error reading value: {}", e.to_string()),
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Must supply serial device address.");
        process::exit(1);
    }
    let mut sensor = device::T6615::new(&args[1])
        .expect("unable to connect to sensor");

    println!("waiting for warmup...");
    loop {
        let status: wire::response::Status = sensor.execute(
            wire::command::Status).unwrap();
        if !status.in_warmup() {
            break;
        }
        thread::sleep(time::Duration::from_millis(500));
    }

    println!("Booting...");
    let mut rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async move {
        tokio::join!(measure(sensor), serve_metrics("0.0.0.0"));
    });
}
