# Expressive Theremin — Didactic Web Flasher

A zero-install, browser-based firmware update utility for the **Expressive Theremin** (Elecrow CrowPanel 5.0" ESP32-S3 + RP2040 coprocessor).

Built with the standard **WebSerial API** and Espressif's ROM bootloader protocol (`esptool-js`).

---

## Features

- **Zero Install**: Runs entirely inside modern Chromium-based browsers (Chrome, Edge, Brave, Opera) with WebSerial support.
- **Single-Click Unified Flash**: Writes a unified factory image at address `0x0000` containing:
  - Second-stage Bootloader (`0x0000`)
  - Partition Table (`0x8000`)
  - Application firmware with embedded Slint UI & RP2040 bootloader (`0x10000`)
- **Safety Checks**: Automatically queries chip model and aborts if anything other than an `ESP32-S3` is detected.
- **High Speed**: Flashes at 460,800 baud with real-time transfer speed and ETA display (~25 seconds).
- **Embedded Release or Custom Binary**: Includes official `v0.1.0` release firmware, with an advanced drop-zone for custom development builds.
- **Auto-Sync Coprocessor**: Upon boot, the ESP32-S3 automatically inspects, programs, and validates the RP2040-Zero USB-MIDI coprocessor via internal I2C.

---

## How to Run Locally

Because WebSerial requires a secure context (`https://` or `http://localhost`), launch the dedicated zero-cache local server:

```bash
cd tools/web-flasher
python3 server.py
```

Then open [http://localhost:8888](http://localhost:8888) in Google Chrome, Microsoft Edge, Brave, or Opera.

---

## Updating the Embedded Factory Image

To update the default binary `tools/web-flasher/firmware/v0.1.0/theremin-factory.bin` after building firmware:

```bash
cd firmwares/expressive-theremin
cargo build --release
espflash save-image --chip esp32s3 target/xtensa-esp32s3-espidf/release/expressive-theremin \
    --merge --skip-padding ../../tools/web-flasher/firmware/v0.1.0/theremin-factory.bin
```
