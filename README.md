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
- **PC Simulator**: Reads sensor events from `simulator/scenario.yml`, processes them through the Core, and outputs MPE MIDI to a virtual MIDI port named "Expressive MPE Out" (macOS/Linux) that you can connect to your DAW.
- **Firmware (nRF5340)**: 
  - **Standard mode**: Polls an I2C sensor and outputs MIDI over USB.
  - **HIL (Hardware-In-the-Loop) mode**: Receives mocked sensor events from the PC simulator over UART (via Postcard/COBS) and processes them on the real hardware, outputting MIDI over USB.

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
