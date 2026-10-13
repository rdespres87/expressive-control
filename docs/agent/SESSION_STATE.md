# Session State

## Status
IN_PROGRESS

## Milestone
V0.6 ToF Theremin Prototype (ESP32-S3 + CrowPanel 5.0" Slint UI)

## Current task
Intégration du touchscreen GT911 via PCA9557 et test tactile en direct sur la CrowPanel 5.0".

## Last completed
- Implémenté le module `touch.rs` :
  - Scanner de bus I2C (SDA=GPIO19, SCL=GPIO20).
  - Séquence matérielle de reset du GT911 via l'IO expander PCA9557 (`0x18`) pour sélection de l'adresse `0x5D`.
  - Pilote GT911 avec quittance de buffer (`0x814E`) et extraction des coordonnées X/Y normalisées (0..800, 0..480).
- Branché les événements de toucher dans Slint (`platform::dispatch_pointer_event`).
- Ajouté le bouton tactile interactif "TOUCH TEST" dans `theremin.slint` avec retour visuel immédiat (cyan, coordonnées X/Y, comptage des taps).
- Le scanner I2C et la séquence de réveil PCA9557 ont bien validé la présence du PCA9557 à `0x18` et du GT911 à `0x5D`.
- Amplificateur audio (`PA_EN` sur IO2 du PCA9557) explicitement désactivé et maintenu à `LOW` (zéro bruit parasite, consommation minimale).

## Next action
- Validation par l'utilisateur du toucher physique sur la dalle CrowPanel (bouton "TOUCH TEST" et comptage des taps).

## Blockers
- Aucun.
