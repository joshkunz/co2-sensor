use std::result;
use std::convert::TryFrom;
use std::io::{Read, Write};
use serialport;
use crate::wire;
use std::time;

#[derive(Debug, PartialEq)]
pub struct Error(String);

impl<T: ToString> From<T> for Error {
    fn from(v: T) -> Error {
        Error(v.to_string())
    }
}

type Result<T> = result::Result<T, Error>;

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
                .timeout(time::Duration::from_secs(5))
        )?;

        return Ok(T6615{
            port: port,
        });
    }

    pub fn send<T: Into<wire::Payload>>(&mut self, v: T) -> Result<()> {
        let p: wire::Payload = v.into();
        if p.len() > u8::MAX.into() {
            return Err(Error::from("payload too long"));
        }
        let msg: Vec<u8> = vec![0xFF, 0xFE, p.len() as u8].into_iter()
            .chain(Vec::from(p).into_iter())
            .collect();
        self.port.write_all(&msg)?;
        return Ok(());
    }

    pub fn recv<E, T>(&mut self) -> Result<T> 
    where
        E: ToString,
        T: TryFrom<wire::Payload, Error=E>
    {
        // We can only represent messages up to size 256 + 4, so use a buffer of
        // size 300 (nice round number with some buffer).
        let mut raw: [u8; 300] = [0; 300];
        let _size = self.port.read(&mut raw)?;
        let hdr = &raw[0..3];
        if hdr[0] != 0xFF {
            return Err(Error::from(
                format!("incorrect Tsunami flag: {:#X}", hdr[0])));
        }
        if hdr[1] != 0xFA {
            return Err(Error::from(
                format!("incorrect Tsunami address: {:#X}", hdr[1])));
        }
        let length: usize = hdr[2] as usize;
        // 4, because 4 is the first byte after the header. Then +length
        // to grab the size of the payload.
        let p = wire::Payload(raw[4..(4 + length)].into());
        return Ok(T::try_from(p)?);
    }
}
