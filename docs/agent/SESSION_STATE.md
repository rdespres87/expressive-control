# Session State

## Status
IN_PROGRESS

## Milestone
V0.6 ToF Theremin Prototype (ESP32-S3)

## Current task
Design and implement a debug telemetry system for tracing MIDI messages, raw sensor values, and processor state.

## Last completed
- Implemented actual VL53L4CD I2C driver with dual-sensor address initialization (`run_master`).
- Implemented TinyUSB Native USB-MIDI transport.
- Defined UART telemetry protocol (`TunerState`) and integrated Slint UI.
- Simulated tuning search gliding in `theremin_scenario.yml`.
- Finalized UI tuner design as a high-resolution strobe-style radial LED gauge (101 segments).

## Next action
- Design the debug telemetry structure (what events to trace, how to output them: RTT, UART, or USB Serial).
- Postpone the Slint touch menu interactivity for later.

## Verification
- cargo fmt --check: OK
- cargo check: OK
- cargo clippy: OK
- Simulator passes `cargo run -p simulator -- simulator/theremin_scenario.yml`.

## Blockers
- The `espup` toolchain (`xtensa-esp32s3-espidf`) is needed to compile the firmware locally, preventing `cargo build` on the firmware crate for now.
