use crate::device;
use crate::wire;
use governor;
use prometheus;
use prometheus::Encoder;
use std::io;
use std::result;
use std::sync;
use std::thread;
use std::time;
use warp;
use warp::Filter;

// Based on https://www.esrl.noaa.gov/gmd/ccgg/trends/. Ambient concentration
// should be +/-5ppm ish.
const AMBIENT_CONCENTRATION: wire::Concentration = wire::Concentration::PPM(410);

// The maximum rate that we can take measurements from a device. Basically
// a random guess.
const MAX_MEASURE_RATE: time::Duration = time::Duration::from_secs(15);

#[derive(Debug)]
struct Error(String);

impl From<&str> for Error {
    fn from(e: &str) -> Error {
        return Error(e.to_string());
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        return Error(e.to_string());
    }
}

impl From<device::Error> for Error {
    fn from(e: device::Error) -> Error {
        return Error(e.to_string());
    }
}

impl From<sync::mpsc::RecvError> for Error {
    fn from(e: sync::mpsc::RecvError) -> Error {
        return Error(e.to_string());
    }
}

type Result<T> = result::Result<T, Error>;

trait Device {
    fn read_co2(&mut self) -> Result<wire::Concentration>;
    fn calibrate_co2<T: Fn(time::Duration)>(
        &mut self,
        reference: wire::Concentration,
        sleep_fn: T,
    ) -> Result<()>;
}

impl<D: device::Device> Device for D {
    fn read_co2(&mut self) -> Result<wire::Concentration> {
        return self.read_co2().map_err(Error::from);
    }

    fn calibrate_co2<T: Fn(time::Duration)>(
        &mut self,
        reference: wire::Concentration,
        sleep_fn: T,
    ) -> Result<()> {
        return self.calibrate_co2(reference, sleep_fn).map_err(Error::from);
    }
}

trait Manager {
    fn measure(&self) -> Result<wire::Concentration>;
    fn calibrate(&self) -> Result<()>;
}

type RateLimiter<C> =
    governor::RateLimiter<governor::state::direct::NotKeyed, governor::state::InMemoryState, C>;

struct DeviceManager<D, C: governor::clock::Clock> {
    device: sync::Arc<sync::Mutex<D>>,
    limiter: sync::Arc<RateLimiter<C>>,
    last_measure: sync::Arc<sync::Mutex<Option<wire::Concentration>>>,
}

impl<D, C: governor::clock::Clock> Clone for DeviceManager<D, C> {
    fn clone(&self) -> Self {
        return DeviceManager {
            device: self.device.clone(),
            limiter: self.limiter.clone(),
            last_measure: self.last_measure.clone(),
        };
    }
}

impl<D> DeviceManager<D, governor::clock::DefaultClock> {
    fn new(dev: D) -> Self {
        return DeviceManager::new_with_clock(dev, &governor::clock::DefaultClock::default());
    }
}

impl<D, C: governor::clock::Clock> DeviceManager<D, C> {
    fn new_with_clock(dev: D, clock: &C) -> Self {
        return DeviceManager {
            device: sync::Arc::new(sync::Mutex::from(dev)),
            // Only allow one measurement per 15s.
            limiter: sync::Arc::new(RateLimiter::direct_with_clock(
                governor::Quota::with_period(MAX_MEASURE_RATE)
                    .expect("Option should only be none for zero durations."),
                clock,
            )),
            last_measure: sync::Arc::new(sync::Mutex::new(Option::None)),
        };
    }
}

impl<D, C> Manager for DeviceManager<D, C>
where
    D: Device + Send + 'static,
    C: governor::clock::Clock + Send + Sync + 'static,
{
    fn measure(&self) -> Result<wire::Concentration> {
        let mut last_measure = self.last_measure.lock().unwrap();
        if self.limiter.check().is_err() {
            // We're rate-limited. Just return the previous measure.
            return match *last_measure {
                Some(v) => Ok(v),
                // TODO(jkz): Make it so this can't happen.
                None => Err(Error::from("rate limited, but no measurement taken")),
            };
        }
        let mut dev = match self.device.try_lock() {
            Ok(guard) => guard,
            Err(sync::TryLockError::WouldBlock) => {
                return Err(Error::from("the managed device is calibrating"))
            }
            // Just panic if we get a poisoned/other error. This shouldn't
            // happen, and indicates a run-time bug.
            e @ Err(_) => e.unwrap(),
        };
        let measurement = dev.read_co2()?;
        *last_measure = Some(measurement);
        return Ok(measurement);
    }

    fn calibrate(&self) -> Result<()> {
        let (calibration_started, calibration_in_progress) = sync::mpsc::channel();
        let mgr = (*self).clone();
        thread::spawn(move || {
            let mut dev = mgr.device.lock().unwrap();
            calibration_started.send(()).unwrap();
            // TODO(jkz): Actually communicate the failure to calibrate
            // somehow. Logs? Lockup the manager? Callback?
            let _ = dev.calibrate_co2(AMBIENT_CONCENTRATION, thread::sleep);
        });
        calibration_in_progress.recv()?;
        return Ok(());
    }
}

