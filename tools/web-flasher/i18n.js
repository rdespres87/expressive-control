/**
 * Expressive Theremin Web Flasher — Internationalization (FR / EN)
 * 
 * Auto-detects browser language (defaults to French if browser is French, otherwise English).
 * Synchronized with the expressive-theremin on-device bilingual UI.
 */

export const TRANSLATIONS = {
    fr: {
        app_title: "Expressive Theremin — Web Firmware Flasher",
        brand_subtitle: "Mise à Jour de votre Instrument",
        status_disconnected: "Non connecté",
        status_connecting: "Connexion...",
        status_connected: "Thérémine Connecté",
        status_completed: "Mise à jour terminée",

        browser_warning_title: "Navigateur non compatible WebSerial",
        browser_warning_desc: "Cette application nécessite l'API WebSerial. Veuillez ouvrir cette page avec Google Chrome, Microsoft Edge, Brave ou Opera.",

        step_1_nav: "Connexion",
        step_2_nav: "Version",
        step_3_nav: "Installation",
        step_4_nav: "Prêt !",

        // Step 1
        step_1_title: "Étape 1 : Connectez votre Thérémine",
        step_1_desc: "Reliez votre Thérémine Expressif à votre ordinateur à l'aide d'un câble USB standard.",
        node_computer: "Ordinateur",
        badge_usb: "Câble USB",
        node_instrument: "Thérémine Expressif",
        node_instrument_sub: "Instrument de musique",
        chip_detected_title: "Thérémine Détecté & Prêt",
        chip_mac_sub: "Identifiant instrument : Connecté",
        metric_audio: "Système Audio",
        metric_ready: "Prêt",
        metric_usb: "Connexion USB",
        metric_high_speed: "Haute Vitesse",
        metric_midi: "Sortie MIDI",
        metric_builtin: "Intégrée",
        btn_connect: "Connecter mon Thérémine",
        btn_connecting: "Connexion en cours...",
        btn_disconnect: "Déconnecter",
        btn_continue_to_step_2: "Continuer vers le logiciel →",

        // Step 2
        step_2_title: "Étape 2 : Version du Logiciel",
        step_2_desc: "Choisissez la version à installer sur votre instrument.",
        badge_recommended: "Recommandé",
        firmware_title: "Système v0.1.0 — Version Officielle",
        firmware_tag: "Mise à jour complète",
        firmware_desc: "Dernière mise à jour logicielle officielle de l'instrument. Intègre le nouvel accordeur stroboscopique haute précision, le moteur sonore expressif et la compatibilité USB-MIDI universelle.",
        feat_1: "✨ Accordeur Stroboscopique haute précision et grand écran tactile",
        feat_2: "⚡ Réponse musicale instantanée et affichage ultra-fluide (60 FPS)",
        feat_3: "🎹 Sortie USB-MIDI universelle pour synthétiseurs et logiciels MAO",
        feat_4: "🇫🇷 Interface multilingue (Français & Anglais)",
        accordion_advanced: "Options avancées : installer un fichier personnalisé (.bin)",
        drop_zone_text: "Glissez-déposez un fichier .bin ici ou cliquez pour parcourir",
        btn_back: "← Retour",
        btn_to_step_3: "Continuer vers l'installation →",

        // Step 3
        step_3_title: "Étape 3 : Installation en cours",
        step_3_desc: "Veuillez patienter quelques instants sans débrancher le câble USB.",
        phase_ready: "Prêt à installer",
        phase_downloading: "Chargement du firmware officiel v0.1.0...",
        phase_flashing: "Installation du nouveau système logiciel...",
        phase_rebooting: "Finalisation et redémarrage de l'instrument...",
        progress_remaining: "Temps restant estimé : ~{sec}s",
        check_1: "Connexion avec l'instrument établie",
        check_2: "Installation du nouveau système logiciel",
        check_3: "Vérification et redémarrage automatique",
        terminal_summary: "Afficher le journal technique détaillé (pour les curieux)",
        btn_clear: "Effacer",
        btn_cancel: "← Annuler",
        btn_start_flash: "Lancer la mise à jour",
        btn_flashing: "Mise à jour en cours...",
        btn_retry: "Réessayer",

        // Step 4
        step_4_title: "Mise à jour terminée avec succès !",
        step_4_desc: "Votre Thérémine Expressif est à jour et prêt à jouer.",
        callout_title: "Ce qui se passe maintenant :",
        callout_desc: "L'instrument redémarre automatiquement avec sa nouvelle version. L'écran s'allume avec le panneau de réglages et l'accordeur stroboscopique.",
        callout_note: "Note : Si l'écran ne s'allume pas immédiatement selon votre modèle de câble USB, appuyez brièvement sur le bouton RESET situé à l'arrière ou rebranchez le câble.",
        btn_finish: "Terminer & Jouer 🎶",

        // Footer
        footer_brand: "Projet Expressive Control — Instrument de musique Open Source",
        link_doc: "Documentation",
        link_github: "GitHub",

        // Messages & Logs
        alert_no_device: "Aucun instrument connecté.",
        alert_invalid_chip: "L'appareil sélectionné n'est pas reconnu comme un Thérémine Expressif.",
        alert_bin_only: "Veuillez sélectionner un fichier binaire avec l'extension .bin.",
        alert_flash_error: "Échec de la mise à jour : {err}\nVérifiez que le câble n'a pas bougé et réessayez.",
        log_port_request: "[Info] Demande de sélection du port USB série...\n",
        log_init: "[Info] Initialisation de la liaison de mise à jour...\n",
        log_connected: "[Succès] Instrument reconnu : {chip}\n",
        log_disconnected: "[Info] Déconnecté de l'instrument.\n",
        log_cable_disconnected: "[Avertissement] Le câble USB a été débranché.\n",
        log_custom_file: "[Fichier] Binaire personnalisé chargé : {file} ({size} octets)\n",
        log_fetch_release: "[Info] Téléchargement du binaire v0.1.0 officiel...\n",
        log_release_ready: "[Info] Firmware officiel v0.1.0 prêt ({size} octets).\n",
        log_flash_success: "[Succès] Écriture du système terminée avec succès !\n",
        log_rebooting: "[Info] Redémarrage automatique de l'instrument...\n"
    },
    en: {
        app_title: "Expressive Theremin — Web Firmware Flasher",
        brand_subtitle: "Instrument Firmware Update",
        status_disconnected: "Not connected",
        status_connecting: "Connecting...",
        status_connected: "Theremin Connected",
        status_completed: "Update completed",

        browser_warning_title: "Browser not compatible with WebSerial",
        browser_warning_desc: "This application requires the WebSerial API. Please open this page using Google Chrome, Microsoft Edge, Brave, or Opera.",

        step_1_nav: "Connection",
        step_2_nav: "Version",
        step_3_nav: "Flashing",
        step_4_nav: "Ready!",

        // Step 1
        step_1_title: "Step 1: Connect your Theremin",
        step_1_desc: "Connect your Expressive Theremin to your computer using a standard USB cable.",
        node_computer: "Computer",
        badge_usb: "USB Cable",
        node_instrument: "Expressive Theremin",
        node_instrument_sub: "Musical instrument",
        chip_detected_title: "Theremin Detected & Ready",
        chip_mac_sub: "Instrument identifier: Connected",
        metric_audio: "Audio Engine",
        metric_ready: "Ready",
        metric_usb: "USB Connection",
        metric_high_speed: "High Speed",
        metric_midi: "MIDI Output",
        metric_builtin: "Built-in",
        btn_connect: "Connect my Theremin",
        btn_connecting: "Connecting...",
        btn_disconnect: "Disconnect",
        btn_continue_to_step_2: "Continue to software version →",

        // Step 2
        step_2_title: "Step 2: Software Version",
        step_2_desc: "Select the software version to install on your instrument.",
        badge_recommended: "Recommended",
        firmware_title: "System v0.1.0 — Official Release",
        firmware_tag: "Full Update",
        firmware_desc: "Latest official software update for the instrument. Includes the new high-precision strobe tuner, expressive sound engine, and universal USB-MIDI compatibility.",
        feat_1: "✨ High-precision Strobe Tuner and large color touchscreen",
        feat_2: "⚡ Zero-latency musical response and ultra-smooth 60 FPS display",
        feat_3: "🎹 Universal USB-MIDI output for synthesizers and DAWs",
        feat_4: "🇬🇧 Multilingual interface (French & English)",
        accordion_advanced: "Advanced options: install custom binary (.bin)",
        drop_zone_text: "Drag & drop a .bin file here or click to browse",
        btn_back: "← Back",
        btn_to_step_3: "Continue to flashing →",

        // Step 3
        step_3_title: "Step 3: Flashing in progress",
        step_3_desc: "Please wait a moment and do not unplug the USB cable.",
        phase_ready: "Ready to flash",
        phase_downloading: "Downloading official v0.1.0 firmware...",
        phase_flashing: "Writing new software to flash memory...",
        phase_rebooting: "Finalizing and rebooting instrument...",
        progress_remaining: "Estimated remaining time: ~{sec}s",
        check_1: "Instrument connection established",
        check_2: "Writing software to flash memory",
        check_3: "Verification & automatic reboot",
        terminal_summary: "Show detailed technical logs (for advanced users)",
        btn_clear: "Clear",
        btn_cancel: "← Cancel",
        btn_start_flash: "Start Update",
        btn_flashing: "Updating...",
        btn_retry: "Retry",

        // Step 4
        step_4_title: "Update completed successfully!",
        step_4_desc: "Your Expressive Theremin is up to date and ready to play.",
        callout_title: "What happens now:",
        callout_desc: "The instrument reboots automatically into the new version. The display turns on with the control settings panel and the strobe tuner.",
        callout_note: "Note: If the display does not turn on immediately depending on your USB cable model, briefly press the RESET button on the back or reconnect the cable.",
        btn_finish: "Finish & Play 🎶",

        // Footer
        footer_brand: "Expressive Control Project — Open Source Music Instrument",
        link_doc: "Documentation",
        link_github: "GitHub",

        // Messages & Logs
        alert_no_device: "No instrument connected.",
        alert_invalid_chip: "The selected device is not recognized as an Expressive Theremin.",
        alert_bin_only: "Please select a binary file with .bin extension.",
        alert_flash_error: "Update failed: {err}\nEnsure the cable is firmly connected and try again.",
        log_port_request: "[Info] Requesting serial USB port selection...\n",
        log_init: "[Info] Initializing firmware update link...\n",
        log_connected: "[Success] Instrument recognized: {chip}\n",
        log_disconnected: "[Info] Disconnected from instrument.\n",
        log_cable_disconnected: "[Warning] USB cable disconnected.\n",
        log_custom_file: "[File] Custom binary loaded: {file} ({size} bytes)\n",
        log_fetch_release: "[Info] Downloading official release v0.1.0...\n",
        log_release_ready: "[Info] Official v0.1.0 firmware ready ({size} bytes).\n",
        log_flash_success: "[Success] System write completed successfully!\n",
        log_rebooting: "[Info] Automatic instrument rebooting...\n"
    }
};

/**
 * Detect user's language: defaults to French if browser is set to French, otherwise English.
 */
export function detectLanguage() {
    const navLang = (navigator.language || navigator.userLanguage || "").toLowerCase();
    return navLang.startsWith("fr") ? "fr" : "en";
}
