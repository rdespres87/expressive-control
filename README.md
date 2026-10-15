# Expressive Control

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Web Flasher](https://img.shields.io/badge/Web%20Flasher-Live%20Demo-brightgreen)](https://rdespres87.github.io/expressive-control/)
[![Rust](https://img.shields.io/badge/Rust-2021%20edition-orange.svg)](https://www.rust-lang.org)

Rust project for a modular, real-time expressive music controller and synthesizer framework.

> ⚡ **Try the Web Flasher:** You can flash your CrowPanel ESP32-S3 theremin directly from Google Chrome or Microsoft Edge without installing local toolchains: [**https://rdespres87.github.io/expressive-control/**](https://rdespres87.github.io/expressive-control/)


Architecture:
```
Sensors -> Controls -> Instrument -> Renderer -> MIDI/MPE -> Transport
```

The Core is strictly `#![no_std]`, independent of hardware. Host platforms (PC Simulator, nRF5340, ESP32-S3, RP2040) are isolated into dedicated workspace crates and firmware targets.

---

## Repository Structure

```
├── core/                              # Hardware-agnostic no_std core (MPE voice allocator, MIDI encoder)
├── instruments/
│   └── theremin/                      # no_std ToF Theremin processor (pitch & volume tracking, quantization)
├── simulator/                         # PC CLI Simulator (YAML scenarios, virtual MIDI output)
├── tools/
│   ├── elf2uf2/                       # Pure Rust zero-dependency ELF to UF2 converter for RP2040
│   └── telemetry-viewer/              # egui real-time GUI telemetry visualizer
├── firmwares/
│   ├── nrf5340/                       # Embassy nRF5340 firmware (MPE keyboard & HIL runner)
│   ├── expressive-theremin/           # ESP32-S3 CrowPanel 5.0" firmware (Dual-Core, Slint UI, DSP)
│   ├── rp2040-i2c-bootloader/         # RP2040 I2C Target Bootloader (fail-safe 16 KB In-System Flasher)
│   └── rp2040-usb-midi/               # RP2040-Zero I2C Target to USB-MIDI Class-Compliant coprocessor
└── docs/                              # Architecture specs, studies, and agent session protocols
```

---

## Current Capabilities & Hardware Modules

### 1. Core & Instruments (`no_std`)
- **`expressive-core`**: High-performance, zero-allocation MPE voice allocator, 14-bit pitch bend calculations, CC74/Channel Pressure routing, and raw MIDI serialization.
- **`instruments/theremin`**: Specialized Time-of-Flight theremin DSP engine featuring chromatic pitch quantization, customizable pitch bend range (+/- 1 to 48 semitones), octave shifting, and low-pass filtering.

### 2. ESP32-S3 ToF Theremin (`firmwares/expressive-theremin`)
- **Dual-Core Architecture**:
  - **Core 1 (`APP_CPU`)**: Dedicated to hard real-time sensor acquisition (GT911 touch, MCP23017 expander, dual VL53L4CD ToF sensors) and musical DSP @ 100 Hz.
  - **Core 0 (`PRO_CPU`)**: Dedicated to driving the Elecrow CrowPanel 5.0" RGB LCD (800x480) with Slint UI @ 60 FPS via double-buffered PSRAM DMA.
- **Inter-Core Synchronization**: Lock-free SPSC queues (`heapless`) for UI/touch events; wait-free atomic snapshot (`AtomicTunerSnapshot`) for 60 FPS strobe tuner visualization.
- **Slint Dual-View Interface**:
  - **Master View**: Interactive control for Root note, Musical Scale, Octave, Pitch Bend range, Pitch Correction strength, Filter Speed, Language toggle (`[ FR / EN ]`), and dynamic Crate Version badge (`v0.1.0`).
  - **Tuner View**: Subpixel-rasterized high-accuracy Strobe Tuner gauge with dual-needle frequency tracking.
- **Front Panel I/O (Waveshare MCP23017 @ `0x20`)**:
  - Rotary encoder with detent filtering for UI navigation.
  - Tactile buttons (Octave +, Octave -, Mute).
  - Hardware `XSHUT` control for sequential boot of dual ToF sensors (`0x30` / `0x31`).
  - Resilient hardware fallback: operates cleanly if expander or sensors are absent.

### 3. Auxiliary RP2040-Zero USB-MIDI Coprocessor
- **Why a Coprocessor?** The onboard USB-C port of the CrowPanel 5.0" is hardwired to a CH340G USB-UART bridge (`GPIO 43/44`). To provide true Class-Compliant USB-MIDI without hardware conflict or breaking firmware flashing, a **Waveshare RP2040-Zero** acts as an I2C Target (`0x40`) USB-MIDI bridge.
- **Unified In-System Firmware Update (I2C ISP)**:
  - **`rp2040-i2c-bootloader`**: 16 KB fail-safe bootloader residing at `0x10000000`. Listens on I2C `0x40` during boot for flash commands. Supports ROM sector erase and page programming with IEEE 802.3 CRC32 verification.
  - **`rpe_flasher.rs`**: Built into `expressive-theremin`. At boot, the ESP32-S3 checks the coprocessor version over I2C. If absent or outdated, it automatically reboots the RP2040 into its bootloader and flashes the updated USB-MIDI binary in < 1 second.
  - **`tools/elf2uf2`**: Zero-dependency pure Rust utility generating `rp2040-i2c-bootloader.uf2` for the initial drag-and-drop bootstrap onto the RP2040 USB mass storage drive (`RPI-RP2`).

---

## Quick Start & Commands

### Prerequisites
- Standard Rust toolchain (`rustup default stable`)
- Target `thumbv6m-none-eabi` for RP2040 (`rustup target add thumbv6m-none-eabi`)
- ESP-IDF Rust toolchain (`espup install`) and `espflash` for ESP32-S3

### Testing & Verification
```bash
# Run unit and integration tests across workspace
cargo test -p expressive-core -p theremin -p elf2uf2

# Check formatting across all crates
cargo fmt --all -- --check

# Check workspace lints
cargo clippy --workspace -- -D warnings
```

### PC Simulator
Simulates the expressive engine on macOS/Linux using virtual MIDI ports and YAML scenarios:
```bash
# Run the PC Simulator (reads scenario.yml and streams to "Expressive MPE Out")
cargo run -p simulator
```

### Generating RP2040 UF2 Binaries
To build the RP2040 bootloader and generate the drag-and-drop `.uf2` file:
```bash
cd firmwares/rp2040-i2c-bootloader
cargo build --release
cargo run -p elf2uf2 -- target/thumbv6m-none-eabi/release/rp2040-i2c-bootloader rp2040-i2c-bootloader.uf2
```

### Flashing the ESP32-S3 CrowPanel
```bash
cd firmwares/expressive-theremin

# Build release firmware (embeds RP2040 coprocessor payload automatically)
cargo build --release

# Flash via CrowPanel CH340 USB-C port at 460,800 baud
espflash flash -B 460800 --chip esp32s3 --port /dev/cu.usbserial-XXXX target/xtensa-esp32s3-espidf/release/expressive-theremin

# Or flash and open serial monitor directly
espflash flash -B 460800 --monitor --chip esp32s3 --port /dev/cu.usbserial-XXXX target/xtensa-esp32s3-espidf/release/expressive-theremin
```

---

## Documentation Links

- Architecture Overview: [docs/architecture.md](docs/architecture.md)
- Hardware Netlist & Schematics Study: [docs/studies/hardware_netlist.md](docs/studies/hardware_netlist.md)
- Development Rules: [docs/agent/RULES.md](docs/agent/RULES.md)
- Agent State & Tasks: [docs/agent/SESSION_STATE.md](docs/agent/SESSION_STATE.md) & [docs/agent/TASKS.md](docs/agent/TASKS.md)

---

## Contributing

Contributions, issues, and feature requests are welcome!  
Please see [CONTRIBUTING.md](CONTRIBUTING.md) for architectural guidelines, real-time constraints, and development workflow.

---

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

