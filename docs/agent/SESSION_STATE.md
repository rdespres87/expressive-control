# Session State

## Status
IN_PROGRESS

## Milestone
V0.6 ToF Theremin Prototype (ESP32-S3 + CrowPanel 5.0" Slint UI)

## Current task
Hardware-in-the-Loop validation of real-time dual-core performance and RP2040-Zero USB-MIDI coprocessor design.

- Implemented Physical Navigation & Focus Management in Slint UI & Firmware:
  - Added `NavigationController` state machine (`firmwares/expressive-theremin/src/navigation.rs`) managing `Navigating` vs `Editing` modes.
  - Rotary encoder rotation cycles cards in Navigate mode and adjusts parameters in Edit mode (Root Note, Scale, Octave, Octave Range, MIDI Mode, Pitch Bend Range, Pitch Quantization, Filter Speed, Tuner Switch).
  - Rotary encoder click toggles between Navigate and Edit modes (or toggles between Master and Tuner views).
  - Dedicated hardware buttons provide instant Octave Up / Down shifting and instant Mute toggle.
  - Slint UI (`master.slint`, `theremin.slint`, `index.slint`) displays dynamic focus highlights (Gold for Navigating, Cyan for Editing), centered `[ MUTED ]` badge, and A4 reference pitch calibration (440 Hz ± 10 Hz).
  - Full real-time decoupling maintained: Core 1 polls MCP23017 front-panel events and dispatches via lock-free queue to Core 0 Slint UI.
- Documented User Configuration & Flash Persistence Architecture ([docs/studies/user_configuration_and_menu_architecture.md](docs/studies/user_configuration_and_menu_architecture.md)):
  - Analyzed ESP32-S3 Flash partition table (`partitions.csv`) and NVS isolation (`0x9000`) vs firmware updates (`0x10000`).
  - Formalized non-destructive `UserPreset` data structure (schema versioning, CRC32, 8 slots + active state).
  - Designed Slint UI hierarchical menu tree and double navigation model (touch + physical rotary encoder).
- Configured Open Source Project Governance (commit `33a523e`):
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
- Bench validation of dual ToF distance sensors (VL53L4CD) on physical CrowPanel I2C bus.
- Implement configuration submenus & preset NVS flash persistence based on `docs/studies/user_configuration_and_menu_architecture.md`.


