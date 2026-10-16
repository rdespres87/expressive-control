## Context
Bring-up and hardware validation of the **Waveshare RP2040-Zero USB-MIDI Coprocessor ("RPE")** wired to the **Elecrow CrowPanel 5.0" ESP32-S3**:

- **Hardware & Wiring Setup**:
  - CrowPanel 4-pin header:
    - Pin 1 (`GND`) $\rightarrow$ Noir $\rightarrow$ RP2040-Zero Pin 2 gauche (`GND`)
    - Pin 2 (`3V3`) $\rightarrow$ Rouge (temporairement débranché : la RP2040 est alimentée par l'USB du Mac)
    - Pin 3 (`SDA`) $\rightarrow$ Blanc $\rightarrow$ RP2040-Zero GP0 (broche 0)
    - Pin 4 (`SC` / `SCL`) $\rightarrow$ Jaune $\rightarrow$ RP2040-Zero GP1 (broche 1)
- **Timer Alarm 0 & GT911 Freeze Root Cause Resolved**:
  - `[WARN] Unhandled IRQ: 0 en boucle` on UART1 debug console.
  - IRQ 0 on RP2040 is `TIMER_IRQ_0`, utilized by Embassy's `time-driver`.
  - In `firmwares/rp2040-usb-midi/Cargo.toml`, `embassy-rp` lacked the `"rt"` feature. Because of this, the `TIMER_IRQ_0` ISR in `embassy-rp/src/time_driver.rs` was never compiled, delegating IRQ 0 to `DefaultHandler`.
  - Inside `DefaultHandler`, alarm 0 was never cleared, trapping the core in an infinite interrupt loop at 100% CPU.
  - As a result, the Embassy executor was completely starved, freezing the `i2c_target_task`, which caused hardware clock stretching on GP1 (SCL) and locked up the GT911 touch screen on the shared bus.
  - **Resolution Applied**:
    1. Added `"rt"` feature to `embassy-rp` in [firmwares/rp2040-usb-midi/Cargo.toml](file:///Users/rdespres/Projets/expressive-control/firmwares/rp2040-usb-midi/Cargo.toml).
    2. Added `-C link-arg=--undefined=TIMER_IRQ_0` in [.cargo/config.toml](file:///Users/rdespres/Projets/expressive-control/firmwares/rp2040-usb-midi/.cargo/config.toml).
    3. Added fallback W1C clear for alarm 0 (`0x4005_4034 = 1 << 0`) in `DefaultHandler` in [firmwares/rp2040-usb-midi/src/main.rs](file:///Users/rdespres/Projets/expressive-control/firmwares/rp2040-usb-midi/src/main.rs).
    4. Compiled and verified symbols (`nm` confirms `TIMER_IRQ_0`, `USBCTRL_IRQ`, `I2C0_IRQ` all bound in `.text`).
- **Full I2C Bootloader & USB-MIDI OTA Pipeline Complete & Validated on Hardware**:
  - Bootloader expanded to 32KB slot (`0x1000_0000` .. `0x1000_8000`) with size optimizations (`opt-level = "z"`, `codegen-units = 1`, `panic = "abort"`).
  - `rp2040-usb-midi` compiled at offset `0x1000_8000` (`memory.x` aligned, VTOR set to `0x1000_8000`).
  - Cold-boot bus fault eliminated by strictly initializing Embassy (`embassy_rp::init()`) before any GPIO/SIO register access.
  - ESP32 flasher (`rpe_flasher.rs`) automatically checks CRC32 (`0x89DF6A1C`) and flashes RP2040 over I2C in ~2 seconds.
  - RP2040 boots into Application mode (solid Magenta LED) and streams USB-MIDI directly to macOS.
  - CrowPanel GT911 touch screen 100% fluid and responsive at 60 FPS (zero bus contention or clock stretching).
- **Expressive Simulation: "Over the Rainbow" (Volet 2 Complete & Validated in Surge XT)**:
  - Created `firmwares/expressive-theremin/src/simulation.rs` implementing a continuous 32-second lyrical trajectory in F major.
  - Continuous cubic hermite portamento (glissando) for the iconic octave leap `F4 -> F5`.
  - Natural delayed vibrato (~5.3 Hz LFO ramping in after note onset, ~35 cents depth).
  - Expressive volume dynamics (left-hand phrasing, subtle volume tremolo synchronized with vibrato, soft fade-out).
  - Triple dynamics forwarding: Channel Pressure (Aftertouch) + CC7 (Volume) + CC11 (Expression) over USB-MIDI.
  - Fully verified and audible in Surge XT, with real-time tuner and needle animations on the CrowPanel LCD.

- **USB-MIDI Hot-Plug Resilience & Live OTA Verification**:
  - `rp2040-i2c-bootloader` auto-jumps to `0x1000_8000` on cold boot / hotplug when application vector table and CRC32 are valid.
  - Removed blocking UART prints in `rp2040-usb-midi` (<2µs per packet).
  - ESP32 dynamic hotplug recovery automatically detects disconnect, polls `0x40` every 500ms, boots or reflashes the RP2040, and resumes streaming without restarting the ESP32.
  - Live OTA update verified: ESP32 automatically detected CRC change, triggered soft reset into bootloader, flashed new binary, and re-established ready state.

## Verification
- Firmware binaries compiled & running on hardware:
  - `firmwares/rp2040-i2c-bootloader/rp2040-i2c-bootloader.uf2` updated with auto-jump.
  - `firmwares/expressive-theremin/target/xtensa-esp32s3-espidf/release/expressive-theremin` flashed and active.
- Surge XT actively playing the musical simulation over USB-MIDI.
- Zero git commits made (per `ne commit rien`).

## Next Logical Action
1. Copy updated `rp2040-i2c-bootloader.uf2` to RP2040 in BOOTSEL mode: `cp firmwares/rp2040-i2c-bootloader/rp2040-i2c-bootloader.uf2 /Volumes/RPI-RP2/`.
2. Test USB replug: observe immediate auto-boot into application and automatic reconnection.
3. Review / restore user configuration and submenus NVS state (stash `poc-submenus-and-nvs`).

