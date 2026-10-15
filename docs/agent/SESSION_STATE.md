# Session State

## Status
IN_PROGRESS

## Milestone
V0.6 ToF Theremin Prototype (ESP32-S3 + CrowPanel 5.0" Slint UI)

## Current task
Hardware-in-the-Loop validation of real-time dual-core performance and RP2040-Zero USB-MIDI coprocessor design.

## Last completed
- Configured Open Source Project Governance (commit `5f3663d`):
  - Created official MIT License (`LICENSE`) for permissive open-source distribution.
  - Added developer contribution guide (`CONTRIBUTING.md`) with real-time zero-heap rules and PR workflow.
  - Added GitHub Issue Templates (`.github/ISSUE_TEMPLATE/bug_report.md`, `feature_request.md`, `config.yml`).
  - Added status badges and live Web Flasher links to `README.md`.
- Implemented and deployed Didactic Web Flasher (`tools/web-flasher`) with WebSerial API:
  - Zero-install, browser-based 1-click update writing unified factory image (`0x0000`).
  - Full bilingual i18n support (French/English with browser auto-detection and live header toggle).
  - Crash-proof hardware RTS reboot sequence and interactive step navigation.
  - Published live to GitHub Pages: https://rdespres87.github.io/expressive-control/
- Optimized Slint Graphic Rendering Performance on CrowPanel 5.0" (commit `c557c64`).

## Next action
- Implement Physical Navigation & Focus Management in Slint UI (`master.slint` + `main.rs`):
  - Parameter card focus outline / highlight for keyboard / encoder navigation.
  - Dual-mode rotary encoder control: Navigate mode (cycle parameters) vs Edit mode (adjust value).
  - Hardware button shortcuts (Octave + / -, instant Mute).
  - Reference pitch calibration (440 Hz ± 10 Hz) in Strobe Tuner view.
- Bench validation of dual ToF distance sensors (VL53L4CD) on physical CrowPanel I2C bus.


