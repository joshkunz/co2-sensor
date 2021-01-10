use std::result;
use std::convert::TryFrom;
use std::io::{Read, Write};
use serialport;
use crate::wire;
use std::time;
use std::thread;

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
                .timeout(time::Duration::from_secs(1))
        )?;

        return Ok(T6615{
            port: port,
        });
    }

    pub fn execute_once<S, T, E>(&mut self, s: S) -> Result<T>
    where
        S: Into<wire::Payload>,
        T: TryFrom<wire::Payload, Error=E>,
        E: ToString
    {
        let out_p: wire::Payload = s.into();
        if out_p.len() > u8::MAX.into() {
            return Err(Error::from("payload too long"));
        }
        let msg: Vec<u8> = vec![0xFF, 0xFE, out_p.len() as u8].into_iter()
            .chain(Vec::from(out_p).into_iter())
            .collect();
        println!("SEN: {:?}", &msg);
        self.port.write_all(&msg)?;

        let mut hdr: [u8; 3] = Default::default();
        self.port.read_exact(&mut hdr)?;
        println!("HDR: {:?}", hdr);
        if hdr[0] != 0xFF {
            return Err(Error::from(
                format!("incorrect Tsunami flag: {:#X}", hdr[0])));
        }
        if hdr[1] != 0xFA {
            return Err(Error::from(
                format!("incorrect Tsunami address: {:#X}", hdr[1])));
        }
        let length: usize = hdr[2] as usize;
        let mut body: Vec<u8> = Vec::with_capacity(length);
        // Though body has 'length' capacity, it is still "empty", so it
        // is coereced to an empty slice. Here we reserve 'length'
        // bytes so it will have non-zero size.
        body.resize(length, 0);
        self.port.read_exact(&mut body)?;
        println!("BDY: {:?}", body);
        return Ok(T::try_from(wire::Payload(body))?);
    }

    pub fn execute<S, T, E>(&mut self, s: S) -> Result<T> 
    where
        S: Into<wire::Payload> + Clone,
        T: TryFrom<wire::Payload, Error=E>,
        E: ToString
    {
        let tries = 10;
        // Rust cannot deduce that last_err would be assigned
        // by the time this is used, so we set a dummy error
        // to make the compiler happy.
        let mut last_err: Error = Error::from("this should not be returned");
        for _ in 0..tries {
            match self.execute_once(s.clone()) {
                Ok(v) => return Ok(v),
                Err(e) => last_err = e,
            }
            thread::sleep(time::Duration::from_secs(1));
        }
        return Err(last_err);
    }
}
