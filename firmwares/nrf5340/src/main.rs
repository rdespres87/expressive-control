#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

mod sensor;

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use embassy_nrf::usb::{Driver, vbus_detect::HardwareVbusDetect};
use embassy_nrf::bind_interrupts;
use embassy_nrf::peripherals::USBD;
use embassy_usb::{Builder, Config};
use embassy_usb::class::midi::MidiClass;
use embassy_sync::channel::Channel;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;

use expressive_core::{Channel as CoreChannel, InstrumentEvent, SensorEvent, U7};
use expressive_core::midi::ToMidi;
use expressive_core::processor::{ControlProcessor, MpeInstrument};
use expressive_core::{MidiTransport, TransportError};

bind_interrupts!(struct Irqs {
    USBD => embassy_nrf::usb::InterruptHandler<USBD>;
    USBREGULATOR => embassy_nrf::usb::vbus_detect::InterruptHandler;
    SERIAL0 => embassy_nrf::twim::InterruptHandler<embassy_nrf::peripherals::SERIAL0>;
});

static MIDI_CHANNEL: Channel<ThreadModeRawMutex, [u8; 4], 64> = Channel::new();

struct UsbMidiTransport;

impl MidiTransport for UsbMidiTransport {
    fn send_bytes(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        let mut packet = [0u8; 4];
        if bytes.is_empty() { return Ok(()); }
        
        let status = bytes[0] >> 4;
        let cin = match status {
            0x8 => 0x08,
            0x9 => 0x09,
            0xB => 0x0B,
            0xC => 0x0C,
            0xD => 0x0D,
            0xE => 0x0E,
            _ => 0x0F,
        };
        
        packet[0] = cin;
        let len = bytes.len().min(3);
        packet[1..1 + len].copy_from_slice(&bytes[..len]);
        
        match MIDI_CHANNEL.try_send(packet) {
            Ok(_) => Ok(()),
            Err(_) => Err(TransportError::TxBufferFull),
        }
    }
}

#[embassy_executor::task]
async fn usb_task(mut usb: embassy_usb::UsbDevice<'static, Driver<'static, HardwareVbusDetect>>) {
    usb.run().await;
}

#[embassy_executor::task]
async fn midi_task(mut midi: MidiClass<'static, Driver<'static, HardwareVbusDetect>>) {
    loop {
        let packet = MIDI_CHANNEL.receive().await;
        let _ = midi.write_packet(&packet).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_nrf::init(Default::default());

    defmt::info!("Expressive Control nRF5340 firmware starting!");

    let driver = Driver::new(p.USBD, Irqs, HardwareVbusDetect::new(Irqs));

    #[allow(static_mut_refs)]
    static mut CONFIG_DESCRIPTOR: [u8; 256] = [0; 256];
    #[allow(static_mut_refs)]
    static mut BOS_DESCRIPTOR: [u8; 256] = [0; 256];
    #[allow(static_mut_refs)]
    static mut CONTROL_BUF: [u8; 64] = [0; 64];

    let mut config = Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Expressive Control");
    config.product = Some("MPE Instrument");
    config.serial_number = Some("12345678");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    let mut builder = Builder::new(
        driver,
        config,
        unsafe { &mut *core::ptr::addr_of_mut!(CONFIG_DESCRIPTOR) },
        unsafe { &mut *core::ptr::addr_of_mut!(BOS_DESCRIPTOR) },
        &mut [],
        unsafe { &mut *core::ptr::addr_of_mut!(CONTROL_BUF) },
    );
    
    let midi_class = MidiClass::new(&mut builder, 1, 1, 64);
    let usb = builder.build();

    match usb_task(usb) {
        Ok(task) => spawner.spawn(task),
        Err(_) => defmt::error!("Failed to allocate USB task"),
    }
    match midi_task(midi_class) {
        Ok(task) => spawner.spawn(task),
        Err(_) => defmt::error!("Failed to allocate MIDI task"),
    }

    let mut instrument = MpeInstrument::default();
    let mut transport = UsbMidiTransport;
    
    // Initialize I2C and sensor
    let twim_config = embassy_nrf::twim::Config::default();
    #[allow(static_mut_refs)]
    static mut TWIM_BUF: [u8; 128] = [0; 128];
    let twim = embassy_nrf::twim::Twim::new(p.SERIAL0, Irqs, p.P0_04, p.P0_05, twim_config, unsafe { &mut *core::ptr::addr_of_mut!(TWIM_BUF) });
    let mut sensor = sensor::I2cSensor::new(twim, 0x42);
    let mut sensor_buffer = [SensorEvent::Released { id: 0 }; 4];
    
    let dummy_event = InstrumentEvent::NoteOff { channel: CoreChannel::new(0), note: U7::new(0), velocity: U7::new(0) };
    let mut out_buffer = [dummy_event; 16];

    loop {
        // Poll actual sensor
        if let Ok(status) = expressive_core::SensorReader::poll_events(&mut sensor, &mut sensor_buffer) {
            for i in 0..status.count {
                if let Ok(count) = instrument.process_event(sensor_buffer[i], &mut out_buffer) {
                    for event_out in out_buffer.iter().take(count) {
                        let midi_msg = event_out.to_midi();
                        defmt::info!("MIDI Out: {:?}", midi_msg.as_slice());
                        let _ = transport.send_bytes(midi_msg.as_slice());
                    }
                }
            }
        }

        Timer::after(Duration::from_millis(500)).await;
    }
}
