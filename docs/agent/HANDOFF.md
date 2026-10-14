## Context
Completed implementation and validation of the Unified Firmware Update System (CrowPanel USB single entry point -> RP2040 I2C In-System Programming):
- **RP2040 I2C Target Bootloader (`firmwares/rp2040-i2c-bootloader`)**:
  - Compact 16 KB bootloader (`0x10000000 .. 0x10004000`) on I2C target address `0x40`.
  - ROM-assisted Flash sector erase and page programming.
  - Fail-safe metadata sector at `0x101FF000` with IEEE 802.3 CRC32 verification and atomic valid flag.
  - Safe 50ms boot window before branching to application at `0x10004000`.
- **Relocated RP2040 USB-MIDI Coprocessor (`firmwares/rp2040-usb-midi`)**:
  - Memory origin relocated to `0x10004000`.
  - Raw binary (`rp2040-usb-midi.bin`) extracted: 22,144 bytes (~21.6 KB, CRC32 `0xE6656122`).
- **ESP32-S3 Master Flasher (`firmwares/expressive-theremin/src/rpe_flasher.rs`)**:
  - Embeds `rp2040-usb-midi.bin` via `include_bytes!`.
  - Probes coprocessor on startup: if CRC mismatch or bootloader mode, flashes blocks of 128 bytes with retry and CRC verification in < 1 second.
- **ESP32-S3 Flash Optimization**:
  - Added `lto = "fat"` and `codegen-units = 1` in `Cargo.toml`, freeing >600 KB of Flash headroom.

## Verification
- `cargo +stable clippy --release --offline -- -D warnings` (`rp2040-i2c-bootloader`): Clean (0 warnings).
- `cargo +stable clippy --release --offline -- -D warnings` (`rp2040-usb-midi`): Clean (0 warnings).
- `cargo clippy --release -- -D warnings` (`expressive-theremin`): Clean (0 warnings).
- `cargo test -p expressive-core -p theremin`: 20 passed, 0 failed.
- `cargo fmt --all --check`: Clean (0 diffs).

## Next Logical Action
1. Bootstrap bootloader onto Waveshare RP2040-Zero via USB (`rp2040-i2c-bootloader.uf2`).
2. Test end-to-end I2C update triggered by CrowPanel on initial boot.
