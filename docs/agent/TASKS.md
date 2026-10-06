# Tasks

## Completed
- [x] Project architecture and rules definition
- [x] Git repository initialization and GitHub remote link
- [x] Cargo workspace setup (core, simulator, firmwares/nrf5340)
- [x] English translation of all documentation
- [x] GitHub Actions CI setup
- [x] Define core foundation traits (SensorReader, MidiTransport, SensorEvent, InstrumentEvent)
- [x] Design MPE Voice Allocator and `ControlProcessor` trait interfaces
- [x] Implement `ControlProcessor` internal routing logic (mapping events to MPE channels)
- [x] Implement raw MIDI byte serialization in `expressive-core`
- [x] Implement a basic PC Simulator driver for end-to-end testing (YAML scenario -> processor -> MIDI hex dump)
- [x] Scaffold Embassy nRF5340 App Core firmware with TrustZone secure feature (`nrf5340-app-s`) and fix workspace CI conflicts.

## Pending (nRF5340 Base)
- [x] Implement USB MIDI Class Compliant transport for nRF5340 (`embassy-usb`)
- [x] Implement I2C/SPI sensor reader for nRF5340 (`embassy-nrf`)

## ToF Theremin Project (ESP32-S3)
- [x] Hardware feasibility study and component selection (ESP32-S3, VL53L5CX, Slint UI)
- [x] Add `SensorEvent::Position3D` definitions to `expressive-core`
- [x] Implement `TofThereminProcessor` logic in `instruments/theremin`
- [x] Initialize new Cargo workspace member `firmwares/expressive-theremin` (`esp-idf` based)
- [x] Integrate PC Simulator multi-instrument dispatch and HIL theremin scenario
- [x] Implement VL53L4CD I2C driver with dual-sensor address initialization
- [x] Implement UART/Hairless MIDI transport for ESP32-S3
- [ ] Integrate Slint UI interface for the 5" touch display
