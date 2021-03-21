use crate::device;
use crate::wire;
use gotham::hyper;
use gotham::router::builder::*;
use governor;
use http;
use mime;
use prometheus;
use prometheus::Encoder;
use serde;
use std::io;
use std::panic::RefUnwindSafe;
use std::result;
use std::sync;
use std::thread;
use std::time;

use gotham;
use gotham::helpers::http::response as gotham_response;
use gotham::middleware::state::StateMiddleware;
use gotham::state::FromState;
use gotham::state::State as GothamState;

// Based on https://www.esrl.noaa.gov/gmd/ccgg/trends/. Ambient concentration
// should be +/-5ppm ish.
const AMBIENT_CONCENTRATION: wire::Concentration = wire::Concentration::PPM(410);

// The maximum rate that we can take measurements from a device. Basically
// a random guess.
const MAX_MEASURE_RATE: time::Duration = time::Duration::from_secs(15);

// The approximate height of Mt. Everest. Used for sanity-checking the
// given elevation on configureation.
const MT_EVEREST_HEIGHT: wire::Distance = wire::Distance::Feet(29_000);

#[derive(Debug)]
pub struct Error(String);

impl Error {
    fn to_response(self) -> http::Response<hyper::Body> {
        return http::response::Builder::default()
            .status(http::StatusCode::INTERNAL_SERVER_ERROR)
            .body(hyper::Body::from(self.0))
            .unwrap();
    }
}

impl From<&str> for Error {
    fn from(e: &str) -> Error {
        return Error(e.to_string());
    }
}

