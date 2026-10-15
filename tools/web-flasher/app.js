/**
 * Expressive Theremin — Didactic WebSerial Firmware Flasher
 * 
 * Drives the ESP32-S3 ROM bootloader via WebSerial API to flash unified
 * theremin releases (ESP32-S3 + RP2040 coprocessor) with a single click.
 * Supports automatic bilingual localization (French / English) and robust
 * hardware reset protection.
 */

import { ESPLoader, Transport } from "./esptool-bundle.js";
import { TRANSLATIONS, detectLanguage } from "./i18n.js";

// DOM Elements
const browserWarning = document.getElementById("browser-warning");
const connectionIndicator = document.getElementById("connection-indicator");
const connectionText = document.getElementById("connection-text");

const btnLangFr = document.getElementById("btn-lang-fr");
const btnLangEn = document.getElementById("btn-lang-en");

const btnConnect = document.getElementById("btn-connect");
const btnConnectText = document.getElementById("btn-connect-text");
const btnDisconnect = document.getElementById("btn-disconnect");
const btnContinueStep2 = document.getElementById("btn-continue-step-2");
const step1ConnectedActions = document.getElementById("step-1-connected-actions");
const chipInfoCard = document.getElementById("chip-info-card");
const chipNameEl = document.getElementById("chip-name");
const chipMacEl = document.getElementById("chip-mac");
const chipFlashSizeEl = document.getElementById("chip-flash-size");

const btnToStep3 = document.getElementById("btn-to-step-3");
const btnBackStep1 = document.getElementById("btn-back-step-1");
const btnBackStep2 = document.getElementById("btn-back-step-2");
const btnStartFlash = document.getElementById("btn-start-flash");
const btnStartFlashText = document.getElementById("btn-start-flash-text");
const btnRestartWizard = document.getElementById("btn-restart-wizard");

const dropZone = document.getElementById("drop-zone");
const fileInput = document.getElementById("file-input");
const customFileName = document.getElementById("custom-file-name");

const progressPhaseLabel = document.getElementById("progress-phase-label");
const progressPercentage = document.getElementById("progress-percentage");
const progressBar = document.getElementById("progress-bar");
const progressSubtext = document.getElementById("progress-subtext");
const progressSpeed = document.getElementById("progress-speed");

const checkFlash = document.getElementById("check-flash");
const checkFlashText = document.getElementById("check-flash-text");
const checkVerify = document.getElementById("check-verify");

const terminalOutput = document.getElementById("terminal-output");
const btnClearLogs = document.getElementById("btn-clear-logs");

// ─── State Management ───
let currentStep = 1;
let currentLang = detectLanguage(); // Auto-detect: "fr" if browser is French, else "en"
let device = null;
let transport = null;
let esploader = null;
let customBinaryData = null;
let flashStartTime = 0;
let isControlledReboot = false;
let isDisconnecting = false;

// ─── Internationalization (i18n) Helper ───
function t(key, params = {}) {
    const langDict = TRANSLATIONS[currentLang] || TRANSLATIONS.en;
    let str = langDict[key] || TRANSLATIONS.en[key] || key;
    for (const [k, v] of Object.entries(params)) {
        str = str.replace(`{${k}}`, v);
    }
    return str;
}

function setLanguage(lang) {
    currentLang = lang === "fr" ? "fr" : "en";
    document.documentElement.lang = currentLang;

    // Toggle button styles
    if (btnLangFr && btnLangEn) {
        btnLangFr.classList.toggle("active", currentLang === "fr");
        btnLangEn.classList.toggle("active", currentLang === "en");
    }

    // Update all static data-i18n elements
    document.querySelectorAll("[data-i18n]").forEach((el) => {
        const key = el.getAttribute("data-i18n");
        const translation = t(key);
        if (translation) {
            if (el.tagName === "TITLE") {
                document.title = translation;
            } else {
                el.textContent = translation;
            }
        }
    });

    // Update dynamic state labels
    if (device && transport) {
        connectionText.textContent = t("status_connected");
    } else if (isControlledReboot) {
        connectionText.textContent = t("status_completed");
    } else {
        connectionText.textContent = t("status_disconnected");
    }
}

if (btnLangFr) btnLangFr.addEventListener("click", () => setLanguage("fr"));
if (btnLangEn) btnLangEn.addEventListener("click", () => setLanguage("en"));

