use core::cell::RefCell;
use core::fmt;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

#[allow(unused_imports)]
use crate::protocol::{MasterMessage, TunerState, MAX_MESSAGE_SIZE};
use embedded_hal_async::i2c::I2c as _;
use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::i2c::{I2cDriver, I2cError};
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::task::block_on;
use esp_idf_svc::sys::EspError;
use expressive_core::processor::ControlProcessor;
use expressive_core::{InstrumentEvent, SensorEvent};
use theremin::TofThereminProcessor;

// Slint UI is always compiled.
slint::include_modules!();
#[cfg(feature = "slave")]
#[allow(dead_code)]
type ThereminUi = SlaveWindow;
#[cfg(not(feature = "slave"))]
#[allow(dead_code)]
type ThereminUi = MasterWindow;

pub mod gauge_renderer;
pub mod platform;
pub mod protocol;

/// Defines the hardware architecture the firmware is built for.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ThereminMode {
    /// Legacy mode: One board reads both sensors and handles everything.
    SingleBoard,
    /// Dual-board: This board is the "Master". It reads BOTH Volume and Pitch ToFs,
    /// ensures perfect synchronization, outputs USB MIDI, and sends UI telemetry via UART.
    MasterBoard,
    /// Dual-board: This board is the "Slave" (Pitch display). It reads no sensors,
    /// but receives UI telemetry via UART to render the Strobe Tuner in real-time.
    SlaveBoard,
    /// Autonomous Demo / Test mode for the Strobe Tuner display on CrowPanel 5.0".
    SlaveDemo,
}

/// Change this constant or feature to build for the desired board mode.
#[cfg(feature = "demo")]
const BOARD_MODE: ThereminMode = ThereminMode::SlaveDemo;

#[cfg(all(feature = "slave", not(feature = "demo")))]
const BOARD_MODE: ThereminMode = ThereminMode::SlaveBoard;

#[cfg(all(not(feature = "slave"), not(feature = "demo")))]
const BOARD_MODE: ThereminMode = ThereminMode::MasterBoard;

/// Default 7-bit I2C address of a VL53L4CD after power-up.
#[allow(dead_code)]
const VL53L4CD_DEFAULT_ADDR: u8 = 0x29;
/// 7-bit I2C address assigned to the volume sensor during the sequential boot.
#[allow(dead_code)]
const VOLUME_SENSOR_ADDR: u8 = 0x2A;
/// Simulated distance value when no sensors are connected (cm).
const SIMULATED_DISTANCE: u16 = 50;
/// 16-bit index of the VL53L4CD `I2C_SLAVE__DEVICE_ADDRESS` register.
#[allow(dead_code)]
const REG_I2C_SLAVE_DEVICE_ADDRESS: [u8; 2] = [0x00, 0x01];
/// Ranging timing budget (20 ms => ~50 Hz). 50Hz provides a more stable, less noisy signal than 100Hz for the Theremin.
#[allow(dead_code)]
const TIMING_BUDGET_MS: u32 = 20;
/// 0 selects continuous ranging mode.
#[allow(dead_code)]
const INTER_MEASUREMENT_MS: u32 = 0;
/// Delay before rebooting after a fatal error, so logs can be read on the console.
const FATAL_RESTART_DELAY_MS: u32 = 5_000;
/// USB-MIDI virtual cable used for all outgoing messages.
#[allow(dead_code)]
const USB_MIDI_CABLE: u8 = 0;

/// Shared sensor state accessible by all threads.
struct SensorState {
    /// Current pitch distance in cm.
    pitch_distance: u16,
    /// Current volume distance in cm.
    volume_distance: u16,
    /// Whether the theremin is "playing" (hand detected).
    is_playing: bool,
    /// Current pitch bend value (0-16383, 8192 = center).
    pitch_bend: u16,
    /// Base MIDI note (0-127).
    base_note: u8,
}

impl Default for SensorState {
    fn default() -> Self {
        Self {
            pitch_distance: SIMULATED_DISTANCE,
            volume_distance: SIMULATED_DISTANCE,
            is_playing: false,
            pitch_bend: 8192,
            base_note: 60, // Middle C
        }
    }
}

type I2cBus = RefCell<I2cDriver<'static>>;
type Sensor<'a> =
    vl53l4cd::Vl53l4cd<AsyncI2cWrapper<'a>, embassy_time::Delay, vl53l4cd::wait::Poll>;

// --- Errors ---

