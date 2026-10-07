#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

#[cfg(not(feature = "hil"))]
mod sensor;

use embassy_executor::Spawner;
use embassy_nrf::bind_interrupts;
use embassy_nrf::peripherals::USBD;
use embassy_nrf::usb::{vbus_detect::HardwareVbusDetect, Driver};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Timer};
use embassy_usb::class::midi::MidiClass;
use embassy_usb::{Builder, Config};

use expressive_core::midi::ToMidi;
use expressive_core::processor::{ControlProcessor, MpeInstrument};
use expressive_core::{Channel as CoreChannel, InstrumentEvent, SensorEvent, U7};
use expressive_core::{MidiTransport, TransportError};

bind_interrupts!(struct Irqs {
    USBD => embassy_nrf::usb::InterruptHandler<USBD>;
    USBREGULATOR => embassy_nrf::usb::vbus_detect::InterruptHandler;
    #[cfg(feature = "hil")]
    SERIAL0 => embassy_nrf::uarte::InterruptHandler<embassy_nrf::peripherals::SERIAL0>;
    #[cfg(not(feature = "hil"))]
    SERIAL0 => embassy_nrf::twim::InterruptHandler<embassy_nrf::peripherals::SERIAL0>;
});

static MIDI_CHANNEL: Channel<ThreadModeRawMutex, [u8; 4], 64> = Channel::new();

struct UsbMidiTransport;

impl MidiTransport for UsbMidiTransport {
    fn send_bytes(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        let mut packet = [0u8; 4];
        if bytes.is_empty() {
            return Ok(());
        }

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
    let mut buf = [0u8; 64];
    loop {
        match embassy_futures::select::select(MIDI_CHANNEL.receive(), midi.read_packet(&mut buf))
            .await
        {
            embassy_futures::select::Either::First(packet) => {
                if midi.write_packet(&packet).await.is_err() {
                    Timer::after(Duration::from_millis(50)).await;
                }
            }
            embassy_futures::select::Either::Second(res) => {
                if res.is_err() {
                    Timer::after(Duration::from_millis(50)).await;
                }
            }
        }
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut config = embassy_nrf::config::Config::default();
    config.hfclk_source = embassy_nrf::config::HfclkSource::ExternalXtal;
    let p = embassy_nrf::init(config);

    #[cfg(feature = "hil")]
    defmt::info!("Expressive Control HIL Firmware starting!");
    #[cfg(not(feature = "hil"))]
    defmt::info!("Expressive Control nRF5340 Firmware starting!");

    let driver = Driver::new(p.USBD, Irqs, HardwareVbusDetect::new(Irqs));

    #[allow(static_mut_refs)]
    static mut CONFIG_DESCRIPTOR: [u8; 256] = [0; 256];
    #[allow(static_mut_refs)]
    static mut BOS_DESCRIPTOR: [u8; 256] = [0; 256];
    #[allow(static_mut_refs)]
    static mut CONTROL_BUF: [u8; 64] = [0; 64];

    let mut config = Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Expressive Control");
    #[cfg(feature = "hil")]
    {
        config.product = Some("MPE Instrument HIL");
    }
    #[cfg(not(feature = "hil"))]
    {
        config.product = Some("MPE Instrument");
    }

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
        Ok(token) => spawner.spawn(token),
        Err(e) => {
            defmt::error!("Failed to spawn USB task: {}", defmt::Debug2Format(&e));
            return;
        }
    }
    match midi_task(midi_class) {
        Ok(token) => spawner.spawn(token),
        Err(e) => {
            defmt::error!("Failed to spawn MIDI task: {}", defmt::Debug2Format(&e));
            return;
        }
    }

    let mut instrument = MpeInstrument::default();
    let mut transport = UsbMidiTransport;

    let dummy_event = InstrumentEvent::NoteOff {
        channel: CoreChannel::new(0),
        note: U7::new(0),
        velocity: U7::new(0),
    };
    let mut out_buffer = [dummy_event; 16];

    #[cfg(feature = "hil")]
    {
        // Initialize UART for HIL Injection
        let mut uarte_config = embassy_nrf::uarte::Config::default();
        uarte_config.baudrate = embassy_nrf::uarte::Baudrate::Baud115200;
        let mut uarte =
            embassy_nrf::uarte::Uarte::new(p.SERIAL0, p.P0_22, p.P0_20, Irqs, uarte_config);

        let mut rx_buf = [0u8; 1];
        let mut cobs_buf = postcard::accumulator::CobsAccumulator::<32>::new();

        defmt::info!("Waiting for data on UART...");

        loop {
            if uarte.read(&mut rx_buf).await.is_ok() {
                let window = &[rx_buf[0]][..];
                match cobs_buf.feed::<SensorEvent>(window) {
                    postcard::accumulator::FeedResult::Consumed => {}
                    postcard::accumulator::FeedResult::OverFull(_) => defmt::error!("OverFull"),
                    postcard::accumulator::FeedResult::DeserError(_) => defmt::error!("DeserError"),
                    postcard::accumulator::FeedResult::Success { data, .. } => {
                        defmt::info!("Received HIL: {:?}", defmt::Debug2Format(&data));
                        if let Ok(count) = instrument.process_event(data, &mut out_buffer) {
                            for event_out in out_buffer.iter().take(count) {
                                let midi_msg = event_out.to_midi();
                                defmt::info!("MIDI Out: {:?}", midi_msg.as_slice());
                                let _ = transport.send_bytes(midi_msg.as_slice());
                            }
                        }
                    }
                }
            }
        }
    }

    #[cfg(not(feature = "hil"))]
    {
        // Initialize I2C and sensor
        let twim_config = embassy_nrf::twim::Config::default();
        #[allow(static_mut_refs)]
        static mut TWIM_BUF: [u8; 128] = [0; 128];
        let twim =
            embassy_nrf::twim::Twim::new(p.SERIAL0, Irqs, p.P0_04, p.P0_05, twim_config, unsafe {
                &mut *core::ptr::addr_of_mut!(TWIM_BUF)
            });
        let mut sensor = sensor::I2cSensor::new(twim, 0x42);
        let mut sensor_buffer = [SensorEvent::Released { id: 0 }; 4];

        loop {
            if let Ok(status) =
                expressive_core::SensorReader::poll_events(&mut sensor, &mut sensor_buffer)
            {
                for event in sensor_buffer.iter().take(status.count) {
                    if let Ok(count) = instrument.process_event(*event, &mut out_buffer) {
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
}