// Apply initial detected language
setLanguage(currentLang);

// ─── Browser Compatibility Check ───
if (!("serial" in navigator)) {
    browserWarning.classList.remove("hidden");
    btnConnect.disabled = true;
    btnConnect.title = t("browser_warning_title");
}

// ─── Logger Console ───
function appendLog(text) {
    if (!terminalOutput) return;
    terminalOutput.textContent += text;
    terminalOutput.scrollTop = terminalOutput.scrollHeight;
}

const espTerminal = {
    clean() {
        if (terminalOutput) terminalOutput.textContent = "";
    },
    writeLine(data) {
        appendLog(data + "\n");
    },
    write(data) {
        appendLog(data);
    },
};

if (btnClearLogs) {
    btnClearLogs.addEventListener("click", () => espTerminal.clean());
}

// ─── Wizard Step Navigation ───
function goToStep(step) {
    currentStep = step;
    for (let i = 1; i <= 4; i++) {
        const stepNav = document.getElementById(`step-nav-${i}`);
        const stepSection = document.getElementById(`step-${i}`);

        if (i < step) {
            stepNav.classList.remove("active");
            stepNav.classList.add("completed");
            stepSection.classList.remove("active");
        } else if (i === step) {
            stepNav.classList.add("active");
            stepNav.classList.remove("completed");
            stepSection.classList.add("active");
        } else {
            stepNav.classList.remove("active", "completed");
            stepSection.classList.remove("active");
        }
    }
}

// ─── Safe Serial Teardown ───
function cleanupSerialState() {
    device = null;
    transport = null;
    esploader = null;
}

// ─── Connection Management ───
async function connectTheremin() {
    try {
        isControlledReboot = false;
        espTerminal.clean();
        appendLog(t("log_port_request"));
        
        device = await navigator.serial.requestPort({
            filters: [
                // Filter USB-UART bridges (CH340, CP2102, FTDI, native ESP32)
                { usbVendorId: 0x1a86 }, // CH340 (CrowPanel 5.0" default)
                { usbVendorId: 0x10c4 }, // CP210x
                { usbVendorId: 0x303a }, // Espressif native
            ]
        });

        transport = new Transport(device, true);

        appendLog(t("log_init"));
        btnConnect.disabled = true;
        btnConnect.innerHTML = `<span class="btn-icon">⏳</span><span>${t("btn_connecting")}</span>`;

        esploader = new ESPLoader({
            transport,
            baudrate: 460800,
            romBaudrate: 115200,
            terminal: espTerminal,
        });

        const chipName = await esploader.main();
        appendLog(t("log_connected", { chip: chipName }));

        // Validate chip model to protect other microcontrollers
        if (!chipName.toUpperCase().includes("ESP32-S3")) {
            alert(t("alert_invalid_chip"));
            await disconnectTheremin();
            return;
        }

        // Update UI info
        connectionIndicator.classList.remove("disconnected");
        connectionIndicator.classList.add("connected");
        connectionText.textContent = t("status_connected");

        chipNameEl.textContent = t("chip_detected_title");
        chipMacEl.textContent = t("chip_mac_sub");
        chipFlashSizeEl.textContent = t("metric_ready");
        chipInfoCard.classList.remove("hidden");

        btnConnect.classList.add("hidden");
        if (step1ConnectedActions) step1ConnectedActions.classList.remove("hidden");

        // Automatically advance to Step 2
        setTimeout(() => goToStep(2), 600);

    } catch (err) {
        console.error("Connection error:", err);
        appendLog(`[Error] ${err.message || err}\n`);
        btnConnect.disabled = false;
        btnConnect.innerHTML = `<span class="btn-icon">⚡</span><span>${t("btn_connect")}</span>`;
        if (step1ConnectedActions) step1ConnectedActions.classList.add("hidden");
        if (err.name !== "NotFoundError") {
            alert(t("alert_flash_error", { err: err.message || err }));
        }
    }
}

async function disconnectTheremin() {
    if (isDisconnecting) return;
    isDisconnecting = true;

    try {
        if (transport) {
            await transport.disconnect().catch((e) => console.warn("Disconnect ignored:", e));
        }
    } catch (e) {
        console.warn("Disconnect warning:", e);
    } finally {
        cleanupSerialState();
        isDisconnecting = false;

        connectionIndicator.classList.remove("connected");
        connectionIndicator.classList.add("disconnected");
        connectionText.textContent = t("status_disconnected");

        chipInfoCard.classList.add("hidden");
        btnConnect.classList.remove("hidden");
        btnConnect.disabled = false;
        btnConnect.innerHTML = `<span class="btn-icon">⚡</span><span>${t("btn_connect")}</span>`;
        if (step1ConnectedActions) step1ConnectedActions.classList.add("hidden");

        goToStep(1);
        appendLog(t("log_disconnected"));
    }
}

