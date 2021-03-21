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

fn print_device(d: &mut device::T6615) -> device::Result<()> {
    let serial: wire::response::SerialNumber =
        d.execute(wire::command::Read(wire::Variable::SerialNumber))?;
    let subvol: wire::response::CompileSubvol =
        d.execute(wire::command::Read(wire::Variable::CompileSubvol))?;
    let date: wire::response::CompileDate =
        d.execute(wire::command::Read(wire::Variable::CompileDate))?;
    println!("Device: Telaire T6615");
    println!("  Serial: {}", serial);
    println!("  Software Version: {}.{}", subvol, date);
    return Ok(());
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Must supply <static-dir> <serial-device>");
    }
    let (static_dir, serial_device_path) = (&args[1], &args[2]);
    let mut sensor = device::T6615::new(serial_device_path).expect("unable to connect to sensor");

    print_device(&mut sensor).expect("failed to read device metadata");

    println!("Waiting for warmup...");
    sensor.wait_warmup(thread::sleep).unwrap();

    let status: wire::response::Status = sensor
        .execute(wire::command::Status)
        .expect("failed to read device status");
    if !status.is_normal() {
        eprintln!("Error: Abnormal device status on startup: {}", status);
        process::exit(1);
    }

    println!("Booting server...");
    let mut server_builder = server::Builder::default();
    server_builder.device(sensor);
    server_builder.static_dir(static_dir);
    let server = server_builder.build().expect("failed to build server");

    println!("Serving on 0.0.0.0:80");
    gotham::start((net::Ipv4Addr::new(0, 0, 0, 0), 80), server.routes());
}
