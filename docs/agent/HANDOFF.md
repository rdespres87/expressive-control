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

- **Centralization of Front-Panel & ToF Reset onto RP2040-Zero (MCP23017 Eliminated)**:
  - Eliminated external Waveshare MCP23017 expander (`0x20`), reducing BOM, physical footprint, and bus contention.
  - Front-panel human controls and sensor resets wired to RP2040-Zero:
    - `GP2`: Left ToF (Volume) XSHUT enable line (Push-Pull Output, default LOW)
    - `GP3`: Right ToF (Pitch) XSHUT enable line (Push-Pull Output, default LOW)
    - `GP6`: Rotary Encoder CLK (Phase A) (Input with Pull-Up)
    - `GP7`: Rotary Encoder DT (Phase B) (Input with Pull-Up)
    - `GP8`: Rotary Encoder SW (Pushbutton) (Input with Pull-Up)
    - `GP14`: Octave + Button (Input with Pull-Up)
    - `GP15`: Octave - Button (Input with Pull-Up)
    - `GP26`: Mute Button (Input with Pull-Up)
  - Collision-proof bidirectional I2C protocol at `0x40`:
    - `cmd::READ_INPUTS (0x10)`: returns 4-byte atomic `RpeInputReport` (encoder delta, debounced button states, latched click flags, USB/XSHUT status) and resets accumulators.
    - `cmd::SET_XSHUT (0x20)`: 1-byte mask controlling Left (GP2) and Right (GP3) ToF resets.
    - Real-time 4-byte USB-MIDI frames (`CIN 0x08..0x0E`) stream unhampered at maximum bandwidth.
  - Shared `#![no_std]` drivers (`RotaryEncoder`, `DebouncedButton`) moved to `expressive-core::input`.
- **Theremin Expressive Control Logic & Silent Pre-Tuning Architecture**:
  - **Main Gauche (MG) - Volume & Timbre Aftertouch:**
    - Bas en haut: CC7 (Volume), 0 en bas ($\le 30$ mm = Silence / Mute hard), 127 au max ($280$ mm = Full Volume).
    - Zone Aftertouch / Timbre au-dessus du volume max ($280$ mm à $480$ mm): CC7 reste à 127 et CC74 (Brightness / Timbre MPE / Filter cutoff) monte de 0 à 127.
    - Forwarding MIDI complet: CC7 (Volume), CC11 (Expression), CC74 (Timbre) et ChannelPressure (Aftertouch).
  - **Main Droite (MD) - Pitch & Visual Pre-Tuning:**
    - Bas/proche à 2 cm ($20$ mm): Aigus (base note + `octave_range * 12`).
    - Haut/loin autour de 60 cm ($600$ mm, réglable): Graves (base note).
    - Entre les 2: 4 à 7 octaves réglables (défaut = 4 octaves).
    - **Accordeur Visuel Découplé du Volume (Silent Pre-Tuning)**:
      - Dès que la main droite est au-dessus du ToF pitch (`processor.is_playing()`), la note, l'octave, les cents, la fréquence et l'aiguille du strobe tuner sont **100% actifs et visibles**, même avec volume à 0 ou en mute!
      - Dès que la main droite est retirée (out of range $> 800$ mm ou status non valide pendant 100-150 ms), `is_playing` repasse à false, NoteOff est envoyé, et l'accordeur affiche `--` / `-- Hz` avec aiguille masquée.
- All 69 tests passing across workspace, 0 clippy warnings, zero `.unwrap()` in production code.

## Verification
- Custom 3MB partition table (`partitions.csv` + `.cargo/config.toml`) applied with `espflash flash --partition-table partitions.csv`, accommodating the 1.99 MB binary.
- Live hardware boot validated on CrowPanel 5.0" & RP2040-Zero:
  - RP2040 coprocessor automatically updated over I2C in ~4 seconds and booted cleanly (CRC `0xC7F299AE` verified).
  - Left ToF (Volume) woke via `GP2`, booted at default `0x29`, and was readdressed to `0x30` by Core 1 DSP task.
  - Right ToF (Pitch) woke via `GP3`, booted at default `0x29`, and was readdressed to `0x31` by Core 1 DSP task.
  - I2C bus scan verified active peripherals: `0x18` (DAC), `0x30` (Left ToF), `0x31` (Right ToF), `0x40` (RP2040-Zero), `0x5D` (GT911 touch).
  - Slint UI running at 60 FPS with `hw_connected = true`.
  - EC11 rotary encoder (`GP6`, `GP7`, `GP8`) verified for view toggle and navigation.
  - Front-panel buttons (`GP14` Octave +, `GP15` Octave -, `GP26` Mute) verified with debouncing.
  - Visual silent pre-tuning verified on hardware (active note/needle tracking even with volume at 0).
  - Left-hand auto-silence implemented and verified (`consecutive_vol_misses >= 5`).
- Adversarial Code Review (Grill) completed:
  - Fixed NoteOff channel routing bug in `instruments/theremin`.
  - Hoisted `front_panel_task` to module level in `rp2040-usb-midi`.
  - Handled Left ToF auto-silence and error propagation on CC writes.
  - Aligned documentation to actual hardware pins (`GP14`, `GP15`, `GP26`).
- Workspace tests: all 69 unit tests in workspace pass, 0 warnings.

## Next Logical Action
1. Solliciter la validation utilisateur (Gate 3) pour le git commit.
2. Effectuer le commit par lots logiques (core, theremin, rp2040, esp32, docs).
3. Procéder au calibrage fin des zones mortes et sensibilité (Volume 20–280 mm, Timbre CC74 280–480 mm, Pitch 20–600 mm).


