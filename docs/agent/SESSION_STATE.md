# Session State

## Status
IN_PROGRESS

## Milestone
V0.5 Embedded HAL Implementation

## Current task
Implement USB MIDI interface for nRF5340.

## Last completed
- Fixed CI workspace resolution issues by excluding firmware from the main `Cargo.toml` members.
- Configured `.gitignore` to prevent tracking of firmware `target/` directories and purged git history.
- Scaffolded `firmwares/nrf5340` with `embassy-nrf` using the correct Cortex-M33 TrustZone Secure feature (`nrf5340-app-s`).
- Validated cross-compilation with `cargo clippy --target thumbv8m.main-none-eabihf`.

## Next action
Write the USB MIDI Class descriptors using `embassy-usb` in `firmwares/nrf5340/src/main.rs` to expose the virtual MIDI port over the nRF5340's USB connection.

## Verification
- cargo fmt --check: OK
- cargo clippy: OK
- cargo test: OK

## Blockers
None.