/// Error returned by the shared I2C bus wrapper.
#[derive(Debug)]
enum BusError {
    /// The bus is already borrowed (re-entrant access), the transfer was not attempted.
    Busy,
    /// The underlying ESP-IDF I2C driver reported an error.
    I2c(I2cError),
}

impl embedded_hal::i2c::Error for BusError {
    fn kind(&self) -> embedded_hal::i2c::ErrorKind {
        match self {
            BusError::Busy => embedded_hal::i2c::ErrorKind::Other,
            BusError::I2c(e) => embedded_hal::i2c::Error::kind(e),
        }
    }
}

/// Root cause of a firmware error.
#[derive(Debug)]
enum ErrorSource {
    Esp(EspError),
    Bus(BusError),
    Sensor(vl53l4cd::Error<BusError>),
    Slint(&'static str),
}

impl From<EspError> for ErrorSource {
    fn from(e: EspError) -> Self {
        ErrorSource::Esp(e)
    }
}

impl From<BusError> for ErrorSource {
    fn from(e: BusError) -> Self {
        ErrorSource::Bus(e)
    }
}

impl From<vl53l4cd::Error<BusError>> for ErrorSource {
    fn from(e: vl53l4cd::Error<BusError>) -> Self {
        ErrorSource::Sensor(e)
    }
}

impl fmt::Display for ErrorSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorSource::Esp(e) => write!(f, "ESP-IDF error: {e}"),
            ErrorSource::Bus(e) => write!(f, "I2C bus error: {e:?}"),
            ErrorSource::Sensor(e) => write!(f, "VL53L4CD error: {e:?}"),
            ErrorSource::Slint(msg) => write!(f, "Slint platform error: {msg}"),
        }
    }
}

/// Firmware error enriched with the step that failed.
#[derive(Debug)]
struct FirmwareError {
    step: &'static str,
    source: ErrorSource,
}

impl fmt::Display for FirmwareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} failed: {}", self.step, self.source)
    }
}

/// Attaches a human-readable step description to any supported error.
trait Context<T> {
    fn context(self, step: &'static str) -> Result<T, FirmwareError>;
}

impl<T, E: Into<ErrorSource>> Context<T> for Result<T, E> {
    fn context(self, step: &'static str) -> Result<T, FirmwareError> {
        self.map_err(|e| FirmwareError {
            step,
            source: e.into(),
        })
    }
}

// --- Async I2C adapter ---

/// Adapts the blocking `I2cDriver` to the async I2C trait (vl53l4cd v0.4 is async).
/// The bus is shared between both sensors through a `RefCell`; borrowing is checked
/// at runtime with `try_borrow_mut` so a conflict is reported as an error, never a panic.
struct AsyncI2cWrapper<'a>(&'a I2cBus);

impl AsyncI2cWrapper<'_> {
    fn with_bus<R>(
        &self,
        f: impl FnOnce(&mut I2cDriver<'static>) -> Result<R, I2cError>,
    ) -> Result<R, BusError> {
        let mut bus = self.0.try_borrow_mut().map_err(|_| BusError::Busy)?;
        f(&mut bus).map_err(BusError::I2c)
    }
}

impl embedded_hal_async::i2c::ErrorType for AsyncI2cWrapper<'_> {
    type Error = BusError;
}

impl embedded_hal_async::i2c::I2c for AsyncI2cWrapper<'_> {
    async fn read(&mut self, address: u8, read: &mut [u8]) -> Result<(), Self::Error> {
        self.with_bus(|bus| embedded_hal::i2c::I2c::read(bus, address, read))
    }

    async fn write(&mut self, address: u8, write: &[u8]) -> Result<(), Self::Error> {
        self.with_bus(|bus| embedded_hal::i2c::I2c::write(bus, address, write))
    }

    async fn write_read(
        &mut self,
        address: u8,
        write: &[u8],
        read: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.with_bus(|bus| embedded_hal::i2c::I2c::write_read(bus, address, write, read))
    }

    async fn transaction(
        &mut self,
        address: u8,
        operations: &mut [embedded_hal_async::i2c::Operation<'_>],
    ) -> Result<(), Self::Error> {
        self.with_bus(|bus| {
            for op in operations {
                match op {
                    embedded_hal_async::i2c::Operation::Read(buf) => {
                        embedded_hal::i2c::I2c::read(bus, address, buf)?;
                    }
                    embedded_hal_async::i2c::Operation::Write(buf) => {
                        embedded_hal::i2c::I2c::write(bus, address, buf)?;
                    }
                }
            }
            Ok(())
        })
    }
}

