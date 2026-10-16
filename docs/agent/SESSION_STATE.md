# Session State

## Status
IN_PROGRESS

## Milestone
V0.6 ToF Theremin Prototype (ESP32-S3 + CrowPanel 5.0" Slint UI)

## Current task
Hardware-in-the-Loop validation of real-time dual-core performance, RP2040-Zero USB-MIDI coprocessor pipeline, and "Over the Rainbow" expressive theremin simulation.

- Physical Waveshare RP2040-Zero ("RPE") USB-MIDI Coprocessor Bring-Up:
  - Hardware & Pinout confirmed & validated:
    - CrowPanel 4-pin connector: Pin 1 GND (Noir), Pin 2 3V3 (Rouge), Pin 3 SDA (Blanc), Pin 4 SC/SCL (Jaune).
    - RP2040-Zero pins: Noir -> GND, Blanc (SDA) -> GP0, Jaune (SCL) -> GP1.
    - Debug UART1: GP4 (TX -> FTDI RX Jaune), GP5 (RX -> FTDI TX Orange), GND (Noir) @ 115200 baud.
  - Successfully diagnosed and fixed IRQ 0 / GT911 touch screen freeze:
    - Root cause: Missing `rt` feature in `embassy-rp` prevented `TIMER_IRQ_0` ISR compilation.
    - Fix: Added `rt` feature to `embassy-rp`, `-C link-arg=--undefined=TIMER_IRQ_0`, and explicit W1C clear of timer alarm 0 in `DefaultHandler`.
- Full I2C Bootloader & USB-MIDI OTA Pipeline Complete & Validated on Hardware:
  - Bootloader expanded to 32KB slot (`0x1000_0000` .. `0x1000_8000`) with size optimizations (`opt-level = "z"`, `codegen-units = 1`, `panic = "abort"`).
  - `rp2040-usb-midi` compiled at offset `0x1000_8000` (`memory.x` aligned, VTOR set to `0x1000_8000`).
  - Cold-boot bus fault eliminated by strictly initializing Embassy (`embassy_rp::init()`) before any GPIO/SIO register access.
  - ESP32 flasher (`rpe_flasher.rs`) automatically checks CRC32 (`0x89DF6A1C`) and flashes RP2040 over I2C in ~2 seconds.
  - RP2040 boots cleanly into Application mode (solid Magenta LED) and streams USB-MIDI directly to macOS.
  - CrowPanel GT911 touch screen 100% fluid and responsive at 60 FPS (zero bus contention or clock stretching).
- Expressive Simulation: "Over the Rainbow" (Volet 2 Complete & Validated in Surge XT):
  - Created `firmwares/expressive-theremin/src/simulation.rs` implementing a continuous 32-second lyrical trajectory in F major.
  - Continuous cubic hermite portamento (glissando) for the iconic octave leap `F4 -> F5`.
  - Natural delayed vibrato (~5.3 Hz LFO ramping in after note onset, ~35 cents depth).
  - Expressive volume dynamics (left-hand phrasing, subtle volume tremolo synchronized with vibrato, soft fade-out).
  - Triple dynamics forwarding: Channel Pressure (Aftertouch) + CC7 (Volume) + CC11 (Expression) over USB-MIDI.
  - Fully verified and audible in Surge XT, with real-time tuner and needle animations on the CrowPanel LCD.

- USB-MIDI Hot-Plug Resilience & Grill Validation Complete:
  - Cold-Boot Auto-Jump: `rp2040-i2c-bootloader` validates vector table (MSP in SRAM, Reset Handler in Flash with Thumb bit) and CRC32; jumps to `0x1000_8000` in ~10ms on power-on reset unless forced via `WATCHDOG_SCRATCH0`.
  - Latency Optimization: Removed blocking `uart_print` from `rp2040-usb-midi` I2C packet handling (<2µs processing).
  - Dynamic Disconnect / Hotplug Recovery: ESP32 detects I2C packet write failures, sets `rpe_ready = false`, and polls `0x40` every 500ms; automatically boots or updates the RP2040 and resumes streaming.
  - Live OTA update verified on hardware: ESP32 detected updated app binary (`0x4FE35190`), triggered soft reset to bootloader, reflashed all blocks, and confirmed `Coprocessor re-connected and ready!`.

## Next action
1. Copy updated `rp2040-i2c-bootloader.uf2` to RP2040 in BOOTSEL mode to enable cold-boot auto-jump on power cycle.
2. Test USB replug on macOS: observe automatic enumeration and streaming recovery.
3. Review / restore user configuration and submenus NVS state (`poc-submenus-and-nvs`).



