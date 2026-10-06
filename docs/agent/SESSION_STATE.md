# Session State

## Status
IN_PROGRESS

## Milestone
V0.6 ToF Theremin Prototype (ESP32-S3)

## Current task
Select next component to implement (VL53L5CX Driver, USB MIDI, or GUI).

## Last completed
- Scaffolding of the `firmwares/expressive-theremin` with `esp-idf` for ESP32-S3.
- Refactored `main.rs` to use an interrupt-driven async architecture (`wait_for_falling_edge().await`).
- Resolved Cargo workspace profile compatibility issues.
- Updated `simulator` to support dynamic dispatch and `Position3D` theremin simulation testing.
- Created `theremin_scenario.yml` for local testing on Mac/PC.

## Next action
- Implement actual VL53L5CX I2C communication in the firmware.

## Verification
- cargo fmt --check: OK
- cargo check: OK
- cargo clippy: OK
- Simulator passes `cargo run -p simulator -- simulator/theremin_scenario.yml` without crashing.

## Blockers
- The `espup` toolchain (`xtensa-esp32s3-espidf`) is needed to compile the firmware locally, preventing `cargo build` on the firmware crate for now.
