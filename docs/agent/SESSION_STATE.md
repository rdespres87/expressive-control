# Session State

## Status
READY_FOR_HANDOFF

## Milestone
V0.6 ToF Theremin Prototype (ESP32-S3 + CrowPanel 5.0" Slint UI)

## Current task
Orchestration Grill : Architecture temps réel lock-free et ergonomie tactile unifiée.

## Last completed
- Démarrage par défaut sur l'écran Master (`is_master_view: true`).
- Agrandissement massif des zones tactiles et des typographies sur tout le système (Master & Slave) :
  - Élargissement significatif des marges gauche et droite du menu Config à 42px pour décoller complètement le texte et les boutons des bords physiques de l'écran.
  - Ajout de paddings horizontaux intérieurs de 10-12px dans chaque panneau pour que le texte et les boutons respirent avec une vraie marge.
  - Ajustement fin des polices ("Chromatic" à 18px, "Quantization" à 17px) pour garantir que le texte ne frotte sur aucun bord.
- Correction du chevauchement de la jauge sur le bouton Config (z-order Slint + restriction x<=560 sur y<85).
- Optimisation de la fluidité et suppression de la latence de bascule d'écran :
  - Harmonisation et symétrie du bouton de retour vers la configuration (`CONFIG ▶` + sous-titre `SETTINGS`) avec flèche assortie au bouton `◀ TUNER` (`LIVE VIEW`).
  - Élargissement de la hitbox du bouton CONFIG sur Slave (200x72px, +55% de surface tactile, police 24px) pour éliminer les ratés tactiles sur les bords d'écran.
  - Suppression des animations CSS de transition de fond (100ms) sur les boutons de bascule pour une réactivité brute sans délai artificiel.
  - Forçage automatique de 2 frames de rafraîchissement complet lors d'une transition de vue pour repeindre instantanément les deux framebuffers PSRAM.
  - Déclenchement instantané dès l'enfoncement physique (`pointer-event` `PointerEventKind.down` au lieu d'attendre le relâchement `clicked`).
  - Débounce temporel de 250 ms (`AtomicU32` ms) pour prévenir tout basculement parasite en boucle sous le doigt.
  - Lecture I2C atomique en 1 seule transaction de 7 octets dans le driver GT911 (`touch.rs`) avec conformité datasheet (lecture des coordonnées avant l'envoi de l'ACK).
  - Introduction de l'enum `TouchEvent::NoUpdate` pour éliminer le contact bounce et les faux `PointerReleased` intermittents.
  - Élimination de `clear_framebuffers()` bloquant sur 1,5 Mo de PSRAM SPI.
  - Réduction des délais FreeRTOS de boucle de 20 ms à 1-2 ms (cadencement à 60 FPS réels synchrone VSYNC et polling tactile GT911 à 60 Hz).
- Réalisation complète de la session Grill Orchestrator (Maker + Inquisitor) :
  - Création du module `concurrency.rs` (SeqLock wait-free `AtomicTunerSnapshot` et file SPSC lock-free `ConfigMessage` via `heapless 0.9`).
  - Découplage complet de `TofThereminProcessor` : suppression de tout `Mutex` bloquant sur la boucle de synthèse audio/capteurs.
  - Résolution intégrale de tous les avertissements Clippy (`cargo clippy --workspace -- -D warnings` et `--features demo` à 100% sans warning).
  - Tâche ajoutée dans `TASKS.md` pour le pilotage direct du double ToF via GPIO 43/44 (XSHUT sans expander).
  - Validation CI locale et flashage du firmware release sur l'ESP32-S3.

## Next action
- Câbler et valider le double ToF VL53L4CD directement sur l'ESP32-S3 via GPIO 43 (XSHUT Pitch) et GPIO 44 (XSHUT Volume) avec polling à 50 Hz.

## Blockers
- Aucun.
