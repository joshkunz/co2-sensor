use crate::wire;
use serialport;
use std::convert::TryFrom;
use std::io;
use std::io::{Read, Write};
use std::result;
use std::time;

#[derive(Debug, PartialEq)]
pub struct Error(String);

impl ToString for Error {
    fn to_string(&self) -> String {
        let Error(s) = self;
        return s.clone();
    }
}

impl From<String> for Error {
    fn from(s: String) -> Error {
        Error(s)
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Error {
        Error(String::from(s))
    }
}

impl From<serialport::Error> for Error {
    fn from(e: serialport::Error) -> Error {
        Error(e.to_string())
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error(e.to_string())
    }
}

impl From<wire::ParseError> for Error {
    fn from(e: wire::ParseError) -> Error {
        Error(e.to_string())
    }
}

/// Result is the common result type used in this module.
pub type Result<T> = result::Result<T, Error>;

fn round(v: u16, nearest: u16) -> u16 {
    let mid = nearest / 2;
    let diff = v % nearest;
    let lower = v - diff;
    if diff < mid {
        return lower;
    }
    return lower + nearest;
}

/// Device represents a device that can execute commands. This is useful
/// for testing purposes.
pub trait Device {
    fn execute<S, T, E>(&mut self, s: S) -> Result<T>
    where
        S: Into<wire::Payload>,
        E: ToString,
        T: TryFrom<wire::Payload, Error = E>;

    /// A special case of `execute`. Assumes that the given command receives
    /// an ACK reply. Since ACK's don't contain any interesting information,
    /// no result is returned.
    fn execute_ack<S: Into<wire::Payload>>(&mut self, s: S) -> Result<()> {
        let _ack: wire::response::Ack = self.execute(s)?;
        return Ok(());
    }

    /// Read a co2 measurement from the sensor.
    fn read_co2(&mut self) -> Result<wire::Concentration> {
        let r: wire::response::GasPPM =
            self.execute(wire::command::Read(wire::Variable::GasPPM))?;
        return Ok(r.concentration());
    }

    /// Read the configured elevation from the sensor.
    fn read_elevation(&mut self) -> Result<wire::Distance> {
        let wire::response::Elevation(d) =
            self.execute(wire::command::Read(wire::Variable::Elevation))?;
        return Ok(d);
    }

    /// Configure the device to operate at elevation `d`. May be rounded to
    /// nearest 500 feet.
    fn set_elevation(&mut self, d: wire::Distance) -> Result<()> {
        let e = wire::Distance::Feet(round(d.feet(), 500));
        let wire::response::Ack = self.execute(wire::command::UpdateElevation(e))?;
        return Ok(());
    }

    /// Wait for the device to enter a particular status. This function will
    /// continuously poll the device until the given predicate function
    /// (which accepts a status) returns true.
    fn wait_status<P, T>(&mut self, pred: P, sleep_fn: T) -> Result<()>
    where
        P: Fn(wire::response::Status) -> bool,
        T: Fn(),
    {
        loop {
            let r: wire::response::Status = self.execute(wire::command::Status)?;
            if pred(r) {
                return Ok(());
            }
            sleep_fn();
        }
    }

    /// Wait for the device to finish warmup. Should be called before
    /// taking co2 measurements. `sleep_fn` is called between polling cycles.
    fn wait_warmup<T: Fn(time::Duration)>(&mut self, sleep_fn: T) -> Result<()> {
        return self.wait_status(
            |s| !s.in_warmup(),
            || sleep_fn(time::Duration::from_secs(5)),
        );
    }

    /// Calibrate the device's co2 readings to a reference concentration.
    /// This function is very heavyweight, it may take a minute or longer.
    fn calibrate_co2<T: Fn(time::Duration)>(
        &mut self,
        reference: wire::Concentration,
        sleep_fn: T,
    ) -> Result<()> {
        self.execute_ack(wire::command::SetSinglePointPPM(reference))?;
        let got: wire::response::GasPPM =
            self.execute(wire::command::VerifySinglePointCalibration)?;
        if reference != got.concentration() {
            return Err(Error::from(format!(
                "failed to verify single point calibration, got {:?} expected {:?}",
                got, reference
            )));
        }
        // Start the actual calibration.
        self.execute_ack(wire::command::StartSinglePointCalibration)?;
        // Wait for the device to enter calibration mode, polling every 5s.
        self.wait_status(
            |s| s.in_calibration(),
            || sleep_fn(time::Duration::from_secs(5)),
        )?;
        // Wait for the device to exit calibration mode, polling every 15s.
        self.wait_status(
            |s| !s.in_calibration(),
            || sleep_fn(time::Duration::from_secs(15)),
        )?;

        let status: wire::response::Status = self.execute(wire::command::Status)?;
        if !status.is_normal() {
            return Err(Error::from(format!("Unexpected status: {}", status)));
        }
        return Ok(());
    }