struct Server<M> {
    registry: sync::Arc<prometheus::Registry>,
    manager: M,
    co2_metric: prometheus::Gauge,
}

impl<M: Manager + Clone> Clone for Server<M> {
    fn clone(&self) -> Self {
        return Server {
            registry: self.registry.clone(),
            manager: self.manager.clone(),
            co2_metric: self.co2_metric.clone(),
        };
    }
}

impl<M: Manager + Clone + Send + Sync + 'static> Server<M> {
    fn new(manager: M) -> Self {
        let registry = prometheus::Registry::new();
        // TODO(jkz): These errors should be propogated probably.
        let co2_metric = prometheus::Gauge::new(
            "co2_ppm",
            "The current concentration of CO2 in the air in parts per million",
        )
        .unwrap();
        registry.register(Box::new(co2_metric.clone())).unwrap();

        return Server {
            registry: sync::Arc::new(registry),
            manager: manager,
            co2_metric: co2_metric,
        };
    }

    fn render_metrics(self) -> String {
        match self.manager.measure() {
            Ok(c) => self.co2_metric.set(c.ppm() as f64),
            Err(e) => println!("got err :( {:?}", e),
        };

        let enc = prometheus::TextEncoder::new();
        let mut out: Vec<u8> = Vec::new();

        if let Err(e) = enc.encode(&self.registry.gather(), &mut out) {
            return e.to_string();
        }
        return String::from_utf8(out).unwrap();
    }

    fn routes(&self) -> warp::filters::BoxedFilter<(impl warp::Reply,)> {
        let srv = (*self).clone();
        let metrics = warp::path!("metrics")
            .and(warp::any().map(move || srv.clone()))
            .map(Self::render_metrics);
        return metrics.boxed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    struct _FakeDeviceData {
        co2: sync::Mutex<Option<wire::Concentration>>,
    }

    #[derive(Clone)]
    struct FakeDevice {
        data: sync::Arc<_FakeDeviceData>,
    }

    impl FakeDevice {
        fn with_data(d: _FakeDeviceData) -> Self {
            return FakeDevice { data: d.into() };
        }
    }

    impl Device for FakeDevice {
        fn read_co2(&mut self) -> Result<wire::Concentration> {
            let current = self.data.co2.lock().unwrap();
            return match *current {
                Some(c) => Ok(c),
                None => Err(Error::from("no concentration set on fake")),
            };
        }

        fn calibrate_co2<T: Fn(time::Duration)>(
            &mut self,
            _reference: wire::Concentration,
            _sleep_fn: T,
        ) -> Result<()> {
            return Err(Error::from("not implemented"));
        }
    }

    #[test]
    fn test_manager_double_read() {
        let fake = FakeDevice::with_data(_FakeDeviceData {
            co2: Option::from(wire::Concentration::PPM(200)).into(),
        });
        let clock = governor::clock::FakeRelativeClock::default();
        let mgr = DeviceManager::new_with_clock(fake.clone(), &clock);

        assert_eq!(mgr.measure().unwrap(), wire::Concentration::PPM(200));

        // Now we update the fake's CO2 concentration, but don't move forward
        // time. The manager should rate-limit the request, and we should see
        // stale data.

        *fake.data.co2.lock().unwrap() = Option::from(wire::Concentration::PPM(55));
        assert_eq!(mgr.measure().unwrap(), wire::Concentration::PPM(200));

        // Advance *just* past the max measure rate, so we can trigger another
        // measurement. Then we should see the updated value.
        clock.advance(MAX_MEASURE_RATE + time::Duration::from_secs(1));
        assert_eq!(mgr.measure().unwrap(), wire::Concentration::PPM(55));

        // Just for good measure, change the concentration back, and
        // make sure we see latch the updated concentration.
        *fake.data.co2.lock().unwrap() = Option::from(wire::Concentration::PPM(200));
        assert_eq!(mgr.measure().unwrap(), wire::Concentration::PPM(55));
    }

    #[test]
    fn test_metrics() {
        // Arrange.
        let fake = FakeDevice::with_data(_FakeDeviceData {
            co2: Option::from(wire::Concentration::PPM(100)).into(),
        });
        let mgr = DeviceManager::new(fake);
        let srv = Server::new(mgr);

        let mut rt = tokio::runtime::Runtime::new().unwrap();
        let reply = rt.block_on(async {
            warp::test::request()
                .path("/metrics")
                .reply(&srv.routes())
                .await
        });

        assert_eq!(reply.status(), 200);
        let body = std::str::from_utf8(reply.body()).unwrap();
        println!("body:\n{}", body);
        assert!(body.contains("co2_ppm 100"));
    }
}
