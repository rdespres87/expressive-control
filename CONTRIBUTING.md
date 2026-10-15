# Contributing to Expressive Control

Thank you for your interest in contributing to the **Expressive Control** project! We are building an open-source, ultra-low latency ecosystem for expressive digital musical instruments (MPE, MIDI 2.0, ToF optical theremins, and continuous controllers).

---

## Code of Conduct

We are committed to providing a welcoming, inclusive, and harassment-free environment for everyone. Please be respectful, constructive, and collaborative in all communications.

---

## Architectural Guidelines & Constraints

Before writing code, please review the repository architecture:

- **`core/` (`expressive-core`)**: Pure `no_std`, zero-allocation core foundation traits (`ControlProcessor`, `MidiTransport`, `SensorEvent`). No hardware or OS coupling.
- **`instruments/`**: DSP engines and musical mappings (e.g., `instruments/theremin`).
- **`firmwares/`**: Target-specific firmwares:
  - `expressive-theremin`: ESP32-S3 firmware driving the Elecrow CrowPanel 5.0" RGB LCD with Slint UI, dual-core FreeRTOS task isolation, and I2C co-processor flasher.
  - `rp2040-usb-midi`: High-speed USB-MIDI coprocessor firmware running on RP2040-Zero.
  - `rp2040-i2c-bootloader`: Minimal ROM flash bootloader for RP2040.
  - `nrf5340-app-s`: Embassy-based dual-core BLE/USB firmware.
- **`simulator/`**: Multi-instrument desktop CLI simulator for testing YAML scenarios and telemetry dumps.
- **`tools/`**: Development utilities (e.g. `web-flasher`, `elf2uf2`, `telemetry-viewer`).

### Critical Real-Time Guarantees

1. **Zero Heap Allocations in Audio/DSP Loops**: High-frequency loops (100 Hz sensor polling, 60 FPS UI loops) must never allocate on the heap. Use stack buffers, `heapless` collections, or precomputed lookup tables.
2. **Dual-Core Decoupling**: On multi-core targets (ESP32-S3), Core 1 handles real-time DSP/sensors while Core 0 handles UI rendering. Communication between cores must remain wait-free (SeqLock) or lock-free (SPSC queues).
3. **Language & Conventions**: All code, comments, documentation, and commit messages must be in English.

---

## Development Workflow

### 1. Discuss Before Building
For significant features, architectural modifications, or new instrument additions:
- Please [open an Issue](https://github.com/rdespres87/expressive-control/issues) first to discuss the approach with the maintainers.
- For small bug fixes, typo corrections, or documentation updates, feel free to submit a Pull Request directly.

### 2. Branching & Commit Messages
- Branch from `main` with a descriptive name (e.g. `feat/rotary-navigation`, `fix/vsync-timing`).
- We use [Conventional Commits](https://www.conventionalcommits.org/):
  - `feat(scope): add new feature`
  - `fix(scope): fix bug`
  - `docs(scope): update documentation`
  - `perf(scope): performance optimization`
  - `refactor(scope): code refactoring`

### 3. Pre-Commit Quality Checks
Before submitting a PR, ensure that the workspace formatters and linters pass cleanly:

```bash
# Check code formatting
cargo fmt --all -- --check

# Check workspace linter
cargo clippy --workspace -- -D warnings

# Run all host unit tests
cargo test --workspace --exclude expressive-theremin --exclude rp2040-usb-midi --exclude rp2040-i2c-bootloader
```

---

## Submitting a Pull Request

1. Fork the repository and create your feature branch.
2. Commit your changes with clear messages.
3. Push to your fork and submit a Pull Request against `main`.
4. Provide a clear description in your PR outlining:
   - What changed and why.
   - Any hardware or simulator tests conducted.
   - Verification output from `cargo test` and `cargo clippy`.

Thank you for helping craft open, expressive instruments! 🎶
