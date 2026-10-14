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
- [x] **Étude Pinout, Architecture USB & IO Expander**:
  - Validation du bus I2C unique (`GPIO 19` SDA, `GPIO 20` SCL) partagé entre l'écran tactile GT911, le PCA9557 interne, l'expander externe et les 2 capteurs ToF.
  - Confirmation matérielle : le port USB-C existant est relié en dur au pont USB-UART CH340G (`GPIO 43/44`), empêchant l'USB-MIDI natif sur ce port sans compromettre le flash du firmware.
  - Choix de l'expander I2C : **Waveshare MCP23017** (16 E/S, pull-ups 100k intégrés) pour centraliser :
    - 2 sorties `XSHUT` pour le boot séquentiel des 2 VL53L4CD
    - 3 entrées pour l'encodeur rotatif (Phase A, Phase B, Switch clic)
    - 3 entrées pour les 3 boutons de commande
  - Choix de l'interface USB-MIDI dédiée : module externe **RP2040-Zero** / **Seeed XIAO** configuré en esclave USB-MIDI Class Compliant.
- [ ] **Intégration du MCP23017 & Double ToF**:
  - Implémenter le driver MCP23017 sur le bus I2C (adresse 0x20).
  - Séquencer le reset des lignes XSHUT des 2 VL53L4CD pour leur attribuer les adresses 0x30 et 0x31.
  - Implémenter la boucle de lecture polling (200 Hz encodeur/boutons, 50 Hz ToF distances) sur le Core 1.
- [ ] **Intégration du module USB-MIDI (RP2040-Zero)**:
  - Définir le protocole de communication ESP32 ↔ RP2040 (I2C esclave ou liaison série).
  - Émettre les paquets MIDI temps réel (NoteOn, PitchBend, CC).
### Interface & Ergonomie (Slint UI)
- [x] **UI Unifiée Master / Slave (Mono-Écran)**: Vue commutable dynamique permettant de basculer entre la vue Master (réglages Root, Scale, Octave, PB range, Quantization, Filter speed) et la vue Slave (grand Strobe Tuner) via boutons tactiles (`CONFIG` / `TUNER`) avec synchronisation directe du `TofThereminProcessor` en local (mono-carte, sans passer par l'UART).
- [x] **Internationalisation (i18n)**: Support multilingue (Français et Anglais) via table de chaînes statiques en Rust sans allocation sur la boucle de rendu (`i18n.rs`) et commutateur interactif `[ FR / EN ]` sur l'UI Slint.

### Système, Déploiement & Outils
- [ ] **Stratégie de Mise à Jour**: Définir et implémenter la stratégie de mise à jour firmware (OTA Wi-Fi / Web Installer, USB CDC / DFU, ou mise à jour UART inter-cartes si configuration dual-board).
- [ ] **Périphérique USB Composite (MIDI + CDC)**: Supporter simultanément USB-MIDI et port série CDC via TinyUSB pour la simulation HIL et le débogage en direct.
- [ ] **Outil GUI de Télémétrie (Telemetry Viewer)**: Finaliser l'outil hôte pour décoder et afficher les logs hardware et la télémétrie en temps réel.
