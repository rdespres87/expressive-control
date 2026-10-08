# Session State

## Status
IN_PROGRESS

## Milestone
V0.6 ToF Theremin Prototype (ESP32-S3)

## Current task
Implement Composite USB device (MIDI + CDC) via TinyUSB to support Hardware-In-The-Loop (HIL) simulation.

## Last completed
- Added Master Config Slint UI and refined Tuner UI logic.
- Implemented `ConfigMessage` pipeline (Master UI -> Simulator -> ControlProcessor) to dynamically configure octave, scale, quantization, and filter speed.
- Adjusted Simulator scenarios with realistic volume tracking.

## Next action
- Investigate TinyUSB Composite Device setup for ESP32-S3 (USB MIDI + USB CDC).
- We need the USB CDC interface so the PC simulator can inject sensor values via HIL.

## Verification
- cargo fmt --check: OK
- cargo check: OK
- cargo clippy: OK
- Simulator passes `cargo run -p simulator -- simulator/theremin_scenario.yml`.

## Blockers
- The `espup` toolchain (`xtensa-esp32s3-espidf`) is needed to compile the firmware locally, preventing `cargo build` on the firmware crate for now.
