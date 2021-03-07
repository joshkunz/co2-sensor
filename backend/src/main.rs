use std::env;
use std::net;
use std::process;
use tokio;
use warp;
mod device;
mod server;
mod wire;
use device::Device;
use std::thread;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Must supply serial device address.");
        process::exit(1);
    }
    let mut sensor = device::T6615::new(&args[1]).expect("unable to connect to sensor");

    println!("Waiting for warmup...");
    sensor.wait_warmup(thread::sleep).unwrap();

    println!("Booting...");
    let server = server::Server::with_device(sensor);

    println!("Serving on 0.0.0.0:8000");
    let mut rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(warp::serve(server.routes()).run((net::Ipv4Addr::new(0, 0, 0, 0), 8000)));
}
