# Session Handoff

## Context
The previous session successfully set up the `nrf5340` firmware architecture. 
- The crate is `no_std`, `no_main`.
- Embassy is configured for the Cortex-M33 TrustZone Secure state (`nrf5340-app-s`).
- The git tree is clean, and the CI is fully passing (the firmware is excluded from the main workspace to prevent host/target compilation clashes).
- All `target/` artifacts have been strictly removed from `.gitignore` and git history.

## Immediate Next Steps
The goal of this new session is to implement the **USB MIDI Class Compliant** transport.

1. Read `firmwares/nrf5340/src/main.rs`.
2. Use the `embassy-usb` crate to configure the USB device descriptors (Vendor ID, Product ID, MIDI streaming interfaces).
3. Implement the `MidiTransport` trait (from `expressive-core`) using the Embassy USB Endpoint buffers.
4. If the implementation is architecturally complex, spawn the **Inquisitor** subagent to review the USB memory layout and `no_std` constraints before committing.

## Agent Instructions
- Read `AGENTS.md` and `SESSION_STATE.md` to load the full context.
- Maintain the strict "Conventional Commits" and english-only code rules.
- Proceed with the implementation.
