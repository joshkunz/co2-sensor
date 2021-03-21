use std::env;
use std::net;
use std::process;
mod device;
mod server;
mod wire;
use device::Device;
use gotham;
use std::default::Default;
use std::thread;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Must supply <static-dir> <serial-device>");
        process::exit(1);
    }
    let (static_dir, serial_device_path) = (&args[1], &args[2]);
    let mut sensor = device::T6615::new(serial_device_path).expect("unable to connect to sensor");

    println!("Waiting for warmup...");
    sensor.wait_warmup(thread::sleep).unwrap();

    println!("Booting...");
    let mut server_builder = server::Builder::default();
    server_builder.device(sensor);
    server_builder.static_dir(static_dir);
    let server = server_builder.build().expect("failed to build server");

    println!("Serving on 0.0.0.0:80");
    gotham::start((net::Ipv4Addr::new(0, 0, 0, 0), 80), server.routes());
}
