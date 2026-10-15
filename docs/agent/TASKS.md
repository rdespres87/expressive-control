# Tasks

## Completed
- [x] Project architecture and rules definition
- [x] Git repository initialization and GitHub remote link
- [x] Cargo workspace setup (core, simulator, firmwares/nrf5340)
- [x] English translation of all documentation
- [x] GitHub Actions CI setup
- [x] Define core foundation traits (SensorReader, MidiTransport, SensorEvent, InstrumentEvent)
- [x] Design MPE Voice Allocator and `ControlProcessor` trait interfaces
- [x] Implement `ControlProcessor` internal routing logic (mapping events to MPE channels)
- [x] Implement raw MIDI byte serialization in `expressive-core`
- [x] Implement a basic PC Simulator driver for end-to-end testing (YAML scenario -> processor -> MIDI hex dump)
- [x] Scaffold Embassy nRF5340 App Core firmware with TrustZone secure feature (`nrf5340-app-s`) and fix workspace CI conflicts.

## Pending (nRF5340 Base)
- [x] Implement USB MIDI Class Compliant transport for nRF5340 (`embassy-usb`)
- [x] Implement I2C/SPI sensor reader for nRF5340 (`embassy-nrf`)

## ToF Theremin Project (ESP32-S3)

### Completed
- [x] Hardware feasibility study and component selection (ESP32-S3, VL53L5CX, Slint UI)
- [x] Add `SensorEvent::Position3D` definitions to `expressive-core`
- [x] Implement `TofThereminProcessor` logic in `instruments/theremin`
- [x] Initialize new Cargo workspace member `firmwares/expressive-theremin` (`esp-idf` based)
- [x] Integrate PC Simulator multi-instrument dispatch and HIL theremin scenario
- [x] Implement VL53L4CD I2C driver with dual-sensor address initialization
- [x] Implement Native USB-MIDI transport via TinyUSB
- [x] Design Dual-Board Architecture and Mockups (Master + Slave with extension arms and wood unibody)
- [x] Define UART protocol structure (COBS + Postcard) between Master and Slave
- [x] Implement UART telemetry transmission in Master (and remove `#![allow(dead_code)]` from `protocol.rs`)
- [x] Integrate Elecrow CrowPanel 5.0" RGB LCD display driver in `platform.rs` (16MHz PCLK, double-buffer PSRAM, tear-free VSYNC)
- [x] Subpixel vector rasterizer for Strobe Tuner gauge & pivoting needle in `gauge_renderer.rs`
- [x] Support Master, Slave (UART telemetry listener), and Demo (autonomous test loop) build modes via Cargo features
- [x] Implement a debug telemetry system for tracing MIDI messages, raw sensor values, and processor state

### Hardware & Peripherals (CrowPanel 5.0" & Sensors)
- [x] **I2C Bus Scanner**: Implement boot-time I2C scan to detect and validate onboard peripheral addresses for CrowPanel 5.0" V3.0 (Touch GT911 at 0x5D/0x14, IO expander PCA9557 at 0x18, etc. from Eagle schematics).
- [x] **Touchscreen Driver**: Initialize capacitive touch GT911 (reset sequence via PCA9557, permanent audio PA_EN mute) and forward touch pointer events to Slint UI backend.
- [x] **Pinout, USB Architecture & IO Expander Study**:
  - Validation of single shared I2C bus (`GPIO 19` SDA, `GPIO 20` SCL) between GT911 touch, PCA9557, external expander, and dual ToF sensors.
  - Hardware confirmation: onboard USB-C port is hardwired to CH340G USB-UART bridge (`GPIO 43/44`), preventing native USB-MIDI without breaking flashing.
  - Expander selection: **Waveshare MCP23017** (16 I/O, built-in 100k pull-ups) to centralize:
    - 2 `XSHUT` outputs for sequential boot of dual VL53L4CD sensors
    - 3 inputs for rotary encoder (Phase A, Phase B, Switch click)
    - 3 inputs for tactile control buttons
  - Auxiliary USB-MIDI interface selection: **RP2040-Zero** coprocessor configured as an I2C Target (@ `0x40`) / USB-MIDI Class-Compliant endpoint.
- [x] **Generic MCP23017 Expander & Composable Driver Architecture**:
  - Implement a generic, reusable MCP23017 driver on I2C (`0x20`) (`mcp23017.rs`).
  - Implement standalone composable peripheral drivers: `DebouncedButton`, `RotaryEncoder` (Gray-code quadrature with detent filter).
  - Implement sequential dual ToF boot controller (`XSHUT` lines on GPA0/GPA1 to re-address sensors to `0x30` and `0x31`).
  - Implement Theremin front-panel driver composing buttons (Octave +, Octave -, Mute) and rotary encoder (UI navigation).
  - Implement resilient hardware-absent fallback with Slint UI warning banner.
- [x] **RP2040-Zero Inter-MCU Protocol**: Validated I2C Target link (Slave @ `0x40`) at 400 kHz to prevent hardware UART0 / CH340G bus contention.
- [x] **Dual-Core Asynchronous Decoupling (Real-Time Sensor & 60 FPS UI)**:
  - Pin sensor polling (GT911 touch, MCP23017, ToF ranging) and musical DSP to **Core 1** (`APP_CPU`) at 100 Hz (10ms), isolated from LCD VSYNC.
  - Pin Slint UI and LCD RGB DMA flush to **Core 0** (`PRO_CPU`) at 60 FPS.
  - Inter-core communication: wait-free SeqLock `AtomicTunerSnapshot` for musical state, lock-free SPSC `heapless::spsc::Queue` for touch/encoder UI input events and configuration changes.
