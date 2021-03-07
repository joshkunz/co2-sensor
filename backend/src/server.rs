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
    fn calibrate(&self) -> ();
    fn is_ready(&self) -> bool;
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

    fn maybe_lock_device(&self) -> Result<sync::MutexGuard<D>> {
        let _dev = match self.device.try_lock() {
            Ok(guard) => guard,
            Err(sync::TryLockError::WouldBlock) => {
                return Err(Error::from("rate limited, but no measurement taken"));
            }
            // Just panic if we get a poisoned/other error. This shouldn't
            // happen, and indicates a run-time bug.
            e @ Err(_) => e.unwrap(),
        };
        return Ok(_dev);
    }
}

impl<D, C> Manager for DeviceManager<D, C>
where
    D: Device + Send + 'static,
    C: governor::clock::Clock + Send + Sync + 'static,
{
    fn is_ready(&self) -> bool {
        // If we can lock the device, then we're "ready" to receive
        // measurements.
        return self.maybe_lock_device().is_ok();
    }

    fn measure(&self) -> Result<wire::Concentration> {
        let mut last_measure = self.last_measure.lock().unwrap();
        if self.limiter.check().is_err() {
            // We're rate-limited. Just return the previous measure.
            return Ok(last_measure.expect(
                "Since this only triggers when we are rate limited, \
                         there should always be a value in this option.",
            ));
        }
        let measurement = self.maybe_lock_device()?.read_co2()?;
        *last_measure = Some(measurement);
        return Ok(measurement);
    }

    fn calibrate(&self) -> () {
        let (calibration_started, calibration_in_progress) = sync::mpsc::channel();
        let mgr = (*self).clone();
        thread::spawn(move || {
            let mut dev = mgr.device.lock().unwrap();
            calibration_started.send(()).unwrap();
            // TODO(jkz): Actually communicate the failure to calibrate
            // somehow. Logs? Lockup the manager? Callback?
            let _ = dev.calibrate_co2(AMBIENT_CONCENTRATION, thread::sleep);
        });
        calibration_in_progress.recv().unwrap();
        return;
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

impl<M> Server<M> {
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
}

impl<D: Device> Server<DeviceManager<D, governor::clock::DefaultClock>> {
    fn with_device(dev: D) -> Self {
        return Server::new(DeviceManager::new(dev));
    }
}

impl<M: Manager + Clone + Send + Sync + 'static> Server<M> {
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

    fn render_put_calibrate(self) -> impl warp::Reply {
        // TODO(jkz): Handle this error correctly.
        self.manager.calibrate();
        return warp::reply();
    }

    fn render_is_ready(self) -> impl warp::Reply {
        return warp::reply::json(&self.manager.is_ready());
    }

    fn routes(&self) -> warp::filters::BoxedFilter<(impl warp::Reply,)> {
        let srv = (*self).clone();
        let self_filter = warp::any().map(move || srv.clone());
        let metrics = warp::path!("metrics")
            .and(self_filter.clone())
            .map(Self::render_metrics);
        let calibrate = warp::path!("calibrate")
            .and(self_filter.clone())
            .map(Self::render_put_calibrate);
        let is_ready = warp::path!("isready")
            .and(self_filter.clone())
            .map(Self::render_is_ready);
        return metrics.or(calibrate).or(is_ready).boxed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[derive(Default)]
    struct _FakeDeviceData {
        co2: Option<wire::Concentration>,
        reference: Option<wire::Concentration>,
        calibrate_called_signal: Option<sync::mpsc::Sender<()>>,
        calibrate_wait_signal: Option<sync::mpsc::Receiver<()>>,
    }

    #[derive(Clone)]
    struct FakeDevice {
        data: sync::Arc<sync::Mutex<_FakeDeviceData>>,
    }

    impl Device for FakeDevice {
        fn read_co2(&mut self) -> Result<wire::Concentration> {
            let data = self.data.lock().unwrap();
            return match data.co2 {
                Some(c) => Ok(c),
                None => Err(Error::from("no concentration set on fake")),
            };
        }

        fn calibrate_co2<T: Fn(time::Duration)>(
            &mut self,
            reference: wire::Concentration,
            _sleep_fn: T,
        ) -> Result<()> {
            let mut data = self.data.lock().unwrap();
            if let Some(chan) = &data.calibrate_called_signal {
                chan.send(()).unwrap();
            }
            data.reference = Option::from(reference);
            if let Some(chan) = &data.calibrate_wait_signal {
                chan.recv_timeout(time::Duration::from_secs(30)).unwrap();
            }
            return Err(Error::from("not implemented"));
        }
    }

    impl FakeDevice {
        fn set_co2(&self, to: wire::Concentration) {
            let mut data = self.data.lock().unwrap();
            data.co2 = Option::from(to);
        }

        fn reference(&self) -> Option<wire::Concentration> {
            let data = self.data.lock().unwrap();
            return data.reference;
        }
    }

    #[derive(Default)]
    struct FakeBuilder {
        data: _FakeDeviceData,
    }

    impl FakeBuilder {
        fn with_co2(mut self, c: wire::Concentration) -> Self {
            self.data.co2 = Option::from(c);
            return self;
        }

        fn with_calibrate_called_signal(mut self, c: sync::mpsc::Sender<()>) -> Self {
            self.data.calibrate_called_signal = Option::from(c);
            return self;
        }

        fn with_calibrate_wait_signal(mut self, c: sync::mpsc::Receiver<()>) -> Self {
            self.data.calibrate_wait_signal = Option::from(c);
            return self;
        }

        fn build(self) -> FakeDevice {
            return FakeDevice {
                data: sync::Mutex::new(self.data).into(),
            };
        }
    }

    #[test]
    fn test_manager_double_read() {
        let fake = FakeBuilder::default()
            .with_co2(wire::Concentration::PPM(200))
            .build();
        let clock = governor::clock::FakeRelativeClock::default();
        let mgr = DeviceManager::new_with_clock(fake.clone(), &clock);

        assert_eq!(mgr.measure().unwrap(), wire::Concentration::PPM(200));

        // Now we update the fake's CO2 concentration, but don't move forward
        // time. The manager should rate-limit the request, and we should see
        // stale data.

        fake.set_co2(wire::Concentration::PPM(55));
        assert_eq!(mgr.measure().unwrap(), wire::Concentration::PPM(200));

        // Advance *just* past the max measure rate, so we can trigger another
        // measurement. Then we should see the updated value.
        clock.advance(MAX_MEASURE_RATE + time::Duration::from_secs(1));
        assert_eq!(mgr.measure().unwrap(), wire::Concentration::PPM(55));

        // Just for good measure, change the concentration back, and
        // make sure we see latch the updated concentration.
        fake.set_co2(wire::Concentration::PPM(200));
        assert_eq!(mgr.measure().unwrap(), wire::Concentration::PPM(55));
    }

    #[test]
    fn test_metrics() {
        // Arrange.
        let fake = FakeBuilder::default()
            .with_co2(wire::Concentration::PPM(100))
            .build();
        let srv = Server::with_device(fake);

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

    #[test]
    fn test_calibration_basic() {
        let (called_in, called_out) = sync::mpsc::channel();
        let fake = FakeBuilder::default()
            .with_calibrate_called_signal(called_in)
            .build();
        let srv = Server::with_device(fake.clone());

        let mut rt = tokio::runtime::Runtime::new().unwrap();
        let reply = rt.block_on(async {
            warp::test::request()
                .method("PUT")
                .path("/calibrate")
                .reply(&srv.routes())
                .await
        });
        assert_eq!(reply.status(), 200);

        // Make sure that calibrate is actually called on the device (i.e.,
        // calibration has started). Note: Even though calibration should
        // be called almost immediately, it's important to use a channel here
        // because it's not guarnteed to be called when /calibrate returns.
        assert!(called_out
            .recv_timeout(time::Duration::from_secs(5))
            .is_ok());

        // And since we know calibrate has been called, make sure the
        // reference concentration was set to the ambient concentration.
        assert_eq!(fake.reference(), Some(AMBIENT_CONCENTRATION));
    }

    // TODO(jkz): This is a mediocre test. It should fail when if `wait_in.send`
    // is never called. Currently, if the calibration thread panics, it's not
    // visibile to this test.
    #[test]
    fn test_is_ready() {
        let (started_in, started_out) = sync::mpsc::channel();
        let (wait_in, wait_out) = sync::mpsc::channel();
        let fake = FakeBuilder::default()
            .with_calibrate_called_signal(started_in)
            .with_calibrate_wait_signal(wait_out)
            .build();
        let mgr = DeviceManager::new(fake.clone());
        let srv = Server::new(mgr.clone());

        let mut rt = tokio::runtime::Runtime::new().unwrap();
        let mut is_ready = || -> bool {
            let reply = rt.block_on(async {
                warp::test::request()
                    .path("/isready")
                    .reply(&srv.routes())
                    .await
            });
            assert_eq!(reply.status(), 200);

            // Should return a json-encoded bool saying that we're ready.
            return serde_json::from_slice(reply.body()).unwrap();
        };

        // No calibrate ongoing, the device should be ready for measurements.
        assert!(is_ready());

        // Start a calibration, plus make sure the calibration thread is going.
        mgr.calibrate();
        started_out
            .recv_timeout(time::Duration::from_secs(5))
            .unwrap();

        // Device should not be ready in calibration.
        assert!(!is_ready());

        // Let the calibration finish, and make sure we've returned to the
        // ready state.
        wait_in.send(()).unwrap();

        // TODO(jkz): Figure out a better way to make sure that the calibration
        // thread has terminated. For now, we use a 250ms grace period.
        thread::sleep(time::Duration::from_millis(250));

        assert!(is_ready());
    }
}
