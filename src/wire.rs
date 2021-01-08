#[derive(Debug, PartialEq)]
pub struct Payload(Vec<u8>);

#[derive(Debug, PartialEq)]
pub struct Request {
    flag: u8,
    address: u8,
    payload: Payload,
}

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
pub enum Toggle {
    On,
    Off,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Distance {
    Feet(u16),
}

impl Distance {
    pub fn feet(&self) -> u16 {
        let Distance::Feet(f) = self;
        return *f;
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Concentration {
    PPM(u16),
}

impl Concentration {
    pub fn ppm(&self) -> u16 {
        let Concentration::PPM(p) = self;
        return *p;
    }
}

pub mod command {
    use super::*;

    #[derive(Debug, PartialEq)]
    pub struct Read(pub Variable);

    impl From<Read> for Payload {
        fn from(r: Read) -> Self {
            let Read(v) = r;
            Payload(vec![0x02, v.into()])
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct UpdateElevation(pub Distance);

    impl From<UpdateElevation> for Payload {
        fn from(u: UpdateElevation) -> Self {
            let UpdateElevation(d) = u;
            let bytes: [u8; 2] = d.feet().to_be_bytes();
            Payload(vec![0x03, 0x0F, bytes[0], bytes[1]])
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct Warmup;

    impl From<Warmup> for Payload {
        fn from(_: Warmup) -> Self {
            Payload(vec![0x84])
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct StartSinglePointCalibration;

    impl From<StartSinglePointCalibration> for Payload {
        fn from(_: StartSinglePointCalibration) -> Payload {
            Payload(vec![0x9B])
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct VerifySinglePointCalibration;

    impl From<VerifySinglePointCalibration> for Payload {
        fn from(_: VerifySinglePointCalibration) -> Payload {
            Payload(vec![0x02, 0x11])
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct SetSinglePointPPM(pub Concentration);
    
    impl From<SetSinglePointPPM> for Payload {
        fn from(s: SetSinglePointPPM) -> Payload {
            let SetSinglePointPPM(c) = s;
            let bytes: [u8; 2] = c.ppm().to_be_bytes();
            Payload(vec![0x03, 0x11, bytes[0], bytes[1]])
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct Status;

    impl From<Status> for Payload {
        fn from(_: Status) -> Payload {
            Payload(vec![0xB6])
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct Idle(pub Toggle);

    impl From<Idle> for Payload {
        fn from(i: Idle) -> Payload {
            match i {
                Idle(Toggle::On) => Payload(vec![0xB9, 0x01]),
                Idle(Toggle::Off) => Payload(vec![0xB9, 0x02]),
            }
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct ABCLogic;

    impl From<ABCLogic> for Payload {
        fn from(_: ABCLogic) -> Payload {
            Payload(vec![0xB7, 0x00])
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct SetABCLogic(pub Toggle);

    impl From<SetABCLogic> for Payload {
        fn from(s: SetABCLogic) -> Payload {
            match s {
                SetABCLogic(Toggle::On) => Payload(vec![0xB7, 0x01]),
                SetABCLogic(Toggle::Off) => Payload(vec![0xB7, 0x02]),
            }
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct ResetABCLogic;

    impl From<ResetABCLogic> for Payload {
        fn from(_: ResetABCLogic) -> Payload {
            Payload(vec![0xB7, 0x03])
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct Halt;

    impl From<Halt> for Payload {
        fn from(_: Halt) -> Payload {
            Payload(vec![0x95])
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct Loopback(pub Vec<u8>);

    impl From<Loopback> for Payload {
        fn from(l: Loopback) -> Payload {
            let Loopback(vs) = l;
            assert!(vs.len() <= 16);
            let res: Vec<u8> = vec![0x00].into_iter()
                .chain(vs.into_iter())
                .collect();
            return Payload(res);
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct StartSelfTest;

    impl From<StartSelfTest> for Payload {
        fn from(_: StartSelfTest) -> Payload {
            Payload(vec![0xC0, 0x00])
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct SelfTestResults;

    impl From<SelfTestResults> for Payload {
        fn from(_: SelfTestResults) -> Payload {
            Payload(vec![0xC0, 0x01])
        }
    }

    #[derive(Debug, PartialEq)]
    pub struct StreamData;

    impl From<StreamData> for Payload {
        fn from(_: StreamData) -> Payload {
            Payload(vec![0xBD])
        }
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
        assert_eq!(
            Payload::from(command::ABCLogic),
            Payload(vec![0xB7, 0x00]),
        );
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
            Payload::from(command::Loopback(vec![])), Payload(vec![0x00]));
    }

    #[test]
    fn test_self_test() {
        assert_eq!(
            Payload::from(command::StartSelfTest), Payload(vec![0xC0, 0x00]));
        assert_eq!(
            Payload::from(command::SelfTestResults), Payload(vec![0xC0, 0x01]));
    }

    #[test]
    fn test_stream_data() {
        assert_eq!(Payload::from(command::StreamData), Payload(vec![0xBD]));
    }
}