- [x] **USB-MIDI Firmware (RP2040-Zero)**:
  - Implement RP2040 I2C Target firmware (`firmwares/rp2040-usb-midi`) receiving 3-byte MIDI packets and streaming Class-Compliant USB-MIDI to host DAW. Relocated to `0x10004000`.

### Interface & Ergonomics (Slint UI)
- [x] **Unified Master / Slave UI (Single-Screen)**: Dynamic switchable view toggling between Master view (Root, Scale, Octave, PB range, Quantization, Filter speed) and Slave view (Strobe Tuner) via touch buttons (`CONFIG` / `TUNER`) with direct local `TofThereminProcessor` sync.
- [x] **Internationalization (i18n)**: Zero-allocation static string table in Rust (`i18n.rs`) with interactive `[ FR / EN ]` toggle on Slint UI.
- [x] **Physical Navigation & Focus Management (Rotary Encoder & Hardware Buttons)**:
  - Implement visual focus outline / highlight in Slint UI (`master.slint`) indicating the currently selected parameter card (Gold for navigate, pulsing cyan for edit).
  - Dual-mode encoder interaction via `NavigationController` (`navigation.rs`): **Navigate mode** (rotate to cycle through parameter cards: Root, Scale, Octave, Range, MIDI Mode, PB Range, Quantization, Filter, Tuner switch) and **Edit mode** (click to activate, rotate to adjust value, click to confirm).
  - Hardware buttons integration: dedicated Octave + / Octave - quick-shifting and instant Mute toggle with tactile visual feedback (`MUTED` banner on UI).
  - Tuner view navigation: encoder rotation for reference pitch calibration (440.0 Hz ± 10 Hz) and click to toggle between Master and Tuner views.
- [ ] **Configuration Submenus & Menu Tree (Slint UI)**:
  - Architecture study documented in [docs/studies/user_configuration_and_menu_architecture.md](docs/studies/user_configuration_and_menu_architecture.md).
  - Implement hierarchical view navigation with header breadcrumb and back button (`[ < RETOUR ]`).
  - Categorized submenus:
    - **Presets Manager**: Slot selection (1 to 8), Save, Recall, Rename, and Factory Reset.
    - **Musical Scales & DSP**: Key root, Microtonal / Maqam / Just intonation scales, Quantization curve, PB range.
    - **Sensors Calibration**: Distance boundaries for Pitch and Volume ToF sensors (VL53L4CD), zero-point offset, filter smoothing.
    - **MIDI & Protocol**: Channel routing, MPE master/member channels, CC mappings, RP2040 coprocessor status.
    - **System & Info**: Firmware versions, bilingual toggle, display brightness, I2C bus diagnostic.
- [ ] **Non-Destructive User Presets Persistence (ESP-IDF NVS Flash)**:
  - Implement `UserPreset` serialization format with magic header (`0x45585052`), schema versioning, and CRC32 integrity check.
  - Dedicate persistent storage in ESP32-S3 `nvs` partition (`0x9000`), strictly isolated from the application code partition (`0x10000`) so firmware updates never erase user configurations.
  - Implement active state auto-save at power-down and transparent migration fallback to hardcoded factory presets (`ROM`).
  - Web Flasher safeguard: transition from monolithic `0x0000` image (`theremin-factory.bin`) to segmented write (`app.bin` @ `0x10000`) so web updates preserve existing musician NVS blocks.

### System, Deployment & Tools
- [x] **Firmware Update Strategy (Unified CrowPanel USB -> RP2040 I2C ISP)**:
  - RP2040 I2C Bootloader (`firmwares/rp2040-i2c-bootloader`) with ROM flash programming and CRC32 metadata validation.
  - ESP32-S3 Master Flasher (`rpe_flasher.rs`) streaming blocks over internal I2C bus at startup.
  - Automated RP2040 build and ELF extraction in `expressive-theremin/build.rs`.
  - Offline zero-dependency pure Rust `tools/elf2uf2` generator producing `rp2040-i2c-bootloader.uf2` for initial USB bootstrap.
- [x] **Didactic Web Flasher (WebSerial)**: Zero-install web update utility running in Chrome/Edge. Allows end-users to flash unified theremin firmware releases (`ESP32-S3 + RP2040`) in one click via the CrowPanel USB-C port with interactive step-by-step guidance.
- [x] **Slint Graphic Rendering Performance Optimization**:
  - Replace VSYNC busy-wait loop in `platform.rs` with zero-CPU FreeRTOS ISR task notifications.
  - Precompute 101 tick geometries (`PRECOMPUTED_TICKS`) and implement squared-distance fast-path in `gauge_renderer.rs`.
  - Eliminate heap `format!` string allocations in 60 FPS UI loop via conditional diffing and `heapless::String` stack formatting.
- [x] **Automated Web Flasher Deployment (GitHub Pages)**: Configured GitHub Actions workflow (`.github/workflows/deploy-flasher.yml`) to automatically publish `tools/web-flasher` to GitHub Pages upon push/release, providing musicians with a zero-install public HTTPS web URL.
- [ ] **Composite USB Device (MIDI + CDC)**: Support simultaneous USB-MIDI and serial CDC via TinyUSB for HIL simulation and live debug.
- [ ] **GUI Telemetry Tool (Telemetry Viewer)**: Finalize host utility to decode and render hardware telemetry logs in real time.