    fn disable_abc(&mut self) -> Result<()> {
        let r: wire::response::ABCState =
            self.execute(wire::command::SetABCLogic(wire::Toggle::Off))?;
        if r != wire::response::ABCState::Off {
            return Err(Error::from("ABC state failed toggle off."));
        }
        return Ok(());
    }
}

/// T6615 implements the `Device` trait for the Telaire T6615 CO2 module.
pub struct T6615 {
    port: serialport::TTYPort,
}

impl T6615 {
    /// Construct a new T6615 instance from a TTY path.
    pub fn new(path: &str) -> Result<T6615> {
        let port = serialport::TTYPort::open(
            &serialport::new(path, 19200)
                .parity(serialport::Parity::None)
                .data_bits(serialport::DataBits::Eight)
                .stop_bits(serialport::StopBits::One)
                .timeout(time::Duration::from_secs(1)),
        )?;

        return Ok(T6615 { port: port });
    }
}

impl Device for T6615 {
    fn execute<S, T, E>(&mut self, s: S) -> Result<T>
    where
        S: Into<wire::Payload>,
        E: ToString,
        T: TryFrom<wire::Payload, Error = E>,
    {
        let msg = wire::Message::from(s.into());
        self.port.write_all(&msg)?;

        // Read out the reply header.
        let mut hdr: [u8; 3] = Default::default();
        self.port.read_exact(&mut hdr)?;
        if hdr[0] != 0xFF {
            return Err(Error::from(format!(
                "incorrect Tsunami flag: {:#X}",
                hdr[0]
            )));
        }
        if hdr[1] != 0xFA {
            return Err(Error::from(format!(
                "incorrect Tsunami address: {:#X}",
                hdr[1]
            )));
        }
        let length: usize = hdr[2] as usize;

        // Read out the body.
        let mut body: Vec<u8> = Vec::with_capacity(length);
        // Though body has 'length' capacity, it is still "empty", so it
        // is coereced to an empty slice. Here we reserve 'length'
        // bytes so it will have non-zero size.
        body.resize(length, 0);
        self.port.read_exact(&mut body)?;

        // And unmarshal the reply body into a reply type.
        return Ok(T::try_from(wire::Payload(body)).map_err(|e| e.to_string())?);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync;
    use std::sync::atomic;
    use std::sync::mpsc;
    use std::thread;

    /// Fake implements the `Device` trait, but is not backed by a physical
    /// device. It can be used for testing.
    struct Fake {
        gas: wire::Concentration,
        elevation: wire::Distance,
        in_warmup: sync::Arc<atomic::AtomicBool>,
        status_notify: Option<mpsc::Sender<()>>,
        reference: wire::Concentration,
        in_calibration: sync::Arc<atomic::AtomicBool>,
    }

    impl Default for Fake {
        fn default() -> Self {
            return Fake {
                gas: wire::Concentration::PPM(0),
                elevation: wire::Distance::Feet(0),
                in_warmup: sync::Arc::new(atomic::AtomicBool::new(false)),
                status_notify: None,
                reference: wire::Concentration::PPM(0),
                in_calibration: sync::Arc::new(atomic::AtomicBool::new(false)),
            };
        }
    }

    impl Fake {
        fn with_gas(ppm: u16) -> Fake {
            let mut f: Fake = Default::default();
            f.gas = wire::Concentration::PPM(ppm);
            return f;
        }

        fn with_elevation(feet: u16) -> Fake {
            let mut f: Fake = Default::default();
            f.elevation = wire::Distance::Feet(feet);
            return f;
        }

        fn with_status_notify(s: mpsc::Sender<()>) -> Fake {
            let mut f: Fake = Default::default();
            f.status_notify = Some(s);
            return f;
        }
    }

    impl Device for Fake {
        fn execute<S, T, E>(&mut self, s: S) -> Result<T>
        where
            S: Into<wire::Payload>,
            E: ToString,
            T: TryFrom<wire::Payload, Error = E>,
        {
            let p: wire::Payload = s.into();
            let r: wire::Payload;
            if p == wire::Payload::from(wire::command::Read(wire::Variable::GasPPM)) {
                r = wire::response::GasPPM::with_ppm(self.gas.ppm()).into();
            } else if p == wire::Payload::from(wire::command::Read(wire::Variable::Elevation)) {
                r = wire::response::Elevation(self.elevation).into();
            } else if p == wire::Payload::from(wire::command::Status) {
                let mut flags = wire::response::StatusFlags::default();
                flags.in_warmup = self.in_warmup.load(atomic::Ordering::SeqCst);
                flags.in_calibration = self.in_calibration.load(atomic::Ordering::SeqCst);
                r = wire::response::Status::from(flags).into();
                if let Some(notify) = &self.status_notify {
                    let _r = notify.send(());
                }
            } else if let Ok(u) = wire::command::UpdateElevation::try_from(p.clone()) {
                let wire::command::UpdateElevation(d) = u;
                self.elevation = d;
                r = wire::Payload::from(wire::response::Ack);
            } else if p == wire::Payload::from(wire::command::StartSinglePointCalibration) {
                self.in_calibration.store(true, atomic::Ordering::SeqCst);
                r = wire::Payload::from(wire::response::Ack);
            } else if let Ok(ssp) = wire::command::SetSinglePointPPM::try_from(p.clone()) {
                let wire::command::SetSinglePointPPM(c) = ssp;
                self.reference = c;
                r = wire::response::Ack.into();
            } else if let Ok(_) = wire::command::VerifySinglePointCalibration::try_from(p.clone()) {
                r = wire::response::GasPPM::with_ppm(self.reference.ppm()).into();
            } else {
                return Err(Error::from(format!("fake not implemented: {:?}", p)));
            }
            return T::try_from(r).map_err(|e| Error::from(e.to_string()));
        }
    }

    #[test]
    fn test_read_co2() {
        assert_eq!(
            Fake::with_gas(1200).read_co2(),
            Ok(wire::Concentration::PPM(1200)),
        );
        assert_eq!(
            Fake::with_gas(0).read_co2(),
            Ok(wire::Concentration::PPM(0)),
        );
    }

    #[test]
    fn test_read_elevation() {
        assert_eq!(
            Fake::with_elevation(500).read_elevation(),
            Ok(wire::Distance::Feet(500)),
        );
        assert_eq!(
            Fake::with_elevation(0).read_elevation(),
            Ok(wire::Distance::Feet(0)),
        );
    }

    #[test]
    fn test_wait_warmup() {
        // in_warm_status is the warmup status of the fake.
        let in_warm_status: sync::Arc<_> = atomic::AtomicBool::new(true).into();

        // status_called signals when the "status" command has been
        // sent to the fake.
        let (status_send, status_called) = mpsc::channel();

        // Create a new fake with our status notify channel + set our
        // warm status value as the warm parameter.
        let mut f = Fake::with_status_notify(status_send);
        f.in_warmup = in_warm_status.clone();

        let (warmup_done_send, warmup_done_recv) = mpsc::channel();

        // Start polling for warmup in a new thread, and capture the
        // warmup status.
        thread::spawn(move || {
            warmup_done_send
                .send(f.wait_warmup(|_d| {
                    thread::sleep(time::Duration::from_millis(100));
                }))
                .unwrap();
        });

        // In a separate thread, we wait for the loop to poll the status
        // at least once, and then mark the status as "warm".
        {
            let in_warm_status = in_warm_status.clone();
            thread::spawn(move || {
                status_called.recv().unwrap();
                in_warm_status.store(false, atomic::Ordering::SeqCst);
            });
        }

        assert_eq!(
            warmup_done_recv
                .recv_timeout(time::Duration::from_secs(5))
                .map_err(|_| "warmup timed out after 5s")
                .unwrap()
                .unwrap(),
            (),
        );
        // Make sure that status was called at least once, so the fake was
        // warmed up.
        assert!(!in_warm_status.load(atomic::Ordering::SeqCst));
    }

    #[test]
    fn test_set_elevation() {
        let mut f = Fake::default();

        f.set_elevation(wire::Distance::Feet(1500)).unwrap();
        assert_eq!(f.read_elevation(), Ok(wire::Distance::Feet(1500)));

        f.set_elevation(wire::Distance::Feet(923)).unwrap();
        assert_eq!(f.read_elevation(), Ok(wire::Distance::Feet(1000)));

        f.set_elevation(wire::Distance::Feet(2270)).unwrap();
        assert_eq!(f.read_elevation(), Ok(wire::Distance::Feet(2500)));
    }

    #[test]
    fn test_calibrate_co2() {
        let mut f = Fake::default();
        let in_calibration = sync::Arc::new(atomic::AtomicBool::from(false));
        f.in_calibration = in_calibration.clone();
        assert_eq!(f.reference, wire::Concentration::PPM(0));

        let (calibrated_tx, calibrated_rx) = mpsc::channel();
        thread::spawn(move || {
            let sleep_fn = move |_d| {
                // The fake device automatically goes into calibration once the
                // calibration starts, so this will only be called once the
                // device starts waiting for calibration to be complete. When
                // that happens, we just automatically move it out of
                // calibration mode.
                in_calibration.store(false, atomic::Ordering::SeqCst);
            };
            f.calibrate_co2(wire::Concentration::PPM(400), sleep_fn)
                .unwrap();
            calibrated_tx.send(f).unwrap();
        });

        assert_eq!(
            calibrated_rx
                .recv_timeout(time::Duration::from_secs(5))
                .map_err(|_| "warmup timed out")
                .unwrap()
                .reference,
            wire::Concentration::PPM(400),
        );
    }
}
