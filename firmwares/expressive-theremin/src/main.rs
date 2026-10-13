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
use esp_idf_hal::units::FromValueType;
use esp_idf_svc::sys::EspError;
use expressive_core::processor::{
    ConfigMessage, ControlProcessor, FilterSpeed, ProcessorMode, RootNote, Scale,
};
use expressive_core::{Channel, InstrumentEvent, SensorEvent, U7};
use theremin::TofThereminProcessor;

// Slint UI is always compiled.
slint::include_modules!();
#[allow(dead_code)]
type ThereminUi = MainWindow;

pub mod concurrency;
pub mod gauge_renderer;
pub mod platform;
pub mod protocol;
pub mod touch;

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
    log::info!("[UI Thread] Creating MainWindow...");
    let ui = match MainWindow::new() {
        Ok(ui) => {
            log::info!("[UI Thread] MainWindow created");
            ui
        }
        Err(e) => {
            log::error!("[UI Thread] MainWindow::new() failed: {:?}", e);
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

    // Initialize Touchscreen (GT911 via PCA9557 I2C) & UART
    let mut i2c_opt = None;
    let mut gt911_opt = None;
    let mut uart_opt = None;

    if let Ok(peripherals) = Peripherals::take() {
        let pins = peripherals.pins;

        // 1. Initialize I2C0 for Touch Screen (SDA=GPIO19, SCL=GPIO20)
        let i2c_config = esp_idf_hal::i2c::I2cConfig::new().baudrate(400.kHz().into());
        match esp_idf_hal::i2c::I2cDriver::new(peripherals.i2c0, pins.gpio19, pins.gpio20, &i2c_config) {
            Ok(mut driver) => {
                log::info!("[Slave] I2C0 driver initialized on SDA=GPIO19, SCL=GPIO20");
                // Pre-reset I2C scan
                touch::scan_i2c_bus(&mut driver);
                // Hardware reset of GT911 via PCA9557
                if let Err(e) = touch::reset_gt911_via_pca9557(&mut driver) {
                    log::warn!("[Slave] PCA9557 reset failed: {:?}", e);
                }
                // Post-reset I2C scan (GT911 should now be responsive at 0x5D)
                touch::scan_i2c_bus(&mut driver);
                // Initialize GT911 driver
                gt911_opt = touch::Gt911Driver::init(&mut driver).ok();
                i2c_opt = Some(driver);
            }
            Err(e) => {
                log::warn!("[Slave] Failed to initialize I2C0 for touch: {:?}", e);
            }
        }

        // 2. Initialize UART1 if in normal Slave telemetry mode
        if !is_demo {
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
                    uart_opt = Some(uart);
                }
                Err(e) => {
                    log::warn!("[Slave] Failed to initialize UART1: {:?}", e);
                }
            }
        }
    } else {
        log::warn!("[Slave] Peripherals::take() failed or already taken");
    }

    // Create the Slint UI (Unified MainWindow with Master & Slave views)
    let window = Box::new(MainWindow::new().map_err(|_| FirmwareError {
        step: "Slint window creation",
        source: ErrorSource::Esp(EspError::from_infallible::<{ esp_idf_sys::ESP_FAIL }>()),
    })?);

    // Initial default state (Slave View)
    window.set_is_active(false);
    window.set_note_name("--".into());
    window.set_octave("".into());
    window.set_cents(0.0);
    window.set_volume(0.0);
    window.set_expression(0.0);
    window.set_frequency("-- Hz".into());
    window.set_transpose("+0".into());
    window.set_is_master_view(true);

    // Initial Master Settings
    window.set_current_root("C".into());
    window.set_current_scale("Chromatic".into());
    window.set_current_octave(4);
    window.set_octave_range(2);
    window.set_pitch_quantization(50);
    window.set_filter_speed("Fast".into());
    window.set_midi_mode("Standard".into());
    window.set_pb_range(2);

    // Initialize lock-free SPSC queue for UI -> Audio Processor config messages (safe static allocation)
    let queue: &'static mut heapless::spsc::Queue<ConfigMessage, { concurrency::CONFIG_QUEUE_CAPACITY }> =
        Box::leak(Box::new(heapless::spsc::Queue::new()));
    let (config_tx, config_rx) = concurrency::create_config_channel(queue);
    let config_tx = Arc::new(Mutex::new(config_tx));

    // Wire up Master UI callbacks to send ConfigMessages over the lock-free queue
    let tx_oct = Arc::clone(&config_tx);
    window.on_octave_changed(move |octave| {
        log::info!("[UI Master] Base Octave changed: {}", octave);
        let _ = tx_oct.lock().unwrap().enqueue(ConfigMessage::SetBaseOctave(octave));
    });

    let tx_scale = Arc::clone(&config_tx);
    window.on_scale_changed(move |scale| {
        log::info!("[UI Master] Scale changed: {}", scale);
        let s = match scale.as_str() {
            "Chromatic" => Scale::Chromatic,
            "Major" => Scale::Major,
            "Minor" => Scale::Minor,
            "Penta" => Scale::Pentatonic,
            _ => Scale::Chromatic,
        };
        let _ = tx_scale.lock().unwrap().enqueue(ConfigMessage::SetScale(s));
    });

    let tx_root = Arc::clone(&config_tx);
    window.on_root_changed(move |root| {
        log::info!("[UI Master] Root Note changed: {}", root);
        let r = match root.as_str() {
            "C" => RootNote::C,
            "C#" => RootNote::Cs,
            "D" => RootNote::D,
            "D#" => RootNote::Ds,
            "E" => RootNote::E,
            "F" => RootNote::F,
            "F#" => RootNote::Fs,
            "G" => RootNote::G,
            "G#" => RootNote::Gs,
            "A" => RootNote::A,
            "A#" => RootNote::As,
            "B" => RootNote::B,
            _ => RootNote::C,
        };
        let _ = tx_root.lock().unwrap().enqueue(ConfigMessage::SetRootNote(r));
    });

    let tx_range = Arc::clone(&config_tx);
    window.on_range_changed(move |r| {
        log::info!("[UI Master] Octave Range changed: {}", r);
        let _ = tx_range.lock().unwrap().enqueue(ConfigMessage::SetOctaveRange(r));
    });

    let tx_pb = Arc::clone(&config_tx);
    let win_pb = window.as_weak();
    window.on_pb_range_changed(move |r| {
        log::info!("[UI Master] PB Range changed: {}", r);
        if let Some(win) = win_pb.upgrade() {
            let mode_str = win.get_midi_mode();
            let proc_mode = if mode_str == "MPE" {
                ProcessorMode::Mpe {
                    manager_channel: Channel::new(0),
                    pitch_bend_range: r as u8,
                }
            } else {
                ProcessorMode::Standard {
                    channel: Channel::new(0),
                    pitch_bend_range: r as u8,
                }
            };
            let _ = tx_pb.lock().unwrap().enqueue(ConfigMessage::SetProcessorMode(proc_mode));
        }
    });

    let tx_mode = Arc::clone(&config_tx);
    let win_mode = window.as_weak();
    window.on_midi_mode_changed(move |m| {
        log::info!("[UI Master] MIDI Mode changed: {}", m);
        if let Some(win) = win_mode.upgrade() {
            let r = win.get_pb_range();
            let target_mode = if m == "MPE" {
                ProcessorMode::Mpe {
                    manager_channel: Channel::new(0),
                    pitch_bend_range: r as u8,
                }
            } else {
                ProcessorMode::Standard {
                    channel: Channel::new(0),
                    pitch_bend_range: r as u8,
                }
            };
            let _ = tx_mode.lock().unwrap().enqueue(ConfigMessage::SetProcessorMode(target_mode));
        }
    });

    let tx_quant = Arc::clone(&config_tx);
    window.on_quantization_changed(move |q| {
        log::info!("[UI Master] Quantization changed: {}%", q);
        let _ = tx_quant.lock().unwrap().enqueue(ConfigMessage::SetPitchQuantization(q));
    });

    let tx_filter = Arc::clone(&config_tx);
    window.on_filter_changed(move |f| {
        log::info!("[UI Master] Filter changed: {}", f);
        let s = match f.as_str() {
            "Slow" => FilterSpeed::Slow,
            "Med" => FilterSpeed::Med,
            "Fast" => FilterSpeed::Fast,
            _ => FilterSpeed::Fast,
        };
        let _ = tx_filter.lock().unwrap().enqueue(ConfigMessage::SetFilterSpeed(s));
    });

    let win_switch = window.as_weak();
    let last_switch = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    window.on_view_switched(move |is_master| {
        let now_ms = unsafe { (esp_idf_sys::esp_timer_get_time() / 1000) as u32 };
        let prev_ms = last_switch.load(std::sync::atomic::Ordering::Relaxed);
        if now_ms.wrapping_sub(prev_ms) < 250 {
            log::info!("[UI] Ignoring rapid view switch (debounce 250ms)");
            return;
        }
        last_switch.store(now_ms, std::sync::atomic::Ordering::Relaxed);
        log::info!("[UI] Screen view toggled: is_master = {}", is_master);
        if let Some(w) = win_switch.upgrade() {
            w.window().request_redraw();
        }
    });

    // Store weak reference for platform updates
    let window_weak = window.as_weak();
    {
        let mut weak_guard = platform::G_WINDOW_WEAK.lock().unwrap_or_else(|e| e.into_inner());
        *weak_guard = Some(window_weak.clone());
    }

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
    log::info!("[UI] Backlight enabled (initial UI painted, zero flash)");

    if is_demo {
        log::info!("[UI] Running autonomous demo/test animation loop with touch...");
        run_slave_demo_loop(&window, i2c_opt.as_mut(), gt911_opt.as_ref(), config_rx).await;
    } else {
        log::info!("[UI] Running UART telemetry listener loop with touch...");
        run_slave_telemetry_loop(&window, i2c_opt.as_mut(), gt911_opt.as_ref(), uart_opt, config_rx).await;
    }

    Ok(())
}