// --- TinyUSB FFI ---

#[allow(dead_code)]
#[repr(C)]
struct TinyUsbConfig {
    _device_descriptor: *const core::ffi::c_void,
    _string_descriptor: *mut *const core::ffi::c_char,
    _string_descriptor_count: core::ffi::c_int,
    _external_phy: bool,
    _configuration_descriptor: *const u8,
    _self_powered: bool,
    _vbus_monitor_io: core::ffi::c_int,
}

#[allow(dead_code)]
extern "C" {
    fn tinyusb_driver_install(config: *const TinyUsbConfig) -> i32;
    fn tud_midi_n_stream_write(itf: u8, cable_num: u8, buffer: *const u8, bufsize: u32) -> u32;
}

/// USB Composite Device Descriptor (MIDI + CDC)
#[allow(dead_code)]
static DEVICE_DESCRIPTOR: [u8; 18] = [
    0x12, 0x01, 0x00, 0x02, 0xEF, 0x02, 0x01, 0x40, 0x89, 0x12, 0x33, 0x0D, 0x01, 0x01, 0x01, 0x03,
    0x01, 0x00,
];

#[allow(dead_code)]
static CONFIG_DESCRIPTOR: [u8; 112] = [
    0x09, 0x02, 0x73, 0x00, 0x03, 0x01, 0x00, 0xA0, 0xFA, 0x08, 0x0B, 0x02, 0x00, 0x02, 0x02, 0x02,
    0x01, 0x09, 0x04, 0x02, 0x00, 0x01, 0x02, 0x02, 0x01, 0x00, 0x05, 0x24, 0x00, 0x10, 0x01, 0x04,
    0x24, 0x02, 0x02, 0x05, 0x24, 0x06, 0x00, 0x03, 0x07, 0x05, 0x83, 0x03, 0x08, 0x01, 0xFF, 0x09,
    0x04, 0x03, 0x00, 0x00, 0x0A, 0x00, 0x00, 0x00, 0x09, 0x04, 0x01, 0x00, 0x00, 0x01, 0x01, 0x03,
    0x00, 0x09, 0x24, 0x01, 0x00, 0x01, 0x10, 0x01, 0x01, 0x01, 0x01, 0x09, 0x24, 0x02, 0x01, 0x01,
    0x03, 0x01, 0x01, 0x00, 0x09, 0x24, 0x03, 0x02, 0x01, 0x01, 0x01, 0x01, 0x00, 0x07, 0x24, 0x01,
    0x00, 0x01, 0x41, 0x00, 0x07, 0x05, 0x81, 0x02, 0x40, 0x00, 0x01, 0x05, 0x25, 0x01, 0x01, 0x00,
];

#[allow(dead_code)]
fn install_usb_midi() -> Result<(), FirmwareError> {
    let cfg = TinyUsbConfig {
        _device_descriptor: DEVICE_DESCRIPTOR.as_ptr() as *const core::ffi::c_void,
        _string_descriptor: core::ptr::null_mut(),
        _string_descriptor_count: 0,
        _external_phy: false,
        _configuration_descriptor: CONFIG_DESCRIPTOR.as_ptr(),
        _self_powered: false,
        _vbus_monitor_io: 0,
    };
    let ret = unsafe { tinyusb_driver_install(&cfg) };
    EspError::convert(ret).context("TinyUSB driver install")
}

/// Sends one MIDI message over USB.
#[allow(dead_code)]
fn send_usb_midi(bytes: &[u8]) {
    let Ok(len) = u32::try_from(bytes.len()) else {
        log::debug!("USB-MIDI TX skipped: message too long");
        return;
    };
    let written = unsafe { tud_midi_n_stream_write(0, USB_MIDI_CABLE, bytes.as_ptr(), len) };
    if written != len {
        log::debug!("USB-MIDI TX incomplete: {written}/{len} bytes ({bytes:02X?})");
    } else {
        log::debug!("USB-MIDI TX: {bytes:02X?}");
    }
}

// --- Sensors ---

#[allow(dead_code)]
fn new_sensor(bus: &I2cBus, addr: u8) -> Sensor<'_> {
    vl53l4cd::Vl53l4cd::with_addr(
        AsyncI2cWrapper(bus),
        addr,
        embassy_time::Delay,
        vl53l4cd::wait::Poll,
    )
}

