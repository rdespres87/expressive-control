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
- [ ] **USB-MIDI Firmware (RP2040-Zero)**:
  - Implement RP2040 I2C Target firmware receiving 3-byte MIDI packets and streaming Class-Compliant USB-MIDI to host DAW.

### Interface & Ergonomics (Slint UI)
- [x] **Unified Master / Slave UI (Single-Screen)**: Dynamic switchable view toggling between Master view (Root, Scale, Octave, PB range, Quantization, Filter speed) and Slave view (Strobe Tuner) via touch buttons (`CONFIG` / `TUNER`) with direct local `TofThereminProcessor` sync.
- [x] **Internationalization (i18n)**: Zero-allocation static string table in Rust (`i18n.rs`) with interactive `[ FR / EN ]` toggle on Slint UI.

### System, Deployment & Tools
- [ ] **Firmware Update Strategy**: Define and implement update strategy (OTA Wi-Fi / Web Installer, USB CDC / DFU, or UART inter-board).
- [ ] **Composite USB Device (MIDI + CDC)**: Support simultaneous USB-MIDI and serial CDC via TinyUSB for HIL simulation and live debug.
- [ ] **GUI Telemetry Tool (Telemetry Viewer)**: Finalize host utility to decode and render hardware telemetry logs in real time.

