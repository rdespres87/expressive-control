use core::cell::RefCell;
use core::fmt;

use embedded_hal_async::i2c::I2c as _;
use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::gpio::{PinDriver, Pull};
use esp_idf_hal::i2c::{I2cConfig, I2cDriver, I2cError};
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::task::block_on;
use esp_idf_hal::units::FromValueType;
use esp_idf_svc::sys::EspError;
use expressive_core::midi::ToMidi;
use expressive_core::processor::ControlProcessor;
use expressive_core::{InstrumentEvent, SensorEvent};
use theremin::TofThereminProcessor;
use esp_idf_hal::uart::{AsyncUartDriver, UartConfig};
use crate::protocol::{MasterMessage, MAX_MESSAGE_SIZE};

pub mod protocol;

/// Defines the hardware architecture the firmware is built for.
#[derive(Debug, PartialEq, Eq)]
pub enum ThereminMode {
    /// Legacy mode: One board reads both sensors and handles everything.
    SingleBoard,
    /// Dual-board: This board is the "Master". It reads BOTH Volume and Pitch ToFs,
    /// ensures perfect synchronization, outputs USB MIDI, and sends UI telemetry via UART.
    MasterBoard,
    /// Dual-board: This board is the "Slave" (Pitch display). It reads no sensors,
    /// but receives UI telemetry via UART to render the Strobe Tuner.
    SlaveBoard,
}

/// Change this constant to build for the desired board.
/// (In the future, we could read a GPIO pin at boot to set this dynamically).
const BOARD_MODE: ThereminMode = ThereminMode::SingleBoard;

/// Default 7-bit I2C address of a VL53L4CD after power-up.
const VL53L4CD_DEFAULT_ADDR: u8 = 0x29;
/// 7-bit I2C address assigned to the volume sensor during the sequential boot.
const VOLUME_SENSOR_ADDR: u8 = 0x2A;
/// 16-bit index of the VL53L4CD `I2C_SLAVE__DEVICE_ADDRESS` register.
const REG_I2C_SLAVE_DEVICE_ADDRESS: [u8; 2] = [0x00, 0x01];
/// Ranging timing budget (20 ms => ~50 Hz). 50Hz provides a more stable, less noisy signal than 100Hz for the Theremin.
const TIMING_BUDGET_MS: u32 = 20;
/// 0 selects continuous ranging mode.
const INTER_MEASUREMENT_MS: u32 = 0;
/// Delay before rebooting after a fatal error, so logs can be read on the console.
const FATAL_RESTART_DELAY_MS: u32 = 5_000;
/// USB-MIDI virtual cable used for all outgoing messages.
const USB_MIDI_CABLE: u8 = 0;

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

/// Mirror of the ESP-IDF `tinyusb_config_t` struct (not generated by `esp-idf-sys`).
// Fields are only read on the C side, through FFI.
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

extern "C" {
    fn tinyusb_driver_install(config: *const TinyUsbConfig) -> i32;
    fn tud_midi_n_stream_write(itf: u8, cable_num: u8, buffer: *const u8, bufsize: u32) -> u32;
}

fn install_usb_midi() -> Result<(), FirmwareError> {
    let cfg = TinyUsbConfig {
        _device_descriptor: core::ptr::null(),
        _string_descriptor: core::ptr::null_mut(),
        _string_descriptor_count: 0,
        _external_phy: false,
        _configuration_descriptor: core::ptr::null(),
        _self_powered: false,
        _vbus_monitor_io: 0,
    };
    // SAFETY: `cfg` is a valid, fully initialised config that outlives the call.
    let ret = unsafe { tinyusb_driver_install(&cfg) };
    EspError::convert(ret).context("TinyUSB driver install")
}

/// Sends one MIDI message over USB. A partial write is logged, never fatal.
fn send_usb_midi(bytes: &[u8]) {
    let Ok(len) = u32::try_from(bytes.len()) else {
        log::warn!("USB-MIDI TX skipped: message too long ({} bytes)", bytes.len());
        return;
    };
    // SAFETY: `bytes` is a valid slice of `len` bytes for the duration of the call.
    let written = unsafe { tud_midi_n_stream_write(0, USB_MIDI_CABLE, bytes.as_ptr(), len) };
    if written != len {
        log::warn!("USB-MIDI TX incomplete: {written}/{len} bytes ({bytes:02X?})");
    } else {
        log::info!("USB-MIDI TX: {bytes:02X?}");
    }
}

