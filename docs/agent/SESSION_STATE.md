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

- Front-Panel & ToF Reset Centralization onto RP2040-Zero (MCP23017 Eliminated):
  - Completely eliminated external MCP23017 IO expander, saving PCB area, wiring, and bus overhead.
  - Front-panel controls wired directly to RP2040-Zero edge pins:
    - `GP6` (CLK / Phase A), `GP7` (DT / Phase B), `GP8` (SW / Click) with 1ms quadrature decoding.
    - `GP14` (Octave +), `GP15` (Octave -), `GP26` (Mute) with 3-sample integrator debouncing.
  - ToF shutdown/wake control wired to `GP2` (Left ToF) and `GP3` (Right ToF), default LOW in bootloader and power-on.
  - Bidirectional I2C protocol formalization on `0x40`:
    - `0x10` (`CMD_READ_INPUTS`): 4-byte atomic `RpeInputReport` returning encoder delta, debounced button state, click events, and USB status.
    - `0x20` (`CMD_SET_XSHUT`): ToF reset mask controlling GP2 and GP3.
    - High-frequency USB-MIDI packets (`CIN 0x08..0x0E`) remain untouched with zero collision and zero wrapping overhead.
  - Re-exported `RotaryEncoder` and `DebouncedButton` in `expressive-core::input`.
  - 68 tests passing across workspace, 0 clippy warnings, zero `.unwrap()` in production code.

  - Custom 3MB Partition Table for CrowPanel 5.0" (4MB Flash):
    - Created `firmwares/expressive-theremin/partitions.csv` allocating 3MB (`0x300000`) to the factory app partition.
    - Updated `.cargo/config.toml` runner to pass `--partition-table partitions.csv`, perfectly fitting the 1.99 MB binary with 1 MB of headroom.
  - Live Hardware Validation: Left ToF (Volume) & RPE OTA I2C Flash:
    - ESP32 automatically updated the RP2040 coprocessor firmware over I2C in ~4 seconds (CRC 0xC7F299AE verified).
    - Sequential ToF boot via RPE: `GP2` set HIGH, Left ToF booted at default `0x29`, readdressed to `0x30` by Core 1 DSP task.
    - I2C bus scan confirms all devices active: `0x18` (DAC/Amp), `0x30` (Left ToF), `0x40` (RP2040 RPE), `0x5D` (GT911 touch).
    - Decoupled sensor acquisition in `main.rs`: individual ToF sensors read live when connected, synthetic simulation only active on disconnected channels.
    - Added data-ready interrupt gating (`has_measurement()`) to ensure optical integration completes.
    - **Real-Time Hand Tracking Validated**: Left ToF continuously outputs `status=Valid` with millimetric precision (66 mm $\rightarrow$ 95 mm $\rightarrow$ 119 mm $\rightarrow$ 168 mm) and real-time logs.
  - **Dual ToF Sensors Live Operation Verified**:
    - Left ToF at `0x30` (Volume antenna) and Right ToF at `0x31` (Pitch antenna) active and ranging simultaneously.
  - **Theremin Expressive Control Logic (Mémorisé & Validé)**:
    - **Main Gauche (MG) - Volume & Timbre Aftertouch:**
      - Bas en haut: CC7 (Volume), 0 en bas ($\le 30$ mm = Silence / Mute hard), 127 au max ($280$ mm = Full Volume).
      - Zone Aftertouch / Timbre au-dessus du volume max ($280$ mm à $480$ mm): CC7 reste à 127 et CC74 (Brightness / Timbre MPE / Filter cutoff) monte de 0 à 127!
      - Triple contrôle: CC7 (Volume), CC11 (Expression), CC74 (Timbre) et ChannelPressure (Aftertouch).
    - **Main Droite (MD) - Pitch & Visual Pre-Tuning:**
      - Bas/proche à 2 cm ($20$ mm): Aigus (base note + `octave_range * 12`).
      - Haut/loin autour de 60 cm ($600$ mm, réglable): Graves (base note).
      - Entre les 2: 4 à 7 octaves réglables (défaut = 4 octaves).
      - **Accordeur Visuel Découplé du Volume (Silent Pre-Tuning)**:
        - Dès que la main droite est au-dessus du ToF pitch (`processor.is_playing()`), la note, l'octave, les cents, la fréquence et l'aiguille du strobe tuner sont **100% actifs et visibles**, même avec volume à 0 ou en mute!
        - Dès que la main droite est retirée (out of range $> 800$ mm ou status non valide), `is_playing` repasse à false, NoteOff est envoyé, et l'accordeur affiche `--` / `-- Hz` avec aiguille masquée.
  - **Validation Matérielle Complète (Physical Hardware Bring-Up Validated)**:
    - **Dual ToF Sensors**: Boot séquentiel via RP2040 `GP2`/`GP3`, adressage dynamique `0x30` / `0x31`, tracking simultané des deux mains.
    - **Pré-accordage Silencieux**: Note, cents, fréquence et aiguille strobe 100% fonctionnels même avec volume à 0.
    - **Contrôles Façade RP2040**:
      - Encodeur rotatif EC11 (`GP6`, `GP7`, `GP8` clic) : défilement fluide et bascule de vue écran validés.
      - Boutons poussoirs (`GP14` Octave +, `GP15` Octave -, `GP26` Mute) : debouncing matériel et actions musicales validés.
    - **Architecture**: Élimination complète de l'expander MCP23017, bus I2C propre, OTA I2C automatique en 4s.
    - **Adversarial Code Review (Grill) Validé**:
      - Résolu : Bug critique NoteOff émis sur canal hardcodé 1 corrigé (routage dynamique selon `processor_mode`).
      - Résolu : Tâche Embassy `front_panel_task` extraite du corps de `main()` et hissée au niveau module.
      - Résolu : Auto-silence de la main gauche (MG) dès le retrait de la main (`consecutive_vol_misses >= 5`) ramenant le volume à 0.
      - Résolu : Détection d'erreurs d'écriture I2C (`write_err`) étendue à tous les messages CC et All Sound Off.
      - Résolu : Documentation alignée sur le câblage physique réel (`GP14`, `GP15`, `GP26`).
    - **Tests & Qualité**: 69 tests unitaires réussis dans tout le workspace (100% de succès), 0 warning, zero `.unwrap()` en production.

## Next action
1. Soumettre la revue Grill détaillée à l'utilisateur et demander l'autorisation de commit (Gate 3).
2. Effectuer les commits par lots logiques (core, theremin, rp2040, esp32, docs).
3. Traitement des calibrations de zones mortes et sensibilité (Volume 20–280 mm, Timbre CC74 280–480 mm, Pitch 20–600 mm).
