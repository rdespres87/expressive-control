# Handoff

The architecture for the ToF Theremin logic has been completed.
We successfully extracted the instrument-specific logic from `expressive-core` into an intermediate abstraction layer (`instruments/theremin`).

**Next Agent Action:**
You must proceed to initialize the new Cargo workspace member `firmwares/expressive-theremin` (`esp-idf` based).

**Context:**
The ESP32-S3 uses `esp-idf-hal` because it requires the `std` library for complex Slint UI rendering later on. Make sure to generate the project using `cargo generate esp-rs/esp-idf-template cargo` or equivalent, and add it to the root `Cargo.toml` workspace.
