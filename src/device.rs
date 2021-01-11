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

pub type Result<T> = result::Result<T, Error>;

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

    pub fn execute<S, T, E>(&mut self, s: S) -> Result<T>
    where
        S: Into<wire::Payload>,
        E: ToString,
        T: TryFrom<wire::Payload, Error = E>,
    {
        // Convert the command into the wire message format.
        let out_p: wire::Payload = s.into();
        if out_p.len() > u8::MAX.into() {
            return Err(Error::from("payload too long"));
        }
        // Construct the message with the flag/address/length header.
        let msg: Vec<u8> = vec![0xFF, 0xFE, out_p.len() as u8]
            .into_iter()
            .chain(Vec::from(out_p).into_iter())
            .collect();
        // Send the message.
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
