# Handoff

The architecture for the ToF Theremin logic and the ESP32 firmware scaffolding has been fully completed!

## What was done
1. **Instrument Logic:** Extracted `TofThereminProcessor` to `instruments/theremin`, with default distance ranges mapped from `20mm` to `1000mm`.
2. **Firmware Scaffolding:** Initialized `firmwares/expressive-theremin` via `esp-idf-template`. Setup the `main.rs` loop to use **async hardware interrupts** (`PinDriver::input`) instead of busy-looping `thread::sleep`, minimizing CPU footprint and eliminating latency. 
3. **Simulation Strategy:** Extended the `simulator` crate with dynamic dispatch to run PC-based scenarios (`theremin_scenario.yml`) allowing end-to-end tests without flashing the ESP32.
4. **Workspace Config:** Moved `[profile.*]` configs to the root `Cargo.toml` and excluded the firmware from workspace pre-commits to prevent `xtensa` compile errors on the host PC.

## Next Agent Action
The foundations are solid. Your next goals should be:
1. **VL53L5CX Driver Integration:** Include an I2C driver in the firmware to read the 8x8 matrices. Pay attention to the `LPn` sequential boot process (detailed in `docs/studies/hardware_netlist.md`) to avoid I2C address collisions.
2. **USB-MIDI Driver:** Setup `usbd-midi` (or `esp-idf-usb`) to stream the generated `InstrumentEvent` buffer to the host PC via USB.
3. **GUI:** Scaffold Slint UI for the Elecrow 5.0" display.

## Context
Remember that `cargo check` inside `firmwares/expressive-theremin` requires `espup` and `cargo-espflash` installed on the host. The simulator is your best friend for testing logic on the host PC!