/// Initialises the sensor currently answering on the default address and moves it
/// to `new_addr`. `set_device_address` was removed in vl53l4cd v0.4, so the register
/// is written directly.
#[allow(dead_code)]
async fn init_and_readdress(bus: &I2cBus, new_addr: u8) -> Result<Sensor<'_>, FirmwareError> {
    let mut sensor = new_sensor(bus, VL53L4CD_DEFAULT_ADDR);
    sensor
        .init()
        .await
        .context("sensor init (default address)")?;

    let [reg_hi, reg_lo] = REG_I2C_SLAVE_DEVICE_ADDRESS;
    AsyncI2cWrapper(bus)
        .write(VL53L4CD_DEFAULT_ADDR, &[reg_hi, reg_lo, new_addr])
        .await
        .context("sensor address change")?;

    Ok(new_sensor(bus, new_addr))
}

#[allow(dead_code)]
async fn start_ranging(sensor: &mut Sensor<'_>, name: &'static str) -> Result<(), FirmwareError> {
    sensor
        .set_range_timing(TIMING_BUDGET_MS, INTER_MEASUREMENT_MS)
        .await
        .context(name)?;
    sensor.start_ranging().await.context(name)
}

/// Reads the measurement signalled by the interrupt line, then re-arms the interrupt.
/// The interrupt is always cleared, even if the read failed, to avoid a stuck line.
#[allow(dead_code)]
async fn read_pending_distance(sensor: &mut Sensor<'_>) -> Result<u16, FirmwareError> {
    let measurement = sensor.read_measurement().await;
    let cleared = sensor.clear_interrupt().await;
    let measurement = measurement.context("pitch measurement read")?;
    cleared.context("pitch interrupt clear")?;
    Ok(measurement.distance)
}

// --- Thread 1: UI Thread ---

/// Run the Slint UI event loop in its own thread.
/// UI thread: initializes LCD, Slint platform, creates MasterWindow, enters the render loop.
/// Receives the shared sensor state so it can store the weak ref and poll updates.
fn ui_thread(_sensor_state: Arc<Mutex<SensorState>>) -> Result<(), FirmwareError> {
    log::info!("[UI Thread] Starting Slint UI...");

    // Initialize LCD display
    log::info!("[UI Thread] Initializing LCD display...");
    if !platform::init_lcd() {
        log::error!("[UI Thread] LCD initialization failed!");
        return Err(FirmwareError {
            step: "LCD init",
            source: ErrorSource::Slint("LCD init failed"),
        });
    }
    log::info!("[UI Thread] LCD display initialized");

    // Initialize Slint platform
    log::info!("[UI Thread] Initializing Slint platform...");
    if let Err(e) = platform::init_slint_platform() {
        log::error!("[UI Thread] Slint platform init failed: {:?}", e);
        return Err(FirmwareError {
            step: "Slint platform",
            source: ErrorSource::Slint("Slint platform init failed"),
        });
    }
    log::info!("[UI Thread] Slint platform initialized");

    // Create the Slint UI
    log::info!("[UI Thread] Creating MasterWindow...");
    let ui = match MasterWindow::new() {
        Ok(ui) => {
            log::info!("[UI Thread] MasterWindow created");
            ui
        }
        Err(e) => {
            log::error!("[UI Thread] MasterWindow::new() failed: {:?}", e);
            return Err(FirmwareError {
                step: "Slint UI create",
                source: ErrorSource::Slint("Slint UI create failed"),
            });
        }
    };

    // Store weak reference for sensor→UI sync
    log::info!("[UI Thread] Storing weak ref to MasterWindow...");
    let window_weak = ui.as_weak();
    {
        let mut weak_guard = platform::G_WINDOW_WEAK.lock().unwrap_or_else(|e| e.into_inner());
        *weak_guard = Some(window_weak.clone());
        log::info!("[UI Thread] Weak ref stored (G_WINDOW_WEAK populated)");
    }

    // Show the window
    log::info!("[UI Thread] Calling ui.show()...");
    if let Err(e) = ui.show() {
        log::error!("[UI Thread] ui.show() failed: {}", e);
        return Err(FirmwareError {
            step: "Slint UI show",
            source: ErrorSource::Slint("UI show failed"),
        });
    }
    log::info!("[UI Thread] ui.show() returned OK");

    // Force first render (draw_if_needed returns false until redraw is requested)
    log::info!("[UI Thread] Requesting first redraw...");
    if let Some(ref window_weak) = *platform::G_WINDOW_WEAK.lock().unwrap_or_else(|e| e.into_inner()) {
        if let Some(window) = window_weak.upgrade() {
            log::info!("[UI Thread] Window upgraded successfully");
            window.window().request_redraw();
            log::info!("[UI Thread] redraw requested");
        } else {
            log::warn!("[UI Thread] Window weak ref failed to upgrade");
        }
    } else {
        log::warn!("[UI Thread] G_WINDOW_WEAK is empty");
    }

    // Render the initial Slint UI frame while the backlight is still OFF
    log::info!("[UI Thread] Rendering initial frame to LCD before turning on backlight...");
    platform::flush_to_lcd();

    // Turn ON the backlight now that the display contains the painted UI (no boot flash!)
    platform::set_backlight(true);
    log::info!("[UI Thread] Backlight enabled (initial UI painted, zero flash)");
    log::info!("[UI Thread] Entering render loop...");

    // UI event loop: update timers, poll sensor state, render, and flush to LCD
    loop {
        // Update animations/timers (required for Slint animations to progress)
        if platform::has_active_window() {
            slint::platform::update_timers_and_animations();
        }

        // Poll sensor state and apply updates to the Slint window
        let weak_guard = platform::G_WINDOW_WEAK.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(ref window_weak) = *weak_guard {
            platform::poll_ui_updates(window_weak);
        }

        // Flush to LCD (returns true if a frame was rendered)
        let rendered = platform::flush_to_lcd();

        if !rendered {
            // No redraw was needed: sleep for 5ms to avoid 100% CPU burn,
            // preventing power supply ripple that causes MT9201 backlight flicker.
            FreeRtos::delay_ms(5);
        } else {
            // Frame was rendered and VSYNC-swapped: yield to background tasks
            esp_idf_hal::task::do_yield();
        }
    }
}

