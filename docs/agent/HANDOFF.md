## Context
Completed implementation and verification of Option 1: RP2040-Zero USB-MIDI coprocessor and ESP32-S3 Core 1 streaming bridge:
- **`expressive-core` (`core/src/midi.rs`)**:
  - Implemented Table 4-1 compliant `UsbMidiPacket([u8; 4])` and `ToUsbMidi` trait with CIN categorization (NoteOn, NoteOff, PitchBend, CC74 Timbre, ChannelPressure).
  - Added 5 unit tests (12 tests total passing).
- **RP2040-Zero Firmware (`firmwares/rp2040-usb-midi`)**:
  - Independent ARM Cortex-M0+ crate (`thumbv6m-none-eabi`) using Embassy async runtime.
  - I2C0 target on GPIO 4 (SDA) / GPIO 5 (SCL) at address `0x40`.
  - Class-compliant USB-MIDI 1.0 bulk IN endpoint.
  - Short-packet batching (max 60 bytes < 64 byte limit) guaranteeing immediate host dispatch without ZLP delays.
- **ESP32-S3 Core 1 Integration (`firmwares/expressive-theremin/src/main.rs`)**:
  - Non-blocking I2C write to `0x40` (5ms timeout) ensuring zero stutter on DSP or UI when RP2040 is unplugged.
  - Corrected `hw_connected` detection: excluded onboard GT911 touchscreen so the red warning banner (`HARDWARE DISCONNECTED`) correctly alerts when expander/sensors are absent.
- **Gate 3 Adversarial Grill**:
  - Completed and documented in `rp2040_usb_midi_implementation_grill.md`.

## Verification
- `cargo fmt --all --check`: Clean (0 diffs).
- `cargo +stable clippy --release --offline -- -D warnings` (`rp2040-usb-midi`): Clean (0 warnings).
- `cargo clippy --release -- -D warnings` (`expressive-theremin`): Clean (0 warnings).
- `cargo test -p expressive-core -p theremin`: 20 passed, 0 failed.
- Committed as `86a5bda`.

## Next Logical Action
1. Flash firmware onto Waveshare RP2040-Zero via SWD/picotool (`target/thumbv6m-none-eabi/release/rp2040-usb-midi`).
2. Test USB enumeration as "Expressive Theremin MIDI" on host DAW (CoreMIDI / ALSA / WASAPI).
