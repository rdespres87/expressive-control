# Session State

## Status
IN_PROGRESS

## Milestone
V0.6 ToF Theremin Prototype (ESP32-S3 + CrowPanel 5.0" Slint UI)

## Current task
Hardware-in-the-Loop validation of real-time dual-core performance and RP2040-Zero USB-MIDI coprocessor design.

## Last completed
- Decoupled ESP32-S3 Dual-Core execution for real-time performance:
  - **Core 1 (APP_CPU)**: Real-time 100 Hz (10ms) sensor polling & DSP loop (`core1_sensor_dsp_task`):
    - Completely isolated from LCD VSYNC and DMA rendering.
    - Probes I2C devices (GT911 touch at `0x5D`, MCP23017 expander at `0x20`, dual VL53L4CD ToF).
    - Captures touch pointer events and front panel controls (octave +/-, mute, rotary encoder) and forwards them lock-free via `heapless::spsc::Queue<UiInputEvent, 32>`.
    - Computes musical pitch/expression DSP via `TofThereminProcessor`.
    - Publishes wait-free state updates to `AtomicTunerSnapshot` (SeqLock).
  - **Core 0 (PRO_CPU)**: Pure 60 FPS Slint UI and LCD RGB DMA flush (`run_core0_ui_render_loop`):
    - Completely decoupled from I2C bus transactions.
    - Drains `ui_input_rx` to dispatch Slint pointer events and update UI controls.
    - Reads latest musical snapshot wait-free for strobe gauge overlay and Slint properties.
- Verified on hardware (ESP32-S3 CrowPanel 5.0" on `/dev/cu.usbserial-1420`):
  - GT911 touch detected and confirmed at 0x5D (Product ID: '911').
  - Live touch view toggles (Master <-> Tuner) and language toggles (FR <-> EN) confirmed over lock-free queue.
  - Zero compiler warnings (`cargo clippy --release -- -D warnings`), code formatted (`cargo fmt`), all tests passing.
- Fixed Tuner button vertical text alignment in `firmwares/expressive-theremin/ui/master.slint` (`c338bed`).

## Next action
- Commit dual-core real-time optimization changes (Gate 3 approval).
- Scaffold and implement RP2040-Zero USB-MIDI coprocessor firmware (I2C Target @ 0x40 -> USB-MIDI Class Compliant).

