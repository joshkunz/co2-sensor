# co2-sensor

![Test](https://github.com/joshkunz/co2-sensor/workflows/Test/badge.svg)

This repository contains the code for a simple CO2 sensor that exports
metrics using the [prometheus](https://prometheus.io/) monitoring format, and
potentially others in the future.

This project is a work-in-progress. TODO:

* Proper configuration and calibration for the sensor.
* On-device display.
* Detailed cross-build/deployment/wiring docs.

## Assembly 

### Parts

The project is relatively simple, only using 3 parts:

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

### Configuring the Raspberry Pi

TODO

### Wiring Schematic

TODO

### Building + Loading the Program

TODO

### Configuring the Sensor

TODO

### Reading Measurements

TODO

## EXtra

### Raspberry Pi-2 Setup

1. Use `raspi-config > Interface Options > Serial`, disable the login console,
   and enable the serial device. Communication is not reliable without it.
