use std::array;
use std::convert::{TryFrom, TryInto};
use std::ops::Deref;
use std::result;
use std::string;

#[derive(Debug, PartialEq, Clone)]
pub struct Payload(pub Vec<u8>);

impl Deref for Payload {
    type Target = Vec<u8>;

    fn deref(&self) -> &Vec<u8> {
        let Payload(bs) = self;
        return bs;
    }
}

impl From<Payload> for Vec<u8> {
    fn from(p: Payload) -> Vec<u8> {
        let Payload(bs) = p;
        return bs;
    }
}

impl Default for Payload {
    fn default() -> Payload {
        Payload(Vec::new())
    }
}

#[derive(Debug, PartialEq)]
pub struct Message(Vec<u8>);

impl From<Payload> for Message {
    fn from(p: Payload) -> Message {
        assert!(p.len() <= (u8::MAX as usize));
        let bs: Vec<u8> = vec![0xFF, 0xFE, (p.len() as u8)]
            .into_iter()
            .chain(Vec::from(p).into_iter())
            .collect();
        return Message(bs);
    }
}

impl Deref for Message {
    type Target = Vec<u8>;

    fn deref(&self) -> &Vec<u8> {
        let Message(bs) = self;
        return bs;
    }
}

#[derive(Debug, PartialEq)]
pub struct Request {
    flag: u8,
    address: u8,
    payload: Payload,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Variable {
    GasPPM,
    SerialNumber,
    CompileSubvol,
    CompileDate,
    Elevation,
}

impl From<Variable> for u8 {
    fn from(v: Variable) -> Self {
        match v {
            Variable::GasPPM => 0x03,
            Variable::SerialNumber => 0x01,
            Variable::CompileSubvol => 0x0D,
            Variable::CompileDate => 0x0C,
            Variable::Elevation => 0x0F,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Toggle {
    On,
    Off,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Distance {
    Feet(u16),
}

impl Distance {
    pub fn feet(&self) -> u16 {
        let Distance::Feet(f) = self;
        return *f;
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Concentration {
    PPM(u16),
}

impl Concentration {
    pub fn ppm(&self) -> u16 {
        let Concentration::PPM(p) = self;
        return *p;
    }
}

#[derive(Debug, PartialEq)]
pub struct ParseError(String);

impl ToString for ParseError {
    fn to_string(&self) -> String {
        let ParseError(s) = self;
        return s.clone();
    }
}

impl From<String> for ParseError {
    fn from(s: String) -> ParseError {
        ParseError(s)
    }
}

impl From<&str> for ParseError {
    fn from(s: &str) -> ParseError {
        ParseError(s.to_string())
    }
}

impl From<chrono::ParseError> for ParseError {
    fn from(p: chrono::ParseError) -> ParseError {
        ParseError(format!("chrono parse error: {}", p))
    }
}

impl From<string::FromUtf8Error> for ParseError {
    fn from(f: string::FromUtf8Error) -> ParseError {
        ParseError(format!("utf8 decode error: {}", f))
    }
}

impl From<array::TryFromSliceError> for ParseError {
    fn from(t: array::TryFromSliceError) -> ParseError {
        ParseError(format!("cannot corce slice to array: {}", t))
    }
}

type Result<T> = result::Result<T, ParseError>;

pub mod command {
    use super::*;

    #[derive(Debug, PartialEq, Clone)]
    pub struct Read(pub Variable);

    impl From<Read> for Payload {
        fn from(r: Read) -> Self {
            let Read(v) = r;
            Payload(vec![0x02, v.into()])
        }
    }

    #[derive(Debug, PartialEq, Clone)]
    pub struct UpdateElevation(pub Distance);

    impl From<UpdateElevation> for Payload {
        fn from(u: UpdateElevation) -> Self {
            let UpdateElevation(d) = u;
            let bytes: [u8; 2] = d.feet().to_be_bytes();
            Payload(vec![0x03, 0x0F, bytes[0], bytes[1]])
        }
    }

    impl TryFrom<Payload> for UpdateElevation {
        type Error = ParseError;

        fn try_from(p: Payload) -> Result<UpdateElevation> {
            if !p.starts_with(&vec![0x03, 0x0F]) {
                return Err(ParseError::from(
                    "invalid command code for update elevation",
                ));
            }
            let raw: [u8; 2] = Vec::from(&p[2..])
                .try_into()
                .expect("should have two bytes");
            let value = u16::from_be_bytes(raw);
            return Ok(UpdateElevation(Distance::Feet(value)));
        }
    }

    #[derive(Debug, PartialEq, Clone)]
    pub struct Warmup;

    impl From<Warmup> for Payload {
        fn from(_: Warmup) -> Self {
            Payload(vec![0x84])
        }
    }

    #[derive(Debug, PartialEq, Clone)]
    pub struct StartSinglePointCalibration;

    impl From<StartSinglePointCalibration> for Payload {
        fn from(_: StartSinglePointCalibration) -> Payload {
            Payload(vec![0x9B])
        }
    }

    #[derive(Debug, PartialEq, Clone)]
    pub struct VerifySinglePointCalibration;

    impl From<VerifySinglePointCalibration> for Payload {
        fn from(_: VerifySinglePointCalibration) -> Payload {
            Payload(vec![0x02, 0x11])
        }
    }

    impl TryFrom<Payload> for VerifySinglePointCalibration {
        type Error = ParseError;

        fn try_from(p: Payload) -> Result<VerifySinglePointCalibration> {
            if Vec::from(p) != vec![0x02, 0x11] {
                return Err(ParseError::from(
                    "wrong command bytes for verify single point calibration",
                ));
            }
            return Ok(VerifySinglePointCalibration);
        }
    }

    #[derive(Debug, PartialEq, Clone)]
    pub struct SetSinglePointPPM(pub Concentration);

    impl From<SetSinglePointPPM> for Payload {
        fn from(s: SetSinglePointPPM) -> Payload {
            let SetSinglePointPPM(c) = s;
            let bytes: [u8; 2] = c.ppm().to_be_bytes();
            Payload(vec![0x03, 0x11, bytes[0], bytes[1]])
        }
    }

    impl TryFrom<Payload> for SetSinglePointPPM {
        type Error = ParseError;
        fn try_from(p: Payload) -> Result<SetSinglePointPPM> {
            if !p.starts_with(&vec![0x03, 0x11]) {
                return Err(ParseError::from("incorrect command bytes"));
            }
            let value = u16::from_be_bytes(p[2..].try_into()?);
            return Ok(SetSinglePointPPM(Concentration::PPM(value)));
        }
    }

    #[derive(Debug, PartialEq, Clone)]
    pub struct Status;

    impl From<Status> for Payload {
        fn from(_: Status) -> Payload {
            Payload(vec![0xB6])
        }
    }

    #[derive(Debug, PartialEq, Clone)]
    pub struct Idle(pub Toggle);

    impl From<Idle> for Payload {
        fn from(i: Idle) -> Payload {
            match i {
                Idle(Toggle::On) => Payload(vec![0xB9, 0x01]),
                Idle(Toggle::Off) => Payload(vec![0xB9, 0x02]),
            }
        }
    }

    #[derive(Debug, PartialEq, Clone)]
    pub struct ABCLogic;

    impl From<ABCLogic> for Payload {
        fn from(_: ABCLogic) -> Payload {
            Payload(vec![0xB7, 0x00])
        }
    }

    #[derive(Debug, PartialEq, Clone)]
    pub struct SetABCLogic(pub Toggle);

    impl From<SetABCLogic> for Payload {
        fn from(s: SetABCLogic) -> Payload {
            match s {
                SetABCLogic(Toggle::On) => Payload(vec![0xB7, 0x01]),
                SetABCLogic(Toggle::Off) => Payload(vec![0xB7, 0x02]),
            }
        }
    }

    #[derive(Debug, PartialEq, Clone)]
    pub struct ResetABCLogic;

    impl From<ResetABCLogic> for Payload {
        fn from(_: ResetABCLogic) -> Payload {
            Payload(vec![0xB7, 0x03])
        }
    }

    #[derive(Debug, PartialEq, Clone)]
    pub struct Halt;

    impl From<Halt> for Payload {
        fn from(_: Halt) -> Payload {
            Payload(vec![0x95])
        }
    }

    #[derive(Debug, PartialEq, Clone)]
    pub struct Loopback(pub Vec<u8>);

    impl From<Loopback> for Payload {
        fn from(l: Loopback) -> Payload {
            let Loopback(vs) = l;
            assert!(vs.len() <= 16);
            let res: Vec<u8> = vec![0x00].into_iter().chain(vs.into_iter()).collect();
            return Payload(res);
        }
    }

    #[derive(Debug, PartialEq, Clone)]
    pub struct StartSelfTest;

    impl From<StartSelfTest> for Payload {
        fn from(_: StartSelfTest) -> Payload {
            Payload(vec![0xC0, 0x00])
        }
    }

    #[derive(Debug, PartialEq, Clone)]
    pub struct SelfTestResults;

    impl From<SelfTestResults> for Payload {
        fn from(_: SelfTestResults) -> Payload {
            Payload(vec![0xC0, 0x01])
        }
    }

    #[derive(Debug, PartialEq, Clone)]
    pub struct StreamData;

    impl From<StreamData> for Payload {
        fn from(_: StreamData) -> Payload {
            Payload(vec![0xBD])
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_read() {
            assert_eq!(
                Payload::from(command::Read(Variable::GasPPM)),
                Payload(vec![0x02, 0x03]),
            );
            assert_eq!(
                Payload::from(command::Read(Variable::Elevation)),
                Payload(vec![0x02, 0x0F]),
            );
            assert_eq!(
                Payload::from(command::Read(Variable::SerialNumber)),
                Payload(vec![0x02, 0x01]),
            );
        }

        #[test]
        fn test_update_elevation() {
            assert_eq!(
                Payload::from(command::UpdateElevation(Distance::Feet(0xAABB))),
                Payload(vec![0x03, 0x0F, 0xAA, 0xBB]),
            );
            // 1500ft seems like a normal elevation. The controller expects
            // elevation to be in 500ft increments.
            assert_eq!(
                Payload::from(command::UpdateElevation(Distance::Feet(1500))),
                Payload(vec![0x03, 0x0F, 0x05, 0xDC]),
            );
        }

        #[test]
        fn test_warmup() {
            assert_eq!(Payload::from(command::Warmup), Payload(vec![0x84]));
        }

        #[test]
        fn test_single_point_calibration() {
            assert_eq!(
                Payload::from(command::StartSinglePointCalibration),
                Payload(vec![0x9B]),
            );
            assert_eq!(
                Payload::from(command::VerifySinglePointCalibration),
                Payload(vec![0x02, 0x11]),
            );
            assert_eq!(
                Payload::from(command::SetSinglePointPPM(Concentration::PPM(0xAABB))),
                Payload(vec![0x03, 0x11, 0xAA, 0xBB]),
            );
            // 400 PPM is a common CO2 measurement.
            assert_eq!(
                Payload::from(command::SetSinglePointPPM(Concentration::PPM(400))),
                Payload(vec![0x03, 0x11, 0x01, 0x90]),
            );
        }

        #[test]
        fn test_status() {
            assert_eq!(Payload::from(command::Status), Payload(vec![0xB6]));
        }

        #[test]
        fn test_idle() {
            assert_eq!(
                Payload::from(command::Idle(Toggle::On)),
                Payload(vec![0xB9, 0x01]),
            );
            assert_eq!(
                Payload::from(command::Idle(Toggle::Off)),
                Payload(vec![0xB9, 0x02]),
            );
        }

        #[test]
        fn test_abc_logic() {
            assert_eq!(Payload::from(command::ABCLogic), Payload(vec![0xB7, 0x00]),);
            assert_eq!(
                Payload::from(command::SetABCLogic(Toggle::On)),
                Payload(vec![0xB7, 0x01]),
            );
            assert_eq!(
                Payload::from(command::SetABCLogic(Toggle::Off)),
                Payload(vec![0xB7, 0x02]),
            );
            assert_eq!(
                Payload::from(command::ResetABCLogic),
                Payload(vec![0xB7, 0x03]),
            );
        }

        #[test]
        fn test_halt() {
            assert_eq!(Payload::from(command::Halt), Payload(vec![0x95]));
        }

        #[test]
        fn test_loopback() {
            assert_eq!(
                Payload::from(command::Loopback(vec![0x01, 0x02, 0x03, 0xAA])),
                Payload(vec![0x00, 0x01, 0x02, 0x03, 0xAA]),
            );
            assert_eq!(
                Payload::from(command::Loopback(vec![])),
                Payload(vec![0x00])
            );
        }

        #[test]
        fn test_self_test() {
            assert_eq!(
                Payload::from(command::StartSelfTest),
                Payload(vec![0xC0, 0x00])
            );
            assert_eq!(
                Payload::from(command::SelfTestResults),
                Payload(vec![0xC0, 0x01])
            );
        }

        #[test]
        fn test_stream_data() {
            assert_eq!(Payload::from(command::StreamData), Payload(vec![0xBD]));
        }
    }
}

pub mod response {
    use super::*;
    use chrono;

    #[derive(Debug, PartialEq)]
    pub struct Ack;

    impl TryFrom<Payload> for Ack {
        type Error = ParseError;
        fn try_from(p: Payload) -> Result<Ack> {
            if p.len() != 0 {
                return Err(ParseError::from("payload not empty"));
            }
            return Ok(Ack);
        }
    }

    impl From<Ack> for Payload {
        fn from(_a: Ack) -> Payload {
            Payload::default()
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct GasPPM(Concentration);

    impl GasPPM {
        pub fn with_ppm(p: u16) -> GasPPM {
            GasPPM(Concentration::PPM(p))
        }

        pub fn concentration(&self) -> Concentration {
            let GasPPM(c) = self;
            return *c;
        }
    }

    impl TryFrom<Payload> for GasPPM {
        type Error = ParseError;
        fn try_from(p: Payload) -> Result<GasPPM> {
            if p.len() != 2 {
                return Err(ParseError::from("payload should consist of 2 bytes"));
            }
            let raw: [u8; 2] = Vec::from(p).try_into().expect("as per assertion");
            let value = u16::from_be_bytes(raw);
            return Ok(GasPPM(Concentration::PPM(value)));
        }
    }

    impl From<GasPPM> for Payload {
        fn from(g: GasPPM) -> Payload {
            let bytes: [u8; 2] = g.concentration().ppm().to_be_bytes();
            return Payload(Vec::from(bytes));
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct SerialNumber(String);

    impl TryFrom<Payload> for SerialNumber {
        type Error = ParseError;

        fn try_from(p: Payload) -> Result<SerialNumber> {
            if p.len() > 15 {
                return Err(ParseError::from("payload should have 15 bytes"));
            }
            let bytes: Vec<u8> = Vec::from(p).into_iter().take_while(|v| *v != 0x0).collect();
            return Ok(SerialNumber(String::from_utf8(bytes)?));
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct CompileSubvol(String);

    impl TryFrom<Payload> for CompileSubvol {
        type Error = ParseError;

        fn try_from(p: Payload) -> Result<CompileSubvol> {
            if p.len() != 3 {
                return Err(ParseError::from("invalid subvol"));
            }
            return Ok(CompileSubvol(String::from_utf8(Vec::from(p))?));
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct CompileDate(pub chrono::NaiveDate);

    impl TryFrom<Payload> for CompileDate {
        type Error = ParseError;

        fn try_from(p: Payload) -> Result<CompileDate> {
            if p.len() != 6 {
                return Err(ParseError::from("invalid date length"));
            }
            let date_raw: String = String::from_utf8(p.into())?;
            let date = chrono::NaiveDate::parse_from_str(&date_raw, "%y%m%d")?;
            return Ok(CompileDate(date));
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct Elevation(pub Distance);

    impl TryFrom<Payload> for Elevation {
        type Error = ParseError;

        fn try_from(p: Payload) -> Result<Elevation> {
            if p.len() != 2 {
                return Err(ParseError::from("elevation should be 2 bytes"));
            }
            // Should always succeed due to preceeding length check.
            let num = u16::from_be_bytes(Vec::from(p).try_into().unwrap());
            return Ok(Elevation(Distance::Feet(num)));
        }
    }

    impl From<Elevation> for Payload {
        fn from(e: Elevation) -> Payload {
            let Elevation(d) = e;
            let bytes: [u8; 2] = d.feet().to_be_bytes();
            return Payload(Vec::from(bytes));
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct Status {
        v: u8,
    }

    // Read the true/false status of a bit in the given byte. idx is a
    // zero-index where 0 is the least significant bit.
    fn bit_at(v: u8, idx: u8) -> bool {
        (v >> idx) & 1 == 1
    }

    impl Status {
        pub fn is_err(&self) -> bool {
            bit_at(self.v, 0)
        }

        pub fn in_warmup(&self) -> bool {
            bit_at(self.v, 1)
        }

        pub fn in_calibration(&self) -> bool {
            bit_at(self.v, 2)
        }

        pub fn in_idle(&self) -> bool {
            bit_at(self.v, 3)
        }

        pub fn in_self_test(&self) -> bool {
            bit_at(self.v, 7)
        }
    }

    impl TryFrom<Payload> for Status {
        type Error = ParseError;

        fn try_from(p: Payload) -> Result<Status> {
            if p.len() != 1 {
                return Err(ParseError::from("status should be a single byte"));
            }
            return Ok(Status { v: p[0] });
        }
    }

    impl From<Status> for Payload {
        fn from(s: Status) -> Payload {
            return Payload(vec![s.v]);
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct StatusFlags {
        pub in_err: bool,
        pub in_warmup: bool,
        pub in_calibration: bool,
        pub in_idle: bool,
        pub in_self_test: bool,
    }

    impl Default for StatusFlags {
        fn default() -> StatusFlags {
            return StatusFlags {
                in_err: false,
                in_warmup: false,
                in_calibration: false,
                in_idle: false,
                in_self_test: false,
            };
        }
    }

    fn set_bit_at(v: bool, idx: u8) -> u8 {
        if !v {
            return 0b0;
        }
        return 1 << idx;
    }

    impl From<StatusFlags> for Status {
        fn from(sf: StatusFlags) -> Status {
            let status_byte = set_bit_at(sf.in_err, 0)
                | set_bit_at(sf.in_warmup, 1)
                | set_bit_at(sf.in_calibration, 2)
                | set_bit_at(sf.in_idle, 3)
                | set_bit_at(sf.in_self_test, 7);
            return Status { v: status_byte };
        }
    }

    #[derive(Debug, PartialEq)]
    pub enum ABCState {
        On,
        Off,
    }

    impl TryFrom<Payload> for ABCState {
        type Error = ParseError;

        fn try_from(p: Payload) -> Result<ABCState> {
            if p.len() != 1 {
                return Err(ParseError::from("ABC state should be a single byte"));
            }
            match p[0] {
                0x1 => Ok(ABCState::On),
                0x2 => Ok(ABCState::Off),
                unk => Err(ParseError::from(format!(
                    "ABC State {:#X} not recognized",
                    unk
                ))),
            }
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct Loopback(pub Vec<u8>);

    // doesn't actually need to be TryFrom, because it cannot fail, but this
    // is just to follow the pattern of the other response message types.
    impl TryFrom<Payload> for Loopback {
        type Error = ParseError;

        fn try_from(p: Payload) -> Result<Loopback> {
            Ok(Loopback(Vec::from(p)))
        }
    }

    #[derive(Debug, PartialEq)]
    enum SelfTestStatus {
        Unknown,
        Ok,
    }

    #[derive(Debug, PartialEq)]
    enum TestResult {
        Pass,
        Fail,
    }

    #[derive(Debug, PartialEq)]
    struct SelfTest {
        status: SelfTestStatus,
        result: TestResult,
        good_dsp: u8,
        total_dsp: u8,
    }

    impl SelfTest {
        pub fn passed(&self) -> bool {
            return self.status == SelfTestStatus::Ok
                && self.result == TestResult::Pass
                && self.good_dsp == self.total_dsp;
        }

        pub fn total_dsp_cycles(&self) -> u8 {
            return self.total_dsp;
        }
    }

    impl TryFrom<Payload> for SelfTest {
        type Error = ParseError;
        fn try_from(p: Payload) -> Result<SelfTest> {
            if p.len() != 4 {
                return Err(ParseError::from("expected exactly 4 bytes"));
            }
            let flag = match p[0] {
                0x0F => SelfTestStatus::Ok,
                _ => SelfTestStatus::Unknown,
            };
            let result = match p[1] {
                0x01 => TestResult::Pass,
                0x00 => TestResult::Fail,
                unk => {
                    return Err(ParseError::from(format!(
                        "unrecognized test result {:#X}",
                        unk
                    )))
                }
            };
            return Ok(SelfTest {
                status: flag,
                result: result,
                good_dsp: p[2],
                total_dsp: p[3],
            });
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::ops::Not;

        #[test]
        fn test_ack() {
            assert_eq!(Ack::try_from(Payload(vec![])), Ok(Ack));
            assert!(
                Ack::try_from(Payload(vec![0x0])).is_err(),
                "Only empty payload is ACK"
            );
        }

        #[test]
        fn test_gas_ppm() {
            assert_eq!(
                GasPPM::try_from(Payload(vec![0xAA, 0xBB])),
                Ok(GasPPM(Concentration::PPM(0xAABB))),
            );
            assert_eq!(
                GasPPM::try_from(Payload(vec![0x01, 0x90])),
                Ok(GasPPM(Concentration::PPM(400))),
            );
            assert!(
                GasPPM::try_from(Payload(vec![0x01])).is_err(),
                "GasPPM requires 2 bytes in the payload to parse",
            );
        }

        #[test]
        fn test_serial_number() {
            assert_eq!(
                SerialNumber::try_from(Payload(vec![b'a', b'b', b'c', b'd'])),
                Ok(SerialNumber(String::from("abcd"))),
            );
            // Make sure we strip trailing nulls.
            assert_eq!(
                SerialNumber::try_from(Payload(vec![b'x', 0x0, 0x0])),
                Ok(SerialNumber(String::from("x"))),
            );
        }

        #[test]
        fn test_compile_subvol() {
            assert_eq!(
                CompileSubvol::try_from(Payload(vec![b'A', b'1', b'0'])),
                Ok(CompileSubvol(String::from("A10"))),
            );
        }

        #[test]
        fn test_compile_date() {
            assert_eq!(
                CompileDate::try_from(Payload("060708".bytes().collect())),
                Ok(CompileDate(chrono::NaiveDate::from_ymd(2006, 7, 8))),
            );
        }

        #[test]
        fn test_elevation() {
            assert_eq!(
                Elevation::try_from(Payload(vec![0xAA, 0xBB])),
                Ok(Elevation(Distance::Feet(0xAABB))),
            );
            assert_eq!(
                Elevation::try_from(Payload(vec![0x05, 0xDC])),
                Ok(Elevation(Distance::Feet(1500))),
            );
        }

        #[test]
        fn test_status() {
            // Helper to generate a status from the byte.
            fn status_of(b: u8) -> Status {
                Status::try_from(Payload(vec![b])).expect("want parse")
            }
            assert!(!status_of(0b0).is_err());
            assert!(status_of(0b1).is_err());
            assert!(status_of(0b10).in_warmup());
            assert!(status_of(0b100).in_calibration());
            assert!(status_of(0b1000).in_idle());
            assert!(status_of(0b10000000).in_self_test());

            let s: Status = status_of(0b101);
            // Should be able to have multiple statuses.
            assert!(s.is_err() && s.in_calibration());
        }

        #[test]
        fn test_abc_state() {
            assert_eq!(ABCState::try_from(Payload(vec![0x01])), Ok(ABCState::On),);
            assert_eq!(ABCState::try_from(Payload(vec![0x02])), Ok(ABCState::Off),);
            assert!(ABCState::try_from(Payload(vec![0x0])).is_err());
        }

        #[test]
        fn test_loopback() {
            assert_eq!(
                Loopback::try_from(Payload(vec![0xa, 0xb, 0xc])),
                Ok(Loopback(vec![0xa, 0xb, 0xc])),
            );
        }

        #[test]
        fn test_self_test() {
            assert!(SelfTest::try_from(Payload(vec![0x0F, 0x01, 12, 12]))
                .expect("should parse correctly")
                .passed(),);
            // Unknown status code.
            assert!(SelfTest::try_from(Payload(vec![0x0, 0x01, 12, 12]))
                .expect("should parse correctly")
                .passed()
                .not(),);
            // Failure result.
            assert!(SelfTest::try_from(Payload(vec![0x0F, 0x00, 12, 12]))
                .expect("should parse correctly")
                .passed()
                .not(),);
            // Mismatched DSP count.
            assert!(SelfTest::try_from(Payload(vec![0x0F, 0x01, 11, 12]))
                .expect("should parse correctly")
                .passed()
                .not(),);
            // Bad result code, should be 0x01 or 0x00.
            assert!(SelfTest::try_from(Payload(vec![0x0F, 0x03, 11, 12])).is_err(),);
        }
    }
}