/// Autonomous test/demo animation loop with live touch interaction and lock-free decoupled processor integration.
async fn run_slave_demo_loop(
    window: &MainWindow,
    mut i2c: Option<&mut esp_idf_hal::i2c::I2cDriver<'static>>,
    gt911: Option<&touch::Gt911Driver>,
    mut config_rx: heapless::spsc::Consumer<'static, ConfigMessage>,
) {
    let mut processor = TofThereminProcessor::default();
    let mut step: u32 = 0;
    let mut last_touch: Option<(u16, u16)> = None;
    let snapshot = concurrency::AtomicTunerSnapshot::new();
    let mut prev_is_master = window.get_is_master_view();
    let mut force_redraw_frames: u8 = 0;

    loop {
        // 1. Drain pending UI configuration messages lock-free (zero mutex contention!)
        while let Some(msg) = config_rx.dequeue() {
            processor.handle_config(msg);
        }

        // 2. Poll Touchscreen and dispatch Slint Pointer events
        if let (Some(gt), Some(ref mut bus)) = (gt911, i2c.as_mut()) {
            match gt.poll_touch(bus) {
                touch::TouchEvent::Pressed { x, y } => {
                    match last_touch {
                        None => {
                            log::info!("[Touch] PointerPressed at ({}, {})", x, y);
                            platform::dispatch_pointer_event(slint::platform::WindowEvent::PointerPressed {
                                position: slint::LogicalPosition::new(x as f32, y as f32),
                                button: slint::platform::PointerEventButton::Left,
                            });
                            last_touch = Some((x, y));
                        }
                        Some((prev_x, prev_y)) => {
                            if (x as i32 - prev_x as i32).abs() > 1 || (y as i32 - prev_y as i32).abs() > 1 {
                                platform::dispatch_pointer_event(slint::platform::WindowEvent::PointerMoved {
                                    position: slint::LogicalPosition::new(x as f32, y as f32),
                                });
                                last_touch = Some((x, y));
                            }
                        }
                    }
                }
                touch::TouchEvent::Released => {
                    if let Some((prev_x, prev_y)) = last_touch {
                        log::info!("[Touch] PointerReleased at ({}, {})", prev_x, prev_y);
                        platform::dispatch_pointer_event(slint::platform::WindowEvent::PointerReleased {
                            position: slint::LogicalPosition::new(prev_x as f32, prev_y as f32),
                            button: slint::platform::PointerEventButton::Left,
                        });
                        last_touch = None;
                    }
                }
                touch::TouchEvent::NoUpdate => {}
            }
        }

        // 3. Sensor simulation loop (Direct execution on Core 1 without Mutex)
        let angle = (step as f32) * 0.015;
        let sim_pitch_z = (525.0 + 380.0 * angle.sin()) as u16; // 145mm .. 905mm
        let sim_vol_z = (200.0 + 120.0 * ((step as f32) * 0.025).cos()) as u16; // 80mm .. 320mm

        let mut events = [InstrumentEvent::NoteOff {
            channel: Channel::new(0),
            note: U7::new(0),
            velocity: U7::new(0),
        }; 4];
        struct NullTelemetry;
        impl expressive_core::telemetry::TelemetryHandler for NullTelemetry {
            fn handle(&mut self, _event: expressive_core::telemetry::TelemetryEvent) {}
        }
        let _ = processor.process_event(
            SensorEvent::Position3D { id: 0, x: 0, y: 0, z: sim_pitch_z },
            &mut events,
            &mut NullTelemetry,
        );
        let _ = processor.process_event(
            SensorEvent::Position3D { id: 1, x: 0, y: 0, z: sim_vol_z },
            &mut events,
            &mut NullTelemetry,
        );

        let note_u8 = processor.base_note.as_u8();
        let bend_u16 = processor.current_pitch_bend();
        let pb_range = processor.pitch_bend_range_semitones as f32;
        let cents = ((bend_u16 as f32 - 8192.0) / 8191.0) * pb_range * 100.0;
        let vol_norm = processor.current_volume() as f32 / 127.0;
        let active = processor.is_playing() && vol_norm > 0.05;

        // Musical tuner calculation: find nearest semitone and detune cents in [-50, +50]
        let total_semis = note_u8 as f32 + (cents / 100.0);
        let display_note = total_semis.round() as i32;
        let display_cents = ((total_semis - display_note as f32) * 100.0).clamp(-50.0, 50.0);

        const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
        let note_idx = ((display_note % 12 + 12) % 12) as usize;
        let note_name = NOTE_NAMES[note_idx];
        let octave = (display_note / 12) - 1;
        let freq = 440.0 * (2.0f32).powf((display_note as f32 - 69.0 + display_cents / 100.0) / 12.0);
        let expression = 0.50 + 0.30 * ((step as f32) * 0.04).sin();

        // 4. Publish lock-free snapshot for the UI thread
        snapshot.write(concurrency::TunerSnapshot {
            note_name,
            octave: octave as i8,
            cents: display_cents,
            volume: vol_norm,
            expression,
            freq_hz: freq,
            is_active: active,
        });

        // 5. Update Slint UI from snapshot if Tuner view is visible
        let is_master = window.get_is_master_view();
        if is_master != prev_is_master {
            log::info!("[Loop] View switched: {} -> {}, forcing 2 redraw frames", prev_is_master, is_master);
            prev_is_master = is_master;
            force_redraw_frames = 2;
        }

        if force_redraw_frames > 0 {
            window.window().request_redraw();
            force_redraw_frames -= 1;
        }

        let current_snap = snapshot.read();

        if !is_master {
            window.set_is_active(current_snap.is_active);
            window.set_note_name(current_snap.note_name.into());
            window.set_octave(format!("{}", current_snap.octave).into());
            window.set_cents(current_snap.cents);
            window.set_volume(current_snap.volume);
            window.set_expression(current_snap.expression);
            window.set_frequency(format!("{:.1} Hz", current_snap.freq_hz).into());

            window.window().request_redraw();
        }

        if platform::has_active_window() {
            slint::platform::update_timers_and_animations();
        }

        let t_render_start = unsafe { esp_idf_sys::esp_timer_get_time() };
        let rendered = if is_master {
            platform::flush_to_lcd()
        } else {
            let gauge_params = gauge_renderer::StrobeGaugeParams {
                cents: current_snap.cents,
                is_active: current_snap.is_active,
            };
            platform::flush_to_lcd_with_overlay(|buf, stride, h| {
                gauge_renderer::draw_strobe_gauge(buf, stride, h, &gauge_params);
            })
        };
        let t_render_dur = unsafe { esp_idf_sys::esp_timer_get_time() } - t_render_start;
        if rendered && t_render_dur > 20_000 {
            log::info!("[Perf] Render (is_master={}) took {} ms", is_master, t_render_dur / 1000);
        }

        if !rendered {
            FreeRtos::delay_ms(2);
        } else {
            FreeRtos::delay_ms(1);
        }

        step = step.wrapping_add(1);
    }
}