// --- Thread 2: Sensor Thread ---

/// Read sensors and update the shared sensor state.
async fn sensor_loop(
    sensor_state: Arc<Mutex<SensorState>>,
    stop_rx: &mut mpsc::Receiver<()>,
) -> Result<(), FirmwareError> {
    log::info!("[Sensor Thread] Starting sensor loop (simulated mode)...");

    let mut processor = TofThereminProcessor::default();
    let mut sim_distance: u16 = 500; // Start at middle distance
    let mut direction: i16 = -20; // Moving closer

    loop {
        // Check for stop signal (non-blocking)
        if stop_rx.try_recv().is_ok() {
            log::info!("[Sensor Thread] Stop signal received");
            break;
        }

        // Simulate distance changes (sweep back and forth)
        sim_distance = if sim_distance as i16 + direction > 1000 {
            direction = -20;
            1000
        } else if sim_distance as i16 + direction < 50 {
            direction = 20;
            50
        } else {
            (sim_distance as i16 + direction) as u16
        };

        let volume_dist = sim_distance; // Same simulation for both
        let pitch_dist = sim_distance;

        // Process pitch sensor event (id=0)
        let mut pitch_events = [InstrumentEvent::NoteOn {
            channel: expressive_core::Channel::new(1),
            note: expressive_core::U7::new(0),
            velocity: expressive_core::U7::new(0),
        }; 4];
        let mut pitch_telemetry = expressive_core::telemetry::NoopTelemetry;
        let _ = processor.process_event(
            SensorEvent::Position3D {
                id: 0,
                x: 0,
                y: 0,
                z: pitch_dist,
            },
            &mut pitch_events,
            &mut pitch_telemetry,
        );

        // Process volume sensor event (id=1)
        let mut volume_events = [InstrumentEvent::NoteOn {
            channel: expressive_core::Channel::new(1),
            note: expressive_core::U7::new(0),
            velocity: expressive_core::U7::new(0),
        }; 4];
        let _ = processor.process_event(
            SensorEvent::Position3D {
                id: 1,
                x: 0,
                y: 0,
                z: volume_dist,
            },
            &mut volume_events,
            &mut pitch_telemetry,
        );

        // Update shared state from processor
        {
            let mut state = sensor_state.lock().unwrap_or_else(|e| e.into_inner());
            state.pitch_distance = pitch_dist;
            state.volume_distance = volume_dist;
            state.is_playing = processor.is_playing();
            state.pitch_bend = processor.current_pitch_bend();
            state.base_note = processor.base_note.as_u8();
        }

        // ~50 Hz sensor update rate (20ms)
        FreeRtos::delay_ms(20);
    }

    Ok(())
}

// --- Thread 3: Comms Thread ---

