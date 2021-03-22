# co2-sensor

![Test](https://github.com/joshkunz/co2-sensor/workflows/Test/badge.svg)

This repository contains the code for a simple CO2 sensor that exports
metrics using the [prometheus](https://prometheus.io/) monitoring format, and
potentially others in the future.

## Assembly 

### Parts

The hardware is relatively simple, only using 3 parts:

* A [Telaire T6615](https://www.amphenol-sensors.com/en/telaire/co2/525-co2-sensor-modules/319-t6615).
  This may be overkill. A T6613 may also work. I think all Telaire CO2 sensors
  use the same protocol, so any other Telaire sensore should work as well.
* A [Raspberry Pi Zero W](https://www.raspberrypi.org/products/raspberry-pi-zero-w/)
  Any other device capable of UART and 5 volt supply would work as well. All
  Pi variants should work.
* A logic-level converter between 3.3v and 5v. The RPi's UART is 3.3v and the
  Telaire sensor's logic operates at 5v so a converter is needed. I'm using an
  [Adafruit 4-Channel Converter](https://www.adafruit.com/product/757), but
  other cheaper converters should work as well.

### Wiring Schematic

![A schematic showing how the raspberry pi connects through the logic-level
converter to the Telaire T6615](docs/wiring-diagram.png)

TODO: Write something nicer.

### Configuring the Raspberry Pi Zero

*   Use the [RPI OS Imager](https://www.raspberrypi.org/documentation/installation/installing-images/README.md)
    to install the image onto an SD Card. Install `Raspberry Pi OS Lite`.
*   On the boot partition, add an empty file named `ssh`.
*   On the booth partition, add a `wpa_supplicant.conf` file as instructed
    [here](https://www.raspberrypi.org/documentation/configuration/wireless/headless.md).
*   Boot the device, and connect via SSH.
*   Run `sudo raspi-config`
*   In `raspi-config`: `1 System Options > S1 Wireless Lan`, then setup WiFi.
*   In `raspi-config`: `3 Interface Options > P2 SSH`, then enable SSH.
*   In `raspi-config`: `3 Interface Options > P6 Serial Port`, then disable
    login over serial, and enable the serial port. **Do not** enable login over
    the serial port, it will make serial communication unreliable.
*   Quit and re-boot.

### Building + Loading the Program

* Install [Rust's `cross`](https://github.com/rust-embedded/cross#installation)
  and start the Docker daemon as mentioned in the instructions.
* In `backend/` run:
  
  ```
  cross build --target=arm-unknown-linux-gnueabihf --release
  ```
  
  This builds a release binary of the backend that can run on a Raspberry Pi
  Zero W.
* Copy the binary over to the Raspberry Pi. Still in `backend/` run:
  
  ```
  RASPBERRY_PI_IP=10.0...  # Replace with actual IP
  scp target/arm-unknown-linux-gnueabihf/release/co2 pi@$RASPBERRY_PI_IP:~
  ```

  To copy the server binary to the Raspberry Pi.
* In `frontend/`, run:

  ```
  rm -r dist/  # to clean up any previous builds.
  yarn run parcel build src/index.html
  ```

  To build the frontend.
* Still in `frontend/` copy the frontend onto the Pi using a command like:

  ```
  RASPBERRY_PI_IP=10.0....  # Replace with actual IP
  rsync -hurt dist/ pi@$RASPBERRY_PI_IP:~/frontend
  ```
* Finally, ssh onto the Pi, and run the server:
  
  ```
  RASPBERRY_PI_IP=10.0...  # Replace with actual IP
  ssh pi@$RASPBERRY_PI_IP
  # Now on the PI
  sudo ./co2 ./frontend /dev/serial0
  # Note, you may need to use /dev/serial1 if not using a Raspberry Pi Zero W
  ```

### Configuring the Sensor

Configuration of the sensor is done through the sensor's web interface. Browse
to the web interface by going to `http://<your raspberry pi IP>` in your web
browser of choice, or `http://raspberrypi.local` (potentially substituting for a
different hostname if you've changed it). On that page is a "Calibrate" button,
when pressed it will walk through the calibration process.

NOTE: Calibration currently doesn't signal very will if calibration fails, but
info will be printed in the server's log if the device fails to calibrate
correctly, so check there after calibration.

### Reading Measurements

The current CO2 reading is displayed on the web-interface at
`http://<your raspberry pi IP>`. The web interface also provides a `/metrics`
endpoint (`http://<your raspberry pi IP>/metrics`) that can be scraped by
the open source [Prometheus](https://prometheus.io/) monitoring software.