// --- Sensors ---

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
async fn init_and_readdress(bus: &I2cBus, new_addr: u8) -> Result<Sensor<'_>, FirmwareError> {
    let mut sensor = new_sensor(bus, VL53L4CD_DEFAULT_ADDR);
    sensor.init().await.context("sensor init (default address)")?;

    let [reg_hi, reg_lo] = REG_I2C_SLAVE_DEVICE_ADDRESS;
    AsyncI2cWrapper(bus)
        .write(VL53L4CD_DEFAULT_ADDR, &[reg_hi, reg_lo, new_addr])
        .await
        .context("sensor address change")?;

    Ok(new_sensor(bus, new_addr))
}

async fn start_ranging(sensor: &mut Sensor<'_>, name: &'static str) -> Result<(), FirmwareError> {
    sensor
        .set_range_timing(TIMING_BUDGET_MS, INTER_MEASUREMENT_MS)
        .await
        .context(name)?;
    sensor.start_ranging().await.context(name)
}

/// Reads the measurement signalled by the interrupt line, then re-arms the interrupt.
/// The interrupt is always cleared, even if the read failed, to avoid a stuck line.
async fn read_pending_distance(sensor: &mut Sensor<'_>) -> Result<u16, FirmwareError> {
    let measurement = sensor.read_measurement().await;
    let cleared = sensor.clear_interrupt().await;
    let measurement = measurement.context("pitch measurement read")?;
    cleared.context("pitch interrupt clear")?;
    Ok(measurement.distance)
}

async fn send_telemetry<T: esp_idf_hal::uart::Uart>(
    uart: &mut AsyncUartDriver<'_, T>,
    processor: &TofThereminProcessor,
) {
    let bend_u14 = processor.current_pitch_bend();
    let bend_cents = (((bend_u14 as f32) - 8192.0) / 8192.0 * 50.0) as i8;

    let msg = MasterMessage::Telemetry {
        midi_note: processor.base_note.as_u8(),
        pitch_bend_cents: bend_cents,
        volume: processor.current_volume(),
        is_muted: !processor.is_playing(),
    };

    let mut buf = [0u8; MAX_MESSAGE_SIZE];
    if let Ok(s_slice) = postcard::to_slice(&msg, &mut buf[..MAX_MESSAGE_SIZE - 2]) {
        let mut cobs_buf = [0u8; MAX_MESSAGE_SIZE];
        let cobs_len = cobs::encode(s_slice, &mut cobs_buf);
        cobs_buf[cobs_len] = 0x00; // terminator
        if let Err(e) = uart.write(&cobs_buf[..=cobs_len]).await {
            log::warn!("UART TX error: {:?}", e);
        }
    }
}

// --- Application ---