impl From<String> for Error {
    fn from(e: String) -> Error {
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

impl ToString for Error {
    fn to_string(&self) -> String {
        return self.0.clone();
    }
}

type Result<T> = result::Result<T, Error>;

pub trait Device {
    fn read_co2(&mut self) -> Result<wire::Concentration>;
    fn calibrate_co2<T: Fn(time::Duration)>(
        &mut self,
        reference: wire::Concentration,
        sleep_fn: T,
    ) -> Result<()>;
    fn read_elevation(&mut self) -> Result<wire::Distance>;
    fn set_elevation(&mut self, to: wire::Distance) -> Result<()>;
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

    fn read_elevation(&mut self) -> Result<wire::Distance> {
        return self.read_elevation().map_err(Error::from);
    }

    fn set_elevation(&mut self, to: wire::Distance) -> Result<()> {
        return self.set_elevation(to).map_err(Error::from);
    }
}

pub trait Manager {
    fn measure(&self) -> Result<wire::Concentration>;
    fn elevation(&self) -> Result<wire::Distance>;
    fn calibrate(&self) -> ();
    fn is_ready(&self) -> bool;
    fn configure_elevation(&self, to: wire::Distance) -> Result<()>;
}

type RateLimiter<C> =
    governor::RateLimiter<governor::state::direct::NotKeyed, governor::state::InMemoryState, C>;

pub struct DeviceManager<D, C: governor::clock::Clock> {
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

    fn elevation(&self) -> Result<wire::Distance> {
        return self.maybe_lock_device()?.read_elevation();
    }

    fn configure_elevation(&self, to: wire::Distance) -> Result<()> {
        return self.maybe_lock_device()?.set_elevation(to);
    }
}

pub struct Server<M> {
    registry: sync::Arc<sync::Mutex<prometheus::Registry>>,
    manager: M,
    co2_metric: prometheus::Gauge,
    static_dir: String,
}

impl<M: Clone> Clone for Server<M> {
    fn clone(&self) -> Self {
        return Server {
            registry: self.registry.clone(),
            manager: self.manager.clone(),
            co2_metric: self.co2_metric.clone(),
            static_dir: self.static_dir.clone(),
        };
    }
}

pub struct Builder<M> {
    manager: Option<M>,
    static_dir: String,
}

impl<M> Default for Builder<M> {
    fn default() -> Self {
        return Builder {
            manager: None,
            static_dir: String::new(),
        };
    }
}

impl<M> Builder<M> {
    pub fn manager(&mut self, manager: M) -> &mut Self {
        self.manager = Some(manager);
        return self;
    }

    pub fn static_dir(&mut self, dir: &'_ str) -> &mut Self {
        self.static_dir = String::from(dir);
        return self;
    }

    pub fn build(self) -> Result<Server<M>> {
        return Ok(Server::new(
            self.manager.ok_or(Error::from("No manager provided"))?,
            &self.static_dir,
        ));
    }
}

impl<D: Device> Builder<DeviceManager<D, governor::clock::DefaultClock>> {
    pub fn device(&mut self, device: D) -> &mut Self {
        self.manager = Some(DeviceManager::new(device));
        return self;
    }
}

impl<M> Server<M> {
    fn new(manager: M, static_dir: &'_ str) -> Self {
        let registry = prometheus::Registry::new();
        // TODO(jkz): These errors should be propogated probably.
        let co2_metric = prometheus::Gauge::new(
            "co2_ppm",
            "The current concentration of CO2 in the air in parts per million",
        )
        .unwrap();
        registry.register(Box::new(co2_metric.clone())).unwrap();

        return Server {
            registry: sync::Arc::new(sync::Mutex::new(registry)),
            manager: manager,
            co2_metric: co2_metric,
            static_dir: String::from(static_dir),
        };
    }
}
fn json_response<J: serde::Serialize>(value: &J) -> http::Response<hyper::Body> {
    let builder = http::response::Builder::default();
    let maybe_resp = match serde_json::to_vec(value) {
        Ok(enc) => builder
            .status(200)
            .header("Content-Type", mime::APPLICATION_JSON.to_string())
            .body(hyper::Body::from(enc)),
        Err(err) => return Error::from(err.to_string()).to_response(),
    };
    return match maybe_resp {
        Ok(r) => r,
        Err(e) => Error::from(e.to_string()).to_response(),
    };
}

impl<M: Manager + Clone + Send + Sync + 'static + RefUnwindSafe> gotham::state::StateData
    for Server<M>
{
}

impl<M: Manager + Clone + Send + Sync + 'static + RefUnwindSafe> Server<M> {
    fn render_metrics(mut state: GothamState) -> (GothamState, http::Response<hyper::Body>) {
        let srv = Self::take_from(&mut state);
        match srv.manager.measure() {
            Ok(c) => srv.co2_metric.set(c.ppm() as f64),
            Err(e) => return (state, e.to_response()),
        };

        let enc = prometheus::TextEncoder::new();
        let mut out: Vec<u8> = Vec::new();

        let registry = srv.registry.lock().unwrap();

        if let Err(e) = enc.encode(&registry.gather(), &mut out) {
            return (state, Error::from(e.to_string()).to_response());
        }
        let resp =
            gotham_response::create_response(&state, http::StatusCode::OK, mime::TEXT_PLAIN, out);
        return (state, resp);
    }

    fn render_put_calibrate(state: GothamState) -> (GothamState, http::Response<hyper::Body>) {
        let srv = Self::borrow_from(&state);
        // TODO(jkz): Handle this error correctly.
        srv.manager.calibrate();
        // Return an empty 200.
        let resp = gotham_response::create_empty_response(&state, http::StatusCode::OK);
        return (state, resp);
    }

    fn render_is_ready(state: GothamState) -> (GothamState, http::Response<hyper::Body>) {
        let srv = Self::borrow_from(&state);
        let resp = json_response(&srv.manager.is_ready());
        return (state, resp);
    }

    fn render_co2(state: GothamState) -> (GothamState, http::Response<hyper::Body>) {
        let srv = Self::borrow_from(&state);
        return match srv.manager.measure() {
            Ok(concentration) => (state, json_response(&concentration.ppm())),
            Err(e) => (state, e.to_response()),
        };
    }

    fn render_elevation(state: GothamState) -> (GothamState, http::Response<hyper::Body>) {
        let srv = Self::borrow_from(&state);
        return match srv.manager.elevation() {
            Ok(d) => (state, json_response(&d.feet())),
            Err(e) => (state, e.to_response()),
        };
    }

    async fn render_put_elevation(mut state: GothamState) -> gotham::handler::HandlerResult {
        let body = match hyper::body::to_bytes(hyper::Body::take_from(&mut state)).await {
            Ok(bytes) => bytes,
            Err(e) => return Ok((state, Error::from(e.to_string()).to_response())),
        };
        let to_configure_raw: u16 = match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(e) => return Ok((state, Error::from(e.to_string()).to_response())),
        };

        let to_configure = wire::Distance::Feet(to_configure_raw);
        // TODO(jkz): allow comparison of these types directly.
        if to_configure.feet() > MT_EVEREST_HEIGHT.feet() {
            return Ok((
                state,
                Error::from(format!(
                    "height {} ft. does not exist on earth",
                    to_configure.feet()
                ))
                .to_response(),
            ));
        }

        let srv = Self::borrow_from(&state);
        return Ok(match srv.manager.configure_elevation(to_configure) {
            Ok(_) => {
                let resp = gotham_response::create_empty_response(&state, http::StatusCode::OK);
                (state, resp)
            }
            Err(e) => (state, e.to_response()),
        });
    }

