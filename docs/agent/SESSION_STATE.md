# Session State

## Status
IN_PROGRESS

## Milestone
V0.6 ToF Theremin Prototype (ESP32-S3 + CrowPanel 5.0" Slint UI)

## Current task
Intégration du Strobe Tuner sur dalle CrowPanel 5.0" ESP32-S3 validée : double mode normal (télémétrie UART) et test autonome (démo).

## Last completed
- Moteur vectoriel subpixel antialiasing (`gauge_renderer.rs`) pour l'ESP32-S3 / Slint MCU sans rotations matérielles :
  - **Rail de l'arc continu** : épaisseur portée à **20.0 px**.
  - **101 encoches radiales** : crans réguliers à **5.0 px**, crans majeurs à **7.5 px**, repère 0 cent à **10.0 px**.
  - **Aiguille pivotante** : tige à **10.0 px**, moyeu à **16.0 px**, tête diamant à **18.0 px**.
  - Absence totale de rémanence / ghosting grâce au nettoyage systématique de la zone du cadran dans le double buffer PSRAM.
- Support du double mode dans `main.rs` et `Cargo.toml` :
  - Mode **Master** (`--features master` ou défaut) : ToF sensors + USB MIDI + UART telemetry.
  - Mode **Slave normal** (`--features slave`) : écoute UART1 télémétrie et affichage Strobe Tuner en temps réel.
  - Mode **Slave demo / test** (`--features demo`) : animation autonome pour valider et démontrer la jauge et l'écran sans master.
- Tous les tests de CI validés (formatage, clippy, unit tests workspace).
- Documentation mise à jour dans `README.md`.

## Next action
- Finaliser le commit et synchroniser.

## Blockers
- Aucun.
