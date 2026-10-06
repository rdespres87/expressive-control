# Session State

## Status
IN_PROGRESS

## Milestone
V0.6 ToF Theremin Prototype (ESP32-S3)

## Current task
Initialize new Cargo workspace member `firmwares/expressive-theremin` (`esp-idf` based).

## Last completed
- Added `SensorEvent::Position3D` to `expressive-core`.
- Created generic abstraction layer via `instruments/theremin` crate.
- Implemented `TofThereminProcessor` logic to map Z distance to PitchBend and Volume.
- Successfully passed CI and committed the changes.

## Next action
- Run `cargo new --bin firmwares/expressive-theremin`.
- Configure `esp-idf` and TrustZone/STD requirements.
- Import `expressive-core` and `instruments/theremin`.
- Scaffold the main I2C polling loop for VL53L5CX.

## Verification
- cargo fmt --check: OK
- cargo check: OK
- cargo clippy: OK
- Architecture cleanly decoupled (Core vs Instruments vs Firmware).

## Blockers
None.
