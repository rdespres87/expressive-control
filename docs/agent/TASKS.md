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
- [x] **I2C Bus Scanner**: Implémenter un scan I2C au démarrage pour détecter et valider les adresses de tous les composants embarqués de la carte CrowPanel 5.0" V3.0 (Touch GT911 à 0x5D/0x14, IO expander PCA9557 à 0x18, etc. d'après les schémas Eagle).
- [x] **Driver Touchscreen**: Faire fonctionner l'écran tactile capacitif GT911 (initialisation/reset via PCA9557, coupure permanente de l'ampli audio PA_EN) et brancher les événements de touch sur le backend Slint.
- [ ] **Étude Pinout & IO Expander (ADA4886 / AW9523)**:
  - Étudier le pinout complet de la carte CrowPanel 5.0" (broches disponibles : bus I2C, GPIO 43/44, etc.).
  - Valider l'architecture d'extension I2C (ex: module GPIO Expander ADA4886 / AW9523 ou MCP23017) pour interfacer :
    - 2 capteurs ToF VL53L4CD (lignes XSHUT pour attribution d'adresses + interruptions)
    - 1 encodeur rotatif avec bouton cliquable
    - 3 boutons poussoirs de navigation/contrôle

### Interface & Ergonomie (Slint UI)
- [ ] **UI Unifiée Master / Slave (Mono-Écran)**: Concevoir une vue commutable permettant de basculer dynamiquement entre la vue Master (paramètres, capteurs, calibration) et la vue Slave (grand Strobe Tuner) via un bouton (cas d'un instrument avec un seul écran final).
- [ ] **Internationalisation (i18n)**: Prévoir le support multilingue (Français et Anglais pour commencer) pour l'ensemble des textes, libellés et menus de l'UI Slint.

### Système, Déploiement & Outils
- [ ] **Stratégie de Mise à Jour**: Définir et implémenter la stratégie de mise à jour firmware (OTA Wi-Fi / Web Installer, USB CDC / DFU, ou mise à jour UART inter-cartes si configuration dual-board).
- [ ] **Périphérique USB Composite (MIDI + CDC)**: Supporter simultanément USB-MIDI et port série CDC via TinyUSB pour la simulation HIL et le débogage en direct.
- [ ] **Outil GUI de Télémétrie (Telemetry Viewer)**: Finaliser l'outil hôte pour décoder et afficher les logs hardware et la télémétrie en temps réel.
