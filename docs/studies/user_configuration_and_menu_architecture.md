# Architectural Study: Configuration Submenus and User Preset Flash Persistence

This document analyzes and specifies the architecture for configuration submenus and user presets/profiles management on the expressive instrument (CrowPanel ESP32-S3), with non-regression guarantees and persistent storage across firmware updates.

---

## 1. Problem Statement & Objectives

1. **Musician Experience (UX & Ergonomics)**:
   - Allow instant switching between musical presets (e.g. Preset 1: Equal-tempered C-Minor scale, 24-semitone pitch bend range, smooth ToF filtering for expressive legato; Preset 2: Microtonal Arabic Rast scale, 12-semitone bend, ultra-fast response for crisp staccato).
   - Provide an intuitive hierarchy of configuration submenus to configure DSP parameters, physical controls, MIDI/MPE outputs, and sensor calibration.
   - Support dual input ergonomics: touch interaction (5.0" LCD touchscreen) and physical controls (quadrature rotary encoder with push button, plus dedicated front-panel buttons).

2. **Data Integrity & Firmware Updates**:
   - **Critical Requirement**: Installing a new firmware release (via the Web Flasher WebSerial utility or local `espflash`) must **never** wipe or corrupt the musician's saved user presets.
   - **Schema Migration**: When future firmware versions introduce new parameters, the deserializer must maintain backward compatibility, safely applying defaults without invalidating existing preset slots.

---

## 2. Flash Memory Strategy: How to Isolate User Configurations?

### 2.1 Actual Partitioning Layout in the Project

Binary inspection of the project's compiled partition table (`partition-table.bin` generated under ESP-IDF 5.2) confirms the following exact physical flash map:

```text
Partition Name   Type      Flash Offset            Flash Size
----------------------------------------------------------------------------------
nvs              0x01      0x00009000 (36 KB)      0x00006000  👉 24 KB (24,576 bytes)
phy_init         0x01      0x0000F000 (60 KB)      0x00001000  👉 4 KB
factory (app)    0x00      0x00010000 (64 KB)      0x00100000  👉 1 MB (1,024 KB)
```

- **The NVS partition already physically exists** at offset `0x9000` with a size of **24 KB**.
- It is physically positioned **before** the main application partition `factory` (`0x10000`).

---

### 2.2 Comparison of Current Flashing Tools Behavior

| Flashing Tool | Current Behavior Regarding NVS | Outcome for User Presets |
| :--- | :--- | :--- |
| **`espflash`** (Local Developer CLI) | **NON-DESTRUCTIVE**: Extracts ELF segments and flashes 3 separate blocks (`bootloader` @ 0x0000, `partition-table` @ 0x8000, `app` @ 0x10000). It **skips** the `0x9000..0xF000` region. | ✅ **Preserved**: NVS is never erased during an `espflash flash` invocation. |
| **`tools/web-flasher`** (v0.1.0 current) | **DESTRUCTIVE**: Flashes a monolithic 3.5 MB factory image (`theremin-factory.bin`) written as a single contiguous payload starting at `0x0000`. The range `0x9000..0xF000` contains padding bytes (`0xFF`). | ⚠️ **Overwritten**: The WebSerial flasher driver erases sectors from 0x0000 up to 0x380000, resetting the NVS partition to raw `0xFF`. |

---

### 2.3 Web Flasher Evolution for Presets Preservation

To make the Web Flasher completely non-destructive during future firmware updates:
1. **Targeted Segmented Flashing (`fileArray`)**:
   In `tools/web-flasher/app.js`, replace the monolithic write with targeted segment flashing:
   ```javascript
   // Standard Firmware Update mode: only write the application partition!
   const fileArray = [
       { data: appBinary, address: 0x00010000 } // NVS at 0x9000 is NEVER touched
   ];
   ```
2. **Web Flasher Options**:
   - Default primary button: **"Update Firmware"** (flashes only to `0x10000`, preserving 100% of musician presets in NVS).
   - Advanced / recovery option: **"Full Factory Restore"** (explicitly re-initializes NVS only if the user deliberately selects it).

---

### 2.4 Technical Advantages of NVS for Musician Presets

1. **Native Wear-Leveling**: ESP-IDF NVS distributes erase/write operations across all 4 KB sectors of the partition, preventing flash cell degradation from frequent preset saving.
2. **Atomicity & Power-Loss Fault Tolerance**: If the musician cuts instrument power during a preset save, NVS's journaling transaction system ensures no corruption occurs (the slot retains its previous valid version).
3. **Storage Capacity**: 24 KB can store over 300 preset structs of 64 bytes each, leaving ample room for 8 to 16 preset slots, the working state, and calibration history.
4. **Zero-Allocation Rust Integration**: The `esp-idf-svc::nvs::EspDefaultNvsPartition` crate provides a robust API for serializing and deserializing binary blobs without heap churn.

---

## 3. Data Model & Preset Versioning

### 3.1 Preset Data Structure (`UserPreset`)

To satisfy real-time constraints (`no_std`, zero heap allocations in performance loops), presets are stored as compact, fixed-size structures:

```rust
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UserPreset {
    pub magic: u32,             // 0x45585052 ("EXPR")
    pub schema_version: u8,     // Format revision (v1, v2, etc.)
    pub slot_id: u8,            // 0..7 (8 user slots)
    pub name: [u8; 16],         // Fixed ASCII/UTF-8 label (e.g. "Lead Theremin  ")
    
    // Musical parameters
    pub root_note: u8,          // Base key (0..11: C to B)
    pub scale_type: u8,         // Major, Minor, Pentatonic, Arabic, etc.
    pub octave: i8,             // -2..+2
    pub pitch_bend_range: u8,   // 1..48 semitones
    pub quantization_mode: u8,  // Off, Smooth, Hard
    pub quantization_amount: u8,// 0..100 %
    
    // DSP & ToF Sensors
    pub filter_speed: u8,       // 0..100 (reactive low-pass smoothing)
    pub pitch_min_dist_mm: u16, // Near pitch active boundary (e.g. 50 mm)
    pub pitch_max_dist_mm: u16, // Far pitch active boundary (e.g. 450 mm)
    pub vol_min_dist_mm: u16,   // Near volume mute boundary (e.g. 30 mm)
    pub vol_max_dist_mm: u16,   // Far volume max boundary (e.g. 350 mm)
    
    // Transports & Protocols
    pub midi_channel: u8,       // 1..16
    pub midi_mode: u8,          // Standard MIDI vs MPE
    pub reference_freq_hz: u16, // Tuner reference (e.g. 4400 for 440.0 Hz)
    
    pub crc32: u32,             // Data integrity checksum
}
```

### 3.2 Storage Slot Management

- **Slots 0 to 7**: 8 slots freely editable and savable by the musician.
- **Slot `ACTIVE_STATE`**: Automatically saved on shutdown or preset switch, allowing seamless restore upon boot.
- **Slot `FACTORY_DEFAULT` (ROM)**: Immutable preset hardcoded in flash memory, enabling an instant factory reset whenever needed.

---

## 4. Menu Hierarchy & Slint UI Navigation

### 4.1 Menu Hierarchy

```
[ Root: Main Live Theremin Performance View ]
│
├── [ Touch Button / Physical MENU ] ────┐
│                                         ▼
├── 1. PRESETS MANAGER
│   ├── Load Preset (Slots 1 to 8 with preview and name)
│   ├── Save to Slot... (Quick rename / overwrite)
│   ├── Revert / Compare (Recall original slot state)
│   └── Factory Reset (Restore default factory sound)
│
├── 2. MUSICAL SETTINGS & SCALES
│   ├── Root Key & Transposition
│   ├── Scale Selection (Equal-tempered, Microtonal, Maqam, Just intonation)
│   ├── Quantization Mode (Continuous, Progressive, Strict)
│   └── Pitch-Bend Range (1 to 48 semitones)
│
├── 3. SENSORS & CALIBRATION (VL53L4CD ToF)
│   ├── Calibrate Pitch Distance (Hand near zero / Hand far limit)
│   ├── Calibrate Volume Distance (Cutoff threshold / Max volume)
│   ├── Filter Speed & Smoothing Responsiveness
│   └── I2C Diagnostics & Sensor Refresh Rate
│
├── 4. PROTOCOL & OUTPUTS (MIDI / MPE / USB)
│   ├── MIDI Channel / MPE Master & Member channels
│   ├── USB-MIDI Coprocessor Mode (RP2040-Zero @ 0x40)
│   └── CC Routing (CC74 Timbre, Aftertouch, Modulation)
│
├── 5. STROBE TUNER VIEW
│   ├── Frequency Reference (A = 440 Hz ± 10 Hz)
│   └── Dual-Needle Gauge Sensitivity
│
└── 6. SYSTEM & INFO
    ├── UI Language [ French / English ]
    ├── Firmware Versions (ESP32-S3 Core + RP2040 Coprocessor)
    └── Reboot / Bootloader Update Mode
```

### 4.2 Dual Navigation Interaction (Touch + Rotary Encoder)

1. **Touch Navigation**:
   - Direct tab selection and interactive cards.
   - Drag / tap adjustments for continuous values.
   - Universal back button `[ < BACK ]` on header.

2. **Rotary Encoder & Hardware Buttons**:
   - **Navigation Mode (Gold outline focus)**: Rotating the encoder cycles through selectable parameter cards or menu items.
   - **Encoder Click**: Enters edit mode on the selected item or opens the chosen submenu.
   - **Edit Mode (Pulsing cyan outline)**: Rotating the encoder increments or decrements the active value. Click to confirm.
   - **Long Press / Double Click**: Navigates back one level in the hierarchy (`[ < BACK ]`).
