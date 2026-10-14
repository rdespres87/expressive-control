# Session Handoff

## Context
Completed dual-core real-time decoupling, sensor loop optimization, and hardened input pipeline:
- **Core 1 (APP_CPU) — 100 Hz Real-Time Sensor & DSP Engine (`core1_sensor_dsp_task`)**:
  - Isolated from LCD VSYNC and DMA rendering.
  - Probes and drives I2C bus: GT911 touch screen (`0x5D`), MCP23017 expander (`0x20`), and dual VL53L4CD ToF sensors (`0x30` / `0x31`) with graceful synthetic fallback.
  - Captures touch events and physical controls (`Octave +/-`, `Mute`, `RotaryEncoder`), forwarding them lock-free via `heapless::spsc::Queue<UiInputEvent, 32>`.
  - Protected against drag saturation (`PointerMoved` enqueued only when `ready()`).
  - Guaranteed `PointerReleased` delivery via bounded retry loop to prevent stuck touches.
  - Corrected mute volume mapping to 1000 mm (silent boundary), triggering clean `NoteOff`.
  - Publishes wait-free state updates to `AtomicTunerSnapshot` (SeqLock).
- **Core 0 (PRO_CPU) — Pure 60 FPS Slint UI & DMA Renderer (`run_core0_ui_render_loop`)**:
  - Drains `ui_input_rx` and dispatches Slint window events without touching I2C.
  - Reads wait-free snapshot and renders Strobe Tuner needle & subpixel strobe gauge overlay.
  - Forwards configuration messages to Core 1 via `heapless::spsc::Queue<ConfigMessage, 16>`.
- **Adversarial Grill Review & Expanded Unit Test Suite**:
  - Added 4 unit test suites in `instruments/theremin/src/lib.rs` (15 tests total across workspace):
    - `test_mute_behavior_sends_note_off`
    - `test_sensor_event_released_volume_and_pitch`
    - `test_configuration_updates`
    - `test_buffer_too_small_returns_error`

## Verification
- `cargo fmt --check`: Clean (0 diffs).
- `cargo clippy --release -- -D warnings`: Clean (0 warnings).
- `cargo test -p expressive-core -p theremin`: 15 passed, 0 failed.
- Physical Hardware Validation (CrowPanel 5.0" on `/dev/cu.usbserial-1420`):
  - GT911 touch confirmed at `0x5D` (Product ID: '911').
  - Live touch view toggles (Master <-> Tuner) and language toggles (FR <-> EN) confirmed in serial logs.
- Committed as `843b6b9`.

## Next Logical Action
1. Implement the RP2040-Zero firmware (I2C Target @ `0x40` -> USB-MIDI Class-Compliant endpoint).
2. Wire I2C transmission of 3-byte MIDI packets from ESP32-S3 Core 1 to RP2040-Zero.
