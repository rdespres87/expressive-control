## Context
Completed implementation, adversarial Inquisitor Grill, and bench flash of the Unified Firmware Update System & UI Versioning:
- **RP2040 I2C Target Bootloader (`firmwares/rp2040-i2c-bootloader`)**:
  - Compact 16 KB bootloader (`0x10000000 .. 0x10004000`) on I2C target address `0x40`.
  - ROM-assisted Flash sector erase (800ms) and 256-byte page programming.
  - Fail-safe metadata sector at `0x101FF000` with IEEE 802.3 CRC32 verification and atomic valid flag.
  - Active 100ms boot window listening on I2C with `select`.
  - Watchdog scratch reboot mechanism (`0xB007_10AD`) and NVIC cleanup before application jump.
- **Relocated RP2040 USB-MIDI Coprocessor (`firmwares/rp2040-usb-midi`)**:
  - Memory origin relocated to `0x10004000`.
  - Added I2C commands `0x01` (status query) and `0xFB` (reboot to bootloader).
- **ESP32-S3 Master Flasher (`firmwares/expressive-theremin/src/rpe_flasher.rs`)**:
  - Automated compilation and raw binary embedding in `build.rs`.
  - Auto-reboots coprocessor into bootloader if application is outdated.
  - Streams 256-byte chunks with retries and CRC32 verification.
- **UI Version Badge (`v0.1.0`)**:
  - Compacted Pitch Correction panel to 364px to create dedicated space in bottom-right corner.
  - Displayed sleek 84px VERSION badge bound to `env!("CARGO_PKG_VERSION")`.
  - Zero unwrap rule strictly respected across all production crates.

## Verification
- `cargo +stable clippy --release --offline -- -D warnings` (`rp2040-i2c-bootloader`): Clean (0 warnings).
- `cargo +stable clippy --release --offline -- -D warnings` (`rp2040-usb-midi`): Clean (0 warnings).
- `cargo clippy --release -- -D warnings` (`expressive-theremin`): Clean (0 warnings).
- `cargo test -p expressive-core -p theremin`: 20 passed, 0 failed.
- `cargo fmt --all --check`: Clean (0 diffs).
- Physical flash onto CrowPanel 5.0" via `/dev/cu.usbserial-1420`: Verified nominal, zero panic, UI running with version badge.

## Next Logical Action
1. Bootstrap bootloader onto Waveshare RP2040-Zero via USB (`rp2040-i2c-bootloader.uf2`).
2. Connect RP2040-Zero I2C lines to CrowPanel (`GPIO 19/20`) and observe autonomous I2C handshake and firmware synchronization (< 1 sec).