    pub fn routes(&self) -> gotham::router::Router {
        let srv: Server<M> = self.clone();
        let srv_middleware = StateMiddleware::new(srv);
        let (chain, pipelines) = gotham::pipeline::single::single_pipeline(
            gotham::pipeline::single_middleware(srv_middleware),
        );

        return gotham::router::builder::build_router(chain, pipelines, |route| {
            route.get("/metrics").to(Self::render_metrics);
            route.get("/co2").to(Self::render_co2);
            route.get("/isready").to(Self::render_is_ready);
            route.put("/calibrate").to(Self::render_put_calibrate);
            route.get("/elevation").to(Self::render_elevation);
            route.put("/elevation").to_async(Self::render_put_elevation);

            if !self.static_dir.is_empty() {
                route.get("/").to_dir(self.static_dir.clone());
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gotham::test::TestServer;

    #[derive(Default)]
    struct _FakeDeviceData {
        co2: Option<wire::Concentration>,
        reference: Option<wire::Concentration>,
        elevation: Option<wire::Distance>,
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
            return Ok(());
        }

        fn read_elevation(&mut self) -> Result<wire::Distance> {
            let data = self.data.lock().unwrap();
            return match data.elevation {
                Some(d) => Ok(d),
                None => Err(Error::from("no elevation set on fake")),
            };
        }

        fn set_elevation(&mut self, to: wire::Distance) -> Result<()> {
            let mut data = self.data.lock().unwrap();
            data.elevation = Option::from(to);
            return Ok(());
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

        fn elevation(&self) -> Option<wire::Distance> {
            let data = self.data.lock().unwrap();
            return data.elevation;
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

        fn with_elevation(mut self, d: wire::Distance) -> Self {
            self.data.elevation = Option::from(d);
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

    fn read_json<T: serde::de::DeserializeOwned>(r: gotham::test::TestResponse) -> Result<T> {
        let body = match r.read_utf8_body() {
            Ok(v) => v,
            Err(e) => return Err(Error::from(e.to_string())),
        };
        return match serde_json::from_str(&body) {
            Ok(v) => Ok(v),
            Err(e) => Err(Error::from(e.to_string())),
        };
    }

    #[test]
    fn test_metrics() {
        // Arrange.
        let fake = FakeBuilder::default()
            .with_co2(wire::Concentration::PPM(100))
            .build();
        let mut builder = Builder::default();
        builder.device(fake);
        let srv = builder.build().unwrap();

        let test_server = TestServer::new(srv.routes()).unwrap();
        let reply = test_server
            .client()
            .get("http://localhost/metrics")
            .perform()
            .unwrap();

        assert_eq!(reply.status(), 200);
        let body = reply.read_utf8_body().unwrap();
        assert!(body.contains("co2_ppm 100"));
    }

    #[test]
    fn test_calibration_basic() {
        let (called_in, called_out) = sync::mpsc::channel();
        let fake = FakeBuilder::default()
            .with_calibrate_called_signal(called_in)
            .build();
        let mut builder = Builder::default();
        builder.device(fake.clone());
        let srv = builder.build().unwrap();

        let test_server = TestServer::new(srv.routes()).unwrap();
        let reply = test_server
            .client()
            .put("http://localhost/calibrate", "", mime::APPLICATION_JSON)
            .perform()
            .unwrap();

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
        let srv = Server::new(mgr.clone(), "");

        let test_server = TestServer::new(srv.routes()).unwrap();

        let is_ready = || -> bool {
            let reply = test_server
                .client()
                .get("http://localhost/isready")
                .perform()
                .unwrap();
            assert_eq!(reply.status(), 200);

            // Should return a json-encoded bool saying that we're ready.
            return read_json(reply).unwrap();
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

    #[test]
    fn test_get_co2() {
        let want_measurement = wire::Concentration::PPM(198);
        let fake = FakeBuilder::default().with_co2(want_measurement).build();
        let mut builder = Builder::default();
        builder.device(fake.clone());
        let srv = builder.build().unwrap();

        let test_server = TestServer::new(srv.routes()).unwrap();
        let reply = test_server
            .client()
            .get("http://localhost/co2")
            .perform()
            .unwrap();

        assert_eq!(reply.status(), 200);
        let measurement: u16 = read_json(reply).unwrap();
        assert_eq!(measurement, want_measurement.ppm());
    }

    #[test]
    fn test_read_elevation() {
        let want_elevation = wire::Distance::Feet(1500);
        let fake = FakeBuilder::default()
            .with_elevation(want_elevation)
            .build();
        let mut builder = Builder::default();
        builder.device(fake.clone());
        let srv = builder.build().unwrap();

        let test_server = TestServer::new(srv.routes()).unwrap();
        let reply = test_server
            .client()
            .get("http://localhost/elevation")
            .perform()
            .unwrap();

        assert_eq!(reply.status(), 200);
        let elevation: u16 = read_json(reply).unwrap();
        assert_eq!(elevation, want_elevation.feet());
    }

    #[test]
    fn test_put_elevation() {
        let fake = FakeBuilder::default().build();
        let mut builder = Builder::default();
        builder.device(fake.clone());
        let srv = builder.build().unwrap();

        let test_server = TestServer::new(srv.routes()).unwrap();
        let reply = test_server
            .client()
            .put("http://localhost/elevation", "500", mime::APPLICATION_JSON)
            .perform()
            .unwrap();

        assert_eq!(reply.status(), 200);
        assert_eq!(fake.elevation(), Some(wire::Distance::Feet(500)));
    }
}
