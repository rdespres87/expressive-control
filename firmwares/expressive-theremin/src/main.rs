use esp_idf_hal::gpio::{PinDriver, Pull};
use esp_idf_hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::task::block_on;
use esp_idf_hal::units::FromValueType;
use expressive_core::processor::ControlProcessor;
use expressive_core::{InstrumentEvent, SensorEvent};
use theremin::TofThereminProcessor;

// Wrapper to adapt blocking I2cDriver to async I2c trait (since vl53l4cd v0.4 is async)
struct AsyncI2cWrapper<'a>(&'a core::cell::RefCell<esp_idf_hal::i2c::I2cDriver<'static>>);

impl<'a> embedded_hal_async::i2c::ErrorType for AsyncI2cWrapper<'a> {
    type Error = esp_idf_hal::i2c::I2cError;
}

impl<'a> embedded_hal_async::i2c::I2c for AsyncI2cWrapper<'a> {
    async fn read(&mut self, address: u8, read: &mut [u8]) -> Result<(), Self::Error> {
        embedded_hal::i2c::I2c::read(&mut *self.0.borrow_mut(), address, read)
    }
    async fn write(&mut self, address: u8, write: &[u8]) -> Result<(), Self::Error> {
        embedded_hal::i2c::I2c::write(&mut *self.0.borrow_mut(), address, write)
    }
    async fn write_read(&mut self, address: u8, write: &[u8], read: &mut [u8]) -> Result<(), Self::Error> {
        embedded_hal::i2c::I2c::write_read(&mut *self.0.borrow_mut(), address, write, read)
    }
    async fn transaction(&mut self, address: u8, operations: &mut [embedded_hal_async::i2c::Operation<'_>]) -> Result<(), Self::Error> {
        for op in operations {
            match op {
                embedded_hal_async::i2c::Operation::Read(buf) => {
                    embedded_hal::i2c::I2c::read(&mut *self.0.borrow_mut(), address, buf)?;
                }
                embedded_hal_async::i2c::Operation::Write(buf) => {
                    embedded_hal::i2c::I2c::write(&mut *self.0.borrow_mut(), address, buf)?;
                }
            }
        }
        Ok(())
    }
}

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Starting ToF Theremin Firmware (Interrupt-driven)...");

    let peripherals = Peripherals::take().unwrap();
    let pins = peripherals.pins;

    let i2c_config = I2cConfig::new().baudrate(400_u32.kHz().into());
    let mut i2c = I2cDriver::new(peripherals.i2c0, pins.gpio1, pins.gpio2, &i2c_config).unwrap();
    let i2c_bus = core::cell::RefCell::new(i2c);

    #[repr(C)]
    pub struct tinyusb_config_t {
        pub device_descriptor: *const core::ffi::c_void,
        pub string_descriptor: *mut *const core::ffi::c_char,
        pub string_descriptor_count: core::ffi::c_int,
        pub external_phy: bool,
        pub configuration_descriptor: *const u8,
        pub self_powered: bool,
        pub vbus_monitor_io: core::ffi::c_int,
    }

    extern "C" {
        fn tinyusb_driver_install(config: *const tinyusb_config_t) -> i32;
        fn tud_midi_stream_write(cable_num: u8, buffer: *const u8, bufsize: u32) -> u32;
    }

    log::info!("Initializing TinyUSB for Native USB-MIDI Class...");
    unsafe {
        let tusb_cfg = tinyusb_config_t {
            device_descriptor: core::ptr::null(),
            string_descriptor: core::ptr::null_mut(),
            string_descriptor_count: 0,
            external_phy: false,
            configuration_descriptor: core::ptr::null(),
            self_powered: false,
            vbus_monitor_io: 0,
        };
        tinyusb_driver_install(&tusb_cfg);
    }

    let mut xshut_pitch = PinDriver::output(pins.gpio6).unwrap();
    let mut xshut_volume = PinDriver::output(pins.gpio7).unwrap();
    let mut int_pitch = PinDriver::input(pins.gpio4, Pull::Floating).unwrap();
    let mut int_volume = PinDriver::input(pins.gpio5, Pull::Floating).unwrap();

    let mut processor = TofThereminProcessor::default();
    let mut out_buffer = [InstrumentEvent::NoteOn {
        channel: expressive_core::Channel::new(1),
        note: expressive_core::U7::new(0),
        velocity: expressive_core::U7::new(0),
    }; 10];

    block_on(async {
        xshut_pitch.set_low().unwrap();
        xshut_volume.set_low().unwrap();
        embassy_time::Timer::after_millis(10).await;

        log::info!("Booting Volume Sensor...");
        xshut_volume.set_high().unwrap();
        embassy_time::Timer::after_millis(50).await;
        
        let mut sensor_vol = vl53l4cd::Vl53l4cd::new(AsyncI2cWrapper(&i2c_bus), embassy_time::Delay, vl53l4cd::wait::Poll);
        sensor_vol.init().await.unwrap();
        // Change address via raw I2C write since set_device_address is missing in v0.4.0
        use embedded_hal_async::i2c::I2c;
        let mut bus = AsyncI2cWrapper(&i2c_bus);
        bus.write(0x29, &[0x00, 0x01, 0x2A]).await.unwrap();
        // Recreate sensor instance with the new address
        let mut sensor_vol = vl53l4cd::Vl53l4cd::with_addr(AsyncI2cWrapper(&i2c_bus), 0x2A, embassy_time::Delay, vl53l4cd::wait::Poll);
        log::info!("Volume Sensor initialized and address changed to 0x2A.");

        log::info!("Booting Pitch Sensor...");
        xshut_pitch.set_high().unwrap();
        embassy_time::Timer::after_millis(50).await;
        
        let mut sensor_pitch = vl53l4cd::Vl53l4cd::new(AsyncI2cWrapper(&i2c_bus), embassy_time::Delay, vl53l4cd::wait::Poll);
        sensor_pitch.init().await.unwrap();
        log::info!("Pitch Sensor initialized at default address 0x29.");

        sensor_pitch.set_range_timing(10, 0).await.unwrap();
        sensor_vol.set_range_timing(10, 0).await.unwrap();

        sensor_pitch.start_ranging().await.unwrap();
        sensor_vol.start_ranging().await.unwrap();

        log::info!("Entering main async interrupt loop...");

        loop {
            if let Err(e) = int_pitch.wait_for_falling_edge().await {
                log::error!("Interrupt error: {:?}", e);
                continue;
            }

            sensor_pitch.clear_interrupt().await.unwrap();

            let measurement = sensor_pitch.measure().await.unwrap();
            let distance_right = measurement.distance;

            let event_pitch = SensorEvent::Position3D {
                id: 0,
                x: 0,
                y: 0,
                z: distance_right,
            };
            
            if let Ok(count) = processor.process_event(event_pitch, &mut out_buffer) {
                for i in 0..count {
                    let event = out_buffer[i];
                    log::info!("Generated Event: {:?}", event);
                    
                    use expressive_core::midi::ToMidi;
                    let midi_msg = event.to_midi();
                    let midi_bytes = midi_msg.as_slice();
                    unsafe {
                        tud_midi_stream_write(0, midi_bytes.as_ptr(), midi_bytes.len() as u32);
                    }
                    log::info!("USB-MIDI TX: {:?}", midi_bytes);
                }
            }
        }
    });
}