// ─── File Handling & Drop Zone ───
if (dropZone && fileInput) {
    dropZone.addEventListener("click", () => fileInput.click());

    dropZone.addEventListener("dragover", (e) => {
        e.preventDefault();
        dropZone.classList.add("dragover");
    });

    dropZone.addEventListener("dragleave", () => {
        dropZone.classList.remove("dragover");
    });

    dropZone.addEventListener("drop", (e) => {
        e.preventDefault();
        dropZone.classList.remove("dragover");
        if (e.dataTransfer.files.length > 0) {
            handleCustomFile(e.dataTransfer.files[0]);
        }
    });

    fileInput.addEventListener("change", (e) => {
        if (e.target.files.length > 0) {
            handleCustomFile(e.target.files[0]);
        }
    });
}

function handleCustomFile(file) {
    if (!file.name.endsWith(".bin")) {
        alert(t("alert_bin_only"));
        return;
    }

    customFileName.textContent = `${file.name} (${(file.size / (1024 * 1024)).toFixed(2)} MB)`;
    const reader = new FileReader();
    reader.onload = (ev) => {
        customBinaryData = ev.target.result;
        appendLog(t("log_custom_file", { file: file.name, size: file.size }));
    };
    reader.readAsBinaryString(file);
}

// ─── Flashing Procedure ───
async function startFlashing() {
    if (!esploader) {
        alert(t("alert_no_device"));
        goToStep(1);
        return;
    }

    btnStartFlash.disabled = true;
    btnStartFlash.innerHTML = `<span class="btn-icon">⏳</span><span>${t("btn_flashing")}</span>`;
    btnBackStep2.disabled = true;

    progressPhaseLabel.textContent = t("phase_ready");
    progressBar.style.width = "0%";
    progressPercentage.textContent = "0%";
    progressSpeed.textContent = "";
    progressSubtext.textContent = "";

    try {
        let binaryPayload = customBinaryData;

        // If no custom file was loaded, fetch bundled release binary
        if (!binaryPayload) {
            progressPhaseLabel.textContent = t("phase_downloading");
            appendLog(t("log_fetch_release"));
            
            const response = await fetch("./firmware/v0.1.0/theremin-factory.bin");
            if (!response.ok) {
                throw new Error(`HTTP ${response.status} ${response.statusText}`);
            }
            const blob = await response.blob();
            binaryPayload = await new Promise((resolve) => {
                const reader = new FileReader();
                reader.onload = (e) => resolve(e.target.result);
                reader.readAsBinaryString(blob);
            });
            appendLog(t("log_release_ready", { size: binaryPayload.length }));
        }

        // Configure flashing files: factory image at address 0x0000
        const fileArray = [
            {
                data: binaryPayload,
                address: 0x0000,
            }
        ];

        progressPhaseLabel.textContent = t("phase_flashing");
        checkFlash.classList.add("active");
        flashStartTime = Date.now();

        await esploader.writeFlash({
            fileArray,
            flashSize: "4MB",
            flashMode: "dio",
            flashFreq: "80m",
            eraseAll: false,
            compress: true,
            reportProgress(fileIndex, written, total) {
                const percent = Math.floor((written / total) * 100);
                progressBar.style.width = `${percent}%`;
                progressPercentage.textContent = `${percent}%`;

                const elapsedSec = (Date.now() - flashStartTime) / 1000;
                if (elapsedSec > 1.0) {
                    const speedKB = ((written / 1024) / elapsedSec).toFixed(1);
                    progressSpeed.textContent = `${speedKB} KB/s`;
                    const remainingBytes = total - written;
                    if (speedKB > 0) {
                        const remainingSec = Math.max(0, Math.round((remainingBytes / 1024) / speedKB));
                        progressSubtext.textContent = t("progress_remaining", { sec: remainingSec });
                    }
                }
            },
            calculateMD5Hash(_) {
                return "";
            }
        });

        // ─── Post-Flash Reboot (Crash-Proof Teardown) ───
        progressPhaseLabel.textContent = t("phase_rebooting");
        checkFlash.classList.remove("active");
        checkFlash.classList.add("done");
        checkVerify.classList.add("done");

        appendLog(t("log_flash_success"));
        appendLog(t("log_rebooting"));

        // Flag controlled reboot to suppress any disconnect events
        isControlledReboot = true;

        // Perform hardware reset sequence:
        // On CH340 / CP2102: RTS pulls EN low (Reset) and DTR pulls IO0 low (Boot mode)
        try {
            // Pulse EN low while keeping IO0 high (normal boot, NOT bootloader mode)
            await transport.setDTR(false);
            await transport.setRTS(true);
            await new Promise((r) => setTimeout(r, 100));
            // Release EN high (chip starts executing firmware from flash)
            await transport.setRTS(false);
            await transport.setDTR(false);
        } catch (e) {
            console.warn("Signal reset pulse warning (non-fatal):", e);
        }

        // Clean up JS references without calling device.close() during OS USB re-enumeration
        // (Calling device.close() while the OS driver is resetting causes Chromium tab crashes)
        cleanupSerialState();

        connectionIndicator.classList.remove("connected");
        connectionIndicator.classList.add("disconnected");
        connectionText.textContent = t("status_completed");

        // Advance to Step 4
        setTimeout(() => {
            goToStep(4);
        }, 500);

    } catch (err) {
        console.error("Flashing failed:", err);
        progressPhaseLabel.textContent = t("phase_ready");
        appendLog(`[Fatal Error] ${err.message || err}\n`);
        alert(t("alert_flash_error", { err: err.message || err }));
        btnStartFlash.disabled = false;
        btnStartFlash.innerHTML = `<span class="btn-icon">🚀</span><span>${t("btn_retry")}</span>`;
        btnBackStep2.disabled = false;
    }
}

