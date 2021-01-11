mod wire;
use wire::command;
use wire::response;
mod device;

use std::{env, process};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Not enough arguments. Need at least 1.");
        process::exit(1);
    }

    let mut dev = device::T6615::new(&args[1]).unwrap();

    let stat: response::Status = dev.execute(command::Status).unwrap();
    println!(
        "in error: {}, in warmup: {}",
        stat.is_err(),
        stat.in_warmup()
    );

    let got: response::GasPPM = dev.execute(command::Read(wire::Variable::GasPPM)).unwrap();
    println!("got PPM: {}", got.concentration().ppm());
}
