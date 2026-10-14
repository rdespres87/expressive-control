## Context
Completed implementation, adversarial Inquisitor Grill, CI verification, and commit of the Pure Rust `tools/elf2uf2` generator and the RP2040 I2C Bootloader UF2 binary:
- **ELF to UF2 Generator (`tools/elf2uf2`)**:
  - Pure Rust workspace binary crate without external dependencies.
  - Generates 512-byte UF2 blocks with RP2040 Bootrom compatibility (magics, Family ID `0xE484FF45`, 256-byte payload alignment, LMA/VMA resolution).
  - 100% test coverage (6 unit tests passing).
- **RP2040 I2C Target Bootloader (`firmwares/rp2040-i2c-bootloader`)**:
  - Linker configured with `-Tlink-rp.x` to guarantee `.boot2` ROM stage mapping at `0x10000000`.
  - UF2 binary generated at `firmwares/rp2040-i2c-bootloader/rp2040-i2c-bootloader.uf2` (58 blocks, 29,696 bytes).
- **All previous components remaining verified**:
  - Relocated RP2040 USB-MIDI coprocessor (`firmwares/rp2040-usb-midi`) at `0x10004000`.
  - ESP32-S3 Master Flasher (`rpe_flasher.rs`).
  - Slint UI configuration screen with dynamic `v0.1.0` badge.

## Verification
- `cargo test -p elf2uf2`: 6 passed, 0 failed.
- `cargo clippy -p elf2uf2 -- -D warnings`: Clean (0 warnings).
- `cargo +stable clippy --release --offline -- -D warnings` (`rp2040-i2c-bootloader`): Clean (0 warnings).
- `cargo test -p expressive-core -p theremin`: 20 passed, 0 failed.
- `cargo fmt --all --check`: Clean (0 diffs).

## Next Logical Action
1. Flash `firmwares/rp2040-i2c-bootloader/rp2040-i2c-bootloader.uf2` onto the Waveshare RP2040-Zero (drag-and-drop onto the `RPI-RP2` USB mass storage drive).
2. Wire RP2040-Zero I2C lines to CrowPanel 5.0" (`GPIO 19` SDA, `GPIO 20` SCL) and observe the ESP32-S3 automatically sync and flash the USB-MIDI application over I2C in < 1 second.
