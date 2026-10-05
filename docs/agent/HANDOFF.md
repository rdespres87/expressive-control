# Handoff

## Context
The `expressive-core` logic is complete and has been proven locally via the `simulator` crate. The simulator successfully routes simulated touches, applies voice stealing, and serializes perfectly formatted MPE MIDI 1.0 hex bytes.

## Next Action
The next milestone is to make the code run on bare metal. Focus on `firmwares/nrf5340`. Implement the hardware-specific implementations of the `SensorReader` trait (e.g., reading from an I2C/SPI sensor) and `MidiTransport` trait (e.g., writing to USB MIDI endpoint) using the chosen embedded runtime (Embassy or Zephyr).

## Verification
The workspace is green, CI is configured and passing.