/// Comms loop (USB-MIDI disabled for UI testing).
async fn comms_loop(
    _sensor_state: Arc<Mutex<SensorState>>,
    stop_rx: &mut mpsc::Receiver<()>,
) -> Result<(), FirmwareError> {
    log::info!("[Comms Thread] Starting communications loop...");

    loop {
        // Check for stop signal (non-blocking)
        if stop_rx.try_recv().is_ok() {
            log::info!("[Comms Thread] Stop signal received");
            break;
        }

        // ~50 Hz comms update rate
        FreeRtos::delay_ms(20);
    }

    Ok(())
}

// --- Slave mode: Autonomous demo & telemetry visualizer ---

async fn run_slave(is_demo: bool) -> Result<(), FirmwareError> {
    log::info!(
        "=== Starting in SLAVE mode (Strobe Tuner Display) [demo_mode = {}] ===",
        is_demo
    );

    // Initialize LCD display (Elecrow CrowPanel 5.0" RGB LCD)
    log::info!("[Slave] Initializing LCD display...");
    if !platform::init_lcd() {
        log::warn!("LCD initialization failed, UI will not display");
    } else {
        log::info!("[Slave] LCD display initialized");
    }

    // Initialize Slint platform
    log::info!("[Slave] Initializing Slint platform...");
    if let Err(e) = platform::init_slint_platform() {
        log::error!("[Slave] Slint platform init failed: {:?}", e);
        return Err(FirmwareError {
            step: "Slint platform",
            source: ErrorSource::Slint("Slint platform init failed"),
        });
    }
    log::info!("[Slave] Slint platform initialized");

    // Create the Slave UI (Strobe Tuner Window)
    let window = Box::new(SlaveWindow::new().map_err(|_| FirmwareError {
        step: "Slint window creation",
        source: ErrorSource::Esp(EspError::from_infallible::<{ esp_idf_sys::ESP_FAIL }>()),
    })?);

    // Initial default state
    window.set_is_active(false);
    window.set_note_name("--".into());
    window.set_octave("".into());
    window.set_cents(0.0);
    window.set_volume(0.0);
    window.set_expression(0.0);
    window.set_frequency("-- Hz".into());
    window.set_transpose("+0".into());

    // Show the window
    window.show().map_err(|_| FirmwareError {
        step: "Slint window show",
        source: ErrorSource::Esp(EspError::from_infallible::<{ esp_idf_sys::ESP_FAIL }>()),
    })?;

    // Render initial frame to LCD while backlight is still OFF
    window.window().request_redraw();
    platform::flush_to_lcd();

    // Turn ON backlight now that the first frame is painted (zero startup flash!)
    platform::set_backlight(true);
    log::info!("[Slave] Backlight enabled (initial UI painted, zero flash)");

    if is_demo {
        log::info!("[Slave] Running autonomous demo/test animation loop...");
        run_slave_demo_loop(&window).await;
    } else {
        log::info!("[Slave] Running UART telemetry listener loop...");
        run_slave_telemetry_loop(&window).await;
    }

    Ok(())
}

/// Autonomous test/demo animation loop for the Strobe Tuner screen.
async fn run_slave_demo_loop(window: &SlaveWindow) {
    let notes: [(&str, &str, f32); 8] = [
        ("A", "4", 440.0),
        ("B", "4", 493.88),
        ("C", "5", 523.25),
        ("D", "5", 587.33),
        ("E", "6", 659.25),
        ("D", "5", 587.33),
        ("C", "5", 523.25),
        ("B", "4", 493.88),
    ];
    let mut step: u32 = 0;

    loop {
        // Change note every ~150 frames (~3 seconds at 50fps)
        let note_idx = ((step / 150) as usize) % notes.len();
        let (n_name, n_octave, base_freq) = notes[note_idx];

        // Smooth pitch detuning sweeping between -35.0 and +35.0 cents
        let angle = (step as f32) * 0.05;
        let cents = 32.0 * angle.sin();

        // Volume and expression breathing animations
        let volume = 0.65 + 0.30 * ((step as f32) * 0.03).cos();
        let expression = 0.50 + 0.30 * ((step as f32) * 0.04).sin();

        // Exact frequency calculation in Hz
        let freq = base_freq * (2.0f32).powf(cents / 1200.0);

        window.set_is_active(true);
        window.set_note_name(n_name.into());
        window.set_octave(n_octave.into());
        window.set_cents(cents);
        window.set_volume(volume);
        window.set_expression(expression);
        window.set_frequency(format!("{:.1} Hz", freq).into());

        // Request a redraw to ensure the buffer is refreshed every frame
        window.window().request_redraw();

        if platform::has_active_window() {
            slint::platform::update_timers_and_animations();
        }

        let gauge_params = gauge_renderer::StrobeGaugeParams {
            cents,
            is_active: true,
        };

        let rendered = platform::flush_to_lcd_with_overlay(|buf, stride, h| {
            gauge_renderer::draw_strobe_gauge(buf, stride, h, &gauge_params);
        });

        if !rendered {
            FreeRtos::delay_ms(5);
        } else {
            FreeRtos::delay_ms(20);
        }

        step = step.wrapping_add(1);
    }
}