/// Normal Slave mode: listens for UART telemetry packets from Master with live touch support.
async fn run_slave_telemetry_loop(
    window: &MainWindow,
    mut i2c: Option<&mut esp_idf_hal::i2c::I2cDriver<'static>>,
    gt911: Option<&touch::Gt911Driver>,
    uart_driver: Option<esp_idf_hal::uart::AsyncUartDriver<'static, esp_idf_hal::uart::UartDriver<'static>>>,
    mut _config_rx: heapless::spsc::Consumer<'static, ConfigMessage>,
) {
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
    let mut prev_is_master = window.get_is_master_view();
    let mut force_redraw_frames: u8 = 0;

    if let Some(uart) = uart_driver {
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
                            if let Ok(MasterMessage::Telemetry(tuner_state)) = postcard::from_bytes::<MasterMessage>(&decode_buf[..len]) {
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
            });
        });
    }

    const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let mut last_touch: Option<(u16, u16)> = None;

    loop {
        // Poll Touchscreen and dispatch Slint Pointer events
        if let (Some(gt), Some(ref mut bus)) = (gt911, i2c.as_mut()) {
            match gt.poll_touch(bus) {
                touch::TouchEvent::Pressed { x, y } => {
                    match last_touch {
                        None => {
                            log::info!("[Touch] PointerPressed at ({}, {})", x, y);
                            platform::dispatch_pointer_event(slint::platform::WindowEvent::PointerPressed {
                                position: slint::LogicalPosition::new(x as f32, y as f32),
                                button: slint::platform::PointerEventButton::Left,
                            });
                            last_touch = Some((x, y));
                        }
                        Some((prev_x, prev_y)) => {
                            if (x as i32 - prev_x as i32).abs() > 1 || (y as i32 - prev_y as i32).abs() > 1 {
                                platform::dispatch_pointer_event(slint::platform::WindowEvent::PointerMoved {
                                    position: slint::LogicalPosition::new(x as f32, y as f32),
                                });
                                last_touch = Some((x, y));
                            }
                        }
                    }
                }
                touch::TouchEvent::Released => {
                    if let Some((prev_x, prev_y)) = last_touch {
                        log::info!("[Touch] PointerReleased at ({}, {})", prev_x, prev_y);
                        platform::dispatch_pointer_event(slint::platform::WindowEvent::PointerReleased {
                            position: slint::LogicalPosition::new(prev_x as f32, prev_y as f32),
                            button: slint::platform::PointerEventButton::Left,
                        });
                        last_touch = None;
                    }
                }
                touch::TouchEvent::NoUpdate => {}
            }
        }

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

        let is_master = window.get_is_master_view();
        if is_master != prev_is_master {
            log::info!("[Telemetry Loop] View switched: {} -> {}, forcing 2 redraw frames", prev_is_master, is_master);
            prev_is_master = is_master;
            force_redraw_frames = 2;
        }

        if force_redraw_frames > 0 {
            window.window().request_redraw();
            force_redraw_frames -= 1;
        }

        let rendered = if is_master {
            platform::flush_to_lcd()
        } else {
            let gauge_params = gauge_renderer::StrobeGaugeParams {
                cents,
                is_active,
            };
            platform::flush_to_lcd_with_overlay(|buf, stride, h| {
                gauge_renderer::draw_strobe_gauge(buf, stride, h, &gauge_params);
            })
        };

        if !rendered {
            FreeRtos::delay_ms(2);
        } else {
            FreeRtos::delay_ms(1);
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
