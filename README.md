# Expressive Control

Rust project for a generic expressive music controller.

Architecture:
`Sensors -> Controls -> Instrument -> Renderer -> MIDI/MPE -> Transport`

The Core is `no_std`, independent of hardware. The host PC and nRF5340
are separate platforms. YAML/Lua remain exclusive to the simulator.

See:
- `docs/architecture.md`
- `docs/roadmap.md`
- `docs/agent/RULES.md`
- `docs/agent/IMPLEMENTATION_PLAN.md`
- `docs/agent/PROMPTS.md`

## Current Capabilities

- **Core (`no_std`)**: Hardware-agnostic logic for translating sensor events into MPE MIDI events.
- **Instruments (`no_std`)**: Reusable hardware-agnostic logic for specific instrument topologies (e.g., `instruments/theremin`).
- **PC Simulator**: Reads sensor events from YAML scenarios (`scenario.yml`, `theremin_scenario.yml`), processes them through the Core/Instruments, and outputs MPE MIDI to a virtual MIDI port named "Expressive MPE Out" (macOS/Linux) that you can connect to your DAW. Includes advanced Theremin simulations (e.g. playing "Somewhere Over the Rainbow" with multi-octave glissando and vibrato over a standard +/- 2 semitone pitch bend range).
- **Firmwares**: 
  - **nRF5340 (MPE Keyboard)**: Polls an I2C sensor array and outputs MIDI over USB. Supports HIL (Hardware-In-the-Loop) mode by receiving mocked sensor events from the PC simulator over UART.
  - **ESP32-S3 (ToF Theremin)**: *[WIP]* An `esp-idf` based firmware using VL53L4CD Time-of-Flight sensors mapped to pitch and volume, featuring a Slint UI on an Elecrow 5.0" Display.

## Commands

### General
```bash
cargo test          # Run unit and integration tests (Core & Simulator)
cargo fmt --check   # Check code formatting
```

### PC Simulation
```bash
# Run the PC Simulator (reads scenario.yml, creates a Virtual MIDI port)
cargo run -p simulator
```

### Firmware (nRF5340)
*Requires `probe-rs` to be installed.*

```bash
cd firmwares/nrf5340

# Run the standard firmware (I2C sensor -> USB MIDI)
cargo run

# Run the HIL firmware (UART injection -> USB MIDI)
cargo run --features hil
```

### Firmware (ESP32-S3 Theremin)
*Requires the `esp` rust toolchain (via `espup`).*

```bash
cd firmwares/expressive-theremin
cargo build
```

### Debug Telemetry
The core exposes generic telemetry events (Sensor readings, Voice allocations, Note generation). 
By default, these are filtered out to avoid console spam. To view them:

- **nRF5340 (defmt)**: Run with the `DEFMT_LOG` environment variable:
  ```bash
  DEFMT_LOG=trace cargo run
  ```
- **ESP32-S3 (esp-idf)**: The firmware uses the standard `log` crate. Telemetry is emitted at the `Debug` level. Configure your `sdkconfig.defaults` or `RUST_LOG=debug` to view them.

### Hardware-In-the-Loop (HIL)
To test the core logic directly on the hardware with deterministic inputs from your PC:

1. Flash and run the HIL firmware on the nRF5340:
   ```bash
   cd firmwares/nrf5340
   cargo run --features hil
   ```
2. In another terminal, run the PC Simulator in HIL mode, passing the nRF5340's serial port:
   ```bash
   # Replace with your actual serial port (e.g. /dev/tty.usbmodem1234 or /dev/ttyACM0)
   cargo run -p simulator -- /dev/tty.usbmodemXXXX
   ```
