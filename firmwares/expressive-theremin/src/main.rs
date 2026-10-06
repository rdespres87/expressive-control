use esp_idf_hal::gpio::PinDriver;
use esp_idf_hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_hal::prelude::*;
use esp_idf_hal::task::block_on;
use expressive_core::processor::ControlProcessor;
use expressive_core::{InstrumentEvent, SensorEvent};
use theremin::TofThereminProcessor;

fn main() {
    // It is necessary to call this function once.
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Starting ToF Theremin Firmware (Interrupt-driven)...");

    let peripherals = Peripherals::take().unwrap();
    let pins = peripherals.pins;

    // 1. I2C Initialization
    let i2c_config = I2cConfig::new().baudrate(400.kHz().into());
    // WARNING: Replace `gpio1` and `gpio2` with the actual SDA/SCL pins of your Elecrow board
    let mut i2c = I2cDriver::new(peripherals.i2c0, pins.gpio1, pins.gpio2, &i2c_config).unwrap();

    let i2c_bus = core::cell::RefCell::new(i2c);

    // 2. Sequential Sensor Boot & Address Management
    // Both sensors default to I2C address 0x52 (0x29 7-bit). We must boot them one by one.
    log::info!("Starting sequential sensor boot...");
    let mut xshut_pitch = PinDriver::output(pins.gpio6).unwrap();
    let mut xshut_volume = PinDriver::output(pins.gpio7).unwrap();
    
    // Put both in reset (LOW)
    xshut_pitch.set_low().unwrap();
    xshut_volume.set_low().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));

    // A. Boot Left Sensor (Volume)
    log::info!("Booting Volume Sensor...");
    xshut_volume.set_high().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(50)); // Wait for boot
    
    // Initialize sensor at default address and change it to 0x2A
    let mut sensor_vol = vl53l4cd::Vl53l4cd::new(embedded_hal_bus::i2c::RefCellDevice::new(&i2c_bus), 0x29);
    sensor_vol.init().unwrap();
    sensor_vol.set_device_address(0x2A).unwrap();
    log::info!("Volume Sensor initialized and address changed to 0x2A.");

    // B. Boot Right Sensor (Pitch)
    log::info!("Booting Pitch Sensor...");
    xshut_pitch.set_high().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(50));
    
    // Initialize Pitch sensor, stays at default address 0x29
    let mut sensor_pitch = vl53l4cd::Vl53l4cd::new(embedded_hal_bus::i2c::RefCellDevice::new(&i2c_bus), 0x29);
    sensor_pitch.init().unwrap();
    log::info!("Pitch Sensor initialized at default address 0x29.");

    // 3. Hardware Interrupt Pins (INT)
    // WARNING: Replace `gpio4` and `gpio5` with the actual pins you choose
    let mut int_pitch = PinDriver::input(pins.gpio4).unwrap();
    let mut int_volume = PinDriver::input(pins.gpio5).unwrap();

    let mut processor = TofThereminProcessor::default();

    // Create a dummy NoteOn event just to initialize the buffer array
    let mut out_buffer = [InstrumentEvent::NoteOn {
        channel: expressive_core::Channel::new(1),
        note: expressive_core::U7::new(0),
        velocity: expressive_core::U7::new(0),
    }; 10];

    // Start continuous ranging
    sensor_pitch.start_ranging().unwrap();
    sensor_vol.start_ranging().unwrap();

    log::info!("Entering main async interrupt loop...");

    // 3. Main Event Loop (Asynchronous)
    block_on(async {
        loop {
            // The ESP32 yields to FreeRTOS here.
            // It will be woken up by a hardware interrupt (Falling Edge) from the sensor!
            if let Err(e) = int_pitch.wait_for_falling_edge().await {
                log::error!("Interrupt error: {:?}", e);
                continue;
            }

            // Clear the interrupt to allow the next one
            sensor_pitch.clear_interrupt().unwrap();

            // Read actual distance from the VL53L4CD via I2C
            let measurement = sensor_pitch.measure().unwrap();
            let distance_right = measurement.distance_mm;

            let event_pitch = SensorEvent::Position3D {
                id: 0,
                x: 0,
                y: 0,
                z: distance_right,
            };
            if let Ok(count) = processor.process_event(event_pitch, &mut out_buffer) {
                for i in 0..count {
                    log::info!("MIDI Pitch Event: {:?}", out_buffer[i]);
                }
            }

            // (In a final implementation, we would use `futures::join!` or two separate
            // tasks to listen to `int_pitch` and `int_volume` concurrently without blocking)
        }
    });
}