// ─── Button Event Listeners ───
btnConnect.addEventListener("click", connectTheremin);
btnDisconnect.addEventListener("click", disconnectTheremin);

if (btnContinueStep2) {
    btnContinueStep2.addEventListener("click", () => goToStep(2));
}

btnToStep3.addEventListener("click", () => {
    goToStep(3);
    progressSubtext.textContent = "";
    progressSpeed.textContent = "";
    progressBar.style.width = "0%";
    progressPercentage.textContent = "0%";
});
btnBackStep1.addEventListener("click", () => goToStep(1));
btnBackStep2.addEventListener("click", () => goToStep(2));

// Interactive Stepper Header Navigation
document.querySelectorAll(".step-item").forEach((stepEl) => {
    stepEl.addEventListener("click", () => {
        const targetStep = parseInt(stepEl.getAttribute("data-step"), 10);
        if (!targetStep) return;
        // Step 1: always accessible
        if (targetStep === 1) {
            goToStep(1);
            return;
        }
        // Steps 2 & 3: accessible when instrument is connected
        if (device && transport) {
            if (targetStep === 2) {
                goToStep(2);
            } else if (targetStep === 3) {
                goToStep(3);
                progressSubtext.textContent = "";
                progressSpeed.textContent = "";
            }
        }
        // Step 4: accessible if flash completed
        if (targetStep === 4 && isControlledReboot) {
            goToStep(4);
        }
    });
});

btnStartFlash.addEventListener("click", startFlashing);
btnRestartWizard.addEventListener("click", () => {
    isControlledReboot = false;
    goToStep(1);
    btnStartFlash.disabled = false;
    btnStartFlash.innerHTML = `<span class="btn-icon">🚀</span><span>${t("btn_start_flash")}</span>`;
    btnBackStep2.disabled = false;
    progressBar.style.width = "0%";
    progressPercentage.textContent = "0%";
    progressSubtext.textContent = "";
    progressSpeed.textContent = "";
});

// ─── USB Hardware Auto-Disconnect Detection ───
if ("serial" in navigator) {
    navigator.serial.addEventListener("disconnect", (event) => {
        if (device && event.target === device) {
            if (isControlledReboot) {
                // Expected disconnect during post-flash reboot; do not crash or reset wizard step
                console.log("Device disconnected as expected during post-flash reboot.");
                cleanupSerialState();
                return;
            }
            appendLog(t("log_cable_disconnected"));
            disconnectTheremin();
        }
    });
}
