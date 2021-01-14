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

impl From<wire::response::ParseError> for Error {
    fn from(e: wire::response::ParseError) -> Error {
        Error(e.to_string())
    }
}

/// Result is the common result type used in this module.
pub type Result<T> = result::Result<T, Error>;

/// Device represents a device that can execute commands. This is useful
/// for testing purposes.
pub trait Device {
    fn execute<S, T, E>(&mut self, s: S) -> Result<T>
    where
        S: Into<wire::Payload>,
        E: ToString,
        T: TryFrom<wire::Payload, Error = E>;

    fn read_co2(&mut self) -> Result<wire::Concentration> {
        let r: wire::response::GasPPM = self.execute(
            wire::command::Read(wire::Variable::GasPPM))?;
        return Ok(r.concentration());
    }

    fn read_elevation(&mut self) -> Result<wire::Distance> {
        let wire::response::Elevation(d) = self.execute(
            wire::command::Read(wire::Variable::Elevation))?;
        return Ok(d);
    }
}

/// T6615 implements the `Device` trait for the Telaire T6615 CO2 module.
pub struct T6615 {
    port: serialport::TTYPort,
}

impl T6615 {
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
mod fake {
    use super::*;

    /// Fake implements the `Device` trait, but is not backed by a physical
    /// device. It can be used for testing.
    struct Fake {
        gas: wire::Concentration,
        elevation: wire::Distance,
    }

    impl Default for Fake {
        fn default() -> Self {
            return Fake{
                gas: wire::Concentration::PPM(0),
                elevation: wire::Distance::Feet(0),
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
    }

    impl Device for Fake {
        fn execute<S, T, E>(&mut self, s: S) -> Result<T>
        where
            S: Into<wire::Payload>,
            E: ToString,
            T: TryFrom<wire::Payload, Error = E>
        {
            let p: wire::Payload = s.into();
            let mut r: wire::Payload = Default::default();
            if p == wire::Payload::from(wire::command::Read(wire::Variable::GasPPM)) {
                r = wire::response::GasPPM::with_ppm(self.gas.ppm()).into();
            } else if p == wire::Payload::from(wire::command::Read(wire::Variable::Elevation)) {
                r = wire::response::Elevation(self.elevation).into();
            } else {
                return Err(Error::from("not implemented"));
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
}