async fn run() -> Result<(), FirmwareError> {
    log::info!("Starting in mode: {:?}", BOARD_MODE);
    
    let peripherals = Peripherals::take().context("peripherals take")?;
    let pins = peripherals.pins;

    let i2c_config = I2cConfig::new().baudrate(400_u32.kHz().into());
    let i2c = I2cDriver::new(peripherals.i2c0, pins.gpio1, pins.gpio2, &i2c_config)
        .context("I2C driver init")?;
    let i2c_bus: I2cBus = RefCell::new(i2c);

    log::info!("Initializing TinyUSB for Native USB-MIDI Class...");
    install_usb_midi()?;

    log::info!("Initializing UART telemetry link (Master -> Slave)...");
    let uart_config = UartConfig::new().baudrate(115200.into());
    let mut uart = AsyncUartDriver::new(
        peripherals.uart1,
        pins.gpio17, // TX to Slave
        pins.gpio18, // RX from Slave
        Option::<esp_idf_hal::gpio::AnyIOPin>::None,
        Option::<esp_idf_hal::gpio::AnyIOPin>::None,
        &uart_config,
    ).context("UART init")?;

    let mut xshut_pitch = PinDriver::output(pins.gpio6).context("XSHUT pitch pin")?;
    let mut xshut_volume = PinDriver::output(pins.gpio7).context("XSHUT volume pin")?;
    let mut int_pitch =
        PinDriver::input(pins.gpio4, Pull::Floating).context("INT pitch pin")?;
    let mut int_volume = PinDriver::input(pins.gpio5, Pull::Floating).context("INT volume pin")?;

    // Sequential boot: hold both sensors in reset, then wake them one by one so
    // each can be addressed individually on the shared bus.
    xshut_pitch.set_low().context("XSHUT pitch low")?;
    xshut_volume.set_low().context("XSHUT volume low")?;
    embassy_time::Timer::after_millis(10).await;

    log::info!("Booting Volume Sensor...");
    xshut_volume.set_high().context("XSHUT volume high")?;
    embassy_time::Timer::after_millis(50).await;
    let mut sensor_vol = init_and_readdress(&i2c_bus, VOLUME_SENSOR_ADDR).await?;
    log::info!("Volume Sensor initialized at 0x{VOLUME_SENSOR_ADDR:02X}.");

    log::info!("Booting Pitch Sensor...");
    xshut_pitch.set_high().context("XSHUT pitch high")?;
    embassy_time::Timer::after_millis(50).await;
    let mut sensor_pitch = new_sensor(&i2c_bus, VL53L4CD_DEFAULT_ADDR);
    sensor_pitch.init().await.context("pitch sensor init")?;
    log::info!("Pitch Sensor initialized at 0x{VL53L4CD_DEFAULT_ADDR:02X}.");

    start_ranging(&mut sensor_pitch, "pitch sensor ranging start").await?;
    start_ranging(&mut sensor_vol, "volume sensor ranging start").await?;

    let mut processor = TofThereminProcessor::default();
    let mut out_buffer = [InstrumentEvent::NoteOn {
        channel: expressive_core::Channel::new(1),
        note: expressive_core::U7::new(0),
        velocity: expressive_core::U7::new(0),
    }; 10];

    log::info!("Entering main async interrupt loop...");
    loop {
        use embassy_futures::select::{select, Either};

        match select(int_pitch.wait_for_falling_edge(), int_volume.wait_for_falling_edge()).await {
            Either::First(res) => {
                if let Err(e) = res {
                    log::error!("Pitch interrupt wait failed: {e:?}");
                    continue;
                }
                match read_pending_distance(&mut sensor_pitch).await {
                    Ok(distance) => {
                        let event = SensorEvent::Position3D { id: 0, x: 0, y: 0, z: distance };
                        if let Ok(count) = processor.process_event(event, &mut out_buffer) {
                            for ev in out_buffer.iter().take(count) {
                                log::info!("Generated Event (Pitch): {ev:?}");
                                send_usb_midi(ev.to_midi().as_slice());
                            }
                            send_telemetry(&mut uart, &processor).await;
                        }
                    }
                    Err(e) => log::warn!("{e}"),
                }
            }
            Either::Second(res) => {
                if let Err(e) = res {
                    log::error!("Volume interrupt wait failed: {e:?}");
                    continue;
                }
                match read_pending_distance(&mut sensor_vol).await {
                    Ok(distance) => {
                        let event = SensorEvent::Position3D { id: 1, x: 0, y: 0, z: distance };
                        if let Ok(count) = processor.process_event(event, &mut out_buffer) {
                            for ev in out_buffer.iter().take(count) {
                                log::info!("Generated Event (Volume): {ev:?}");
                                send_usb_midi(ev.to_midi().as_slice());
                            }
                            send_telemetry(&mut uart, &processor).await;
                        }
                    }
                    Err(e) => log::warn!("{e}"),
                }
            }
        }
    }
}

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Starting ToF Theremin Firmware (Interrupt-driven)...");

    if let Err(e) = block_on(run()) {
        log::error!("Fatal: {e}. Restarting in {FATAL_RESTART_DELAY_MS} ms...");
        FreeRtos::delay_ms(FATAL_RESTART_DELAY_MS);
        esp_idf_hal::reset::restart();
    }
}