/// Normal Slave mode: listens for UART telemetry packets from Master.
async fn run_slave_telemetry_loop(window: &SlaveWindow) {
    #[derive(Default, Clone, Copy)]
    struct SharedTelemetry {
        cents: f32,
        is_active: bool,
        midi_note: u8,
        volume: f32,
        expression: f32,
        updated: bool,
    }

    let telemetry_state = Arc::new(Mutex::new(SharedTelemetry {
        cents: 0.0,
        is_active: false,
        midi_note: 69, // A4
        volume: 0.0,
        expression: 0.0,
        updated: true,
    }));
    let telemetry_clone = Arc::clone(&telemetry_state);

    // Initialize UART1 receiver (GPIO 17 TX, GPIO 18 RX)
    if let Ok(peripherals) = Peripherals::take() {
        let pins = peripherals.pins;
        let uart_config = esp_idf_hal::uart::UartConfig::new().baudrate(115200.into());
        match esp_idf_hal::uart::AsyncUartDriver::new(
            peripherals.uart1,
            pins.gpio17, // TX
            pins.gpio18, // RX
            Option::<esp_idf_hal::gpio::AnyIOPin>::None,
            Option::<esp_idf_hal::gpio::AnyIOPin>::None,
            &uart_config,
        ) {
            Ok(uart) => {
                log::info!("[Slave Telemetry] UART1 listening on TX=GPIO17, RX=GPIO18 at 115200 baud");
                std::thread::spawn(move || {
                    esp_idf_hal::task::block_on(async move {
                        let mut cobs_buf = [0u8; MAX_MESSAGE_SIZE];
                        let mut decode_buf = [0u8; MAX_MESSAGE_SIZE];
                        loop {
                            let mut byte = [0u8; 1];
                            let mut idx = 0;
                            loop {
                                if uart.read(&mut byte).await.is_ok() {
                                    if byte[0] == 0x00 {
                                        break;
                                    }
                                    if idx < cobs_buf.len() {
                                        cobs_buf[idx] = byte[0];
                                        idx += 1;
                                    }
                                } else {
                                    break;
                                }
                            }

                            if idx > 0 {
                                if let Ok(len) = cobs::decode(&cobs_buf[..idx], &mut decode_buf) {
                                    if let Ok(msg) = postcard::from_bytes::<MasterMessage>(&decode_buf[..len]) {
                                        if let MasterMessage::Telemetry(tuner_state) = msg {
                                            let mut state = telemetry_clone.lock().unwrap_or_else(|e| e.into_inner());
                                            match tuner_state {
                                                TunerState::On { midi_note, pitch_bend_cents } => {
                                                    state.is_active = true;
                                                    state.midi_note = midi_note;
                                                    state.cents = pitch_bend_cents as f32;
                                                    state.updated = true;
                                                }
                                                TunerState::Off => {
                                                    state.is_active = false;
                                                    state.updated = true;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    });
                });
            }
            Err(e) => {
                log::warn!("[Slave Telemetry] Could not initialize UART1: {:?}", e);
            }
        }
    } else {
        log::warn!("[Slave Telemetry] Peripherals already taken; UART receiver thread skipped");
    }

    const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

    loop {
        let (cents, is_active, midi_note, volume, expression, updated) = {
            let mut state = telemetry_state.lock().unwrap_or_else(|e| e.into_inner());
            let u = state.updated;
            state.updated = false;
            (state.cents, state.is_active, state.midi_note, state.volume, state.expression, u)
        };

        if updated {
            window.set_is_active(is_active);
            if is_active {
                let note_name = NOTE_NAMES[(midi_note % 12) as usize];
                let octave = (midi_note / 12) as i8 - 1;
                let freq = 440.0 * (2.0f32).powf((midi_note as f32 - 69.0 + cents / 100.0) / 12.0);

                window.set_note_name(note_name.into());
                window.set_octave(format!("{}", octave).into());
                window.set_cents(cents);
                window.set_volume(volume);
                window.set_expression(expression);
                window.set_frequency(format!("{:.1} Hz", freq).into());
            } else {
                window.set_note_name("--".into());
                window.set_octave("".into());
                window.set_frequency("-- Hz".into());
            }
            window.window().request_redraw();
        }

        if platform::has_active_window() {
            slint::platform::update_timers_and_animations();
        }

        let gauge_params = gauge_renderer::StrobeGaugeParams {
            cents,
            is_active,
        };

        let rendered = platform::flush_to_lcd_with_overlay(|buf, stride, h| {
            gauge_renderer::draw_strobe_gauge(buf, stride, h, &gauge_params);
        });

        if !rendered {
            FreeRtos::delay_ms(5);
        } else {
            FreeRtos::delay_ms(20);
        }
    }
}

// --- Main entry point ---

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!(
        "Starting ToF Theremin Firmware (BOARD_MODE = {:?})",
        BOARD_MODE
    );

    let app_future = async {
        match BOARD_MODE {
            ThereminMode::SlaveDemo => run_slave(true).await,
            ThereminMode::SlaveBoard => run_slave(false).await,
            _ => run_master_3_threads().await,
        }
    };

    if let Err(e) = block_on(app_future) {
        log::error!("Fatal: {e}. Restarting in {FATAL_RESTART_DELAY_MS} ms...");
        FreeRtos::delay_ms(FATAL_RESTART_DELAY_MS);
        esp_idf_hal::reset::restart();
    }
}

/// Master mode with 3 threads: UI, Sensors, Comms
async fn run_master_3_threads() -> Result<(), FirmwareError> {
    log::info!("=== Master Board with 3-Thread Architecture ===");

    // Initialize peripherals
    let peripherals = Peripherals::take().context("peripherals take")?;
    let _pins = peripherals.pins;

    // I2C bus for sensors (currently stubbed - using simulated sensor values)
    // We still need to take the peripherals for future sensor initialization
    log::info!("Peripherals taken (I2C stubbed for now)");
    let _ = peripherals;

    // Shared sensor state
    let sensor_state: Arc<Mutex<SensorState>> = Arc::new(Mutex::new(SensorState::default()));

    // Create stop channels for graceful shutdown
    let (_sensor_stop_tx, mut sensor_stop_rx) = mpsc::channel::<()>();
    let (_comms_stop_tx, mut comms_stop_rx) = mpsc::channel::<()>();

    // Thread 2: Sensor Thread (pinned to Core 1)
    log::info!("Starting sensor thread pinned to Core 1 (APP_CPU)...");
    let sensor_state_clone = Arc::clone(&sensor_state);
    esp_idf_hal::task::thread::ThreadSpawnConfiguration {
        name: Some(c"sensor_loop"),
        stack_size: 8192,
        priority: 15,
        pin_to_core: Some(esp_idf_hal::cpu::Core::Core1),
        ..Default::default()
    }
    .set()
    .ok();
    let _sensor_handle = std::thread::spawn(move || {
        esp_idf_hal::task::block_on(async move {
            if let Err(e) = sensor_loop(sensor_state_clone, &mut sensor_stop_rx).await {
                log::error!("[Sensor Thread] Error: {}", e);
            }
        });
    });

    // Thread 3: Comms Thread (pinned to Core 1)
    log::info!("Starting comms thread pinned to Core 1 (APP_CPU)...");
    let sensor_state_clone = Arc::clone(&sensor_state);
    esp_idf_hal::task::thread::ThreadSpawnConfiguration {
        name: Some(c"comms_loop"),
        stack_size: 8192,
        priority: 12,
        pin_to_core: Some(esp_idf_hal::cpu::Core::Core1),
        ..Default::default()
    }
    .set()
    .ok();
    let _comms_handle = std::thread::spawn(move || {
        esp_idf_hal::task::block_on(async move {
            if let Err(e) = comms_loop(sensor_state_clone, &mut comms_stop_rx).await {
                log::error!("[Comms Thread] Error: {}", e);
            }
        });
    });

    // Reset thread spawn configuration to defaults
    esp_idf_hal::task::thread::ThreadSpawnConfiguration::default().set().ok();

    log::info!("Sensor and comms threads started. Running UI on main task...");

    // Run UI loop directly on main task (uses main task stack, saves ~40KB DRAM)
    if let Err(e) = ui_thread(sensor_state) {
        log::error!("[UI Thread] Fatal error: {}", e);
    }

    loop {
        FreeRtos::delay_ms(1000);
    }
}
