#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::i2c::InterruptHandler as I2cInterruptHandler;
use embassy_rp::i2c_slave::{Command, Config as I2cSlaveConfig, I2cSlave};
use embassy_rp::peripherals::{I2C0, USB};
use embassy_rp::usb::{Driver, InterruptHandler as UsbInterruptHandler};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_usb::class::midi::MidiClass;
use embassy_usb::{Builder, Config as UsbConfig};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => UsbInterruptHandler<USB>;
    I2C0_IRQ => I2cInterruptHandler<I2C0>;
});

/// FIFO channel holding 4-byte USB-MIDI 1.0 packets received from I2C.
/// Capacity of 64 packets provides ample headroom even during fast continuous pitch bends.
static MIDI_QUEUE: Channel<CriticalSectionRawMutex, [u8; 4], 64> = Channel::new();

struct UsbBuffers {
    config_descriptor: [u8; 256],
    bos_descriptor: [u8; 256],
    msos_descriptor: [u8; 256],
    control_buf: [u8; 64],
}

struct UsbBuffersCell(core::cell::UnsafeCell<UsbBuffers>);
unsafe impl Sync for UsbBuffersCell {}

static USB_BUFFERS: UsbBuffersCell = UsbBuffersCell(core::cell::UnsafeCell::new(UsbBuffers {
    config_descriptor: [0; 256],
    bos_descriptor: [0; 256],
    msos_descriptor: [0; 256],
    control_buf: [0; 64],
}));

#[embassy_executor::task]
async fn usb_device_task(mut device: embassy_usb::UsbDevice<'static, Driver<'static, USB>>) -> ! {
    device.run().await
}

#[embassy_executor::task]
async fn midi_sender_task(
    mut sender: embassy_usb::class::midi::Sender<'static, Driver<'static, USB>>,
) -> ! {
    loop {
        // Wait for next packet
        let first = MIDI_QUEUE.receive().await;

        // Batch up to 15 packets (60 bytes < max_packet_size 64)
        // This ensures the USB bulk transfer is always a short packet (< 64 bytes),
        // guaranteeing immediate delivery to the host DAW without needing zero-length packets (ZLP).
        let mut buf = [0u8; 64];
        buf[0..4].copy_from_slice(&first);
        let mut count = 4;

        while count + 4 <= 60 {
            if let Ok(pkt) = MIDI_QUEUE.try_receive() {
                buf[count..count + 4].copy_from_slice(&pkt);
                count += 4;
            } else {
                break;
            }
        }

        let _ = sender.write_packet(&buf[..count]).await;
    }
}

#[embassy_executor::task]
async fn i2c_target_task(mut i2c_slave: I2cSlave<'static, I2C0>) -> ! {
    defmt::info!("I2C Target listening on address 0x40");
    let mut rx_buf = [0u8; 64];

    loop {
        match i2c_slave.listen(&mut rx_buf).await {
            Ok(Command::Write(len)) | Ok(Command::WriteRead(len)) => {
                // Process received data in 4-byte USB-MIDI packets
                let valid_len = (len / 4) * 4;
                for &packet in rx_buf[..valid_len].as_chunks::<4>().0 {
                    let _ = MIDI_QUEUE.try_send(packet);
                }
            }
            Ok(Command::Read) => {
                // ESP32 master currently only writes MIDI packets.
            }
            Ok(Command::GeneralCall(_)) => {}
            Err(e) => {
                defmt::warn!("I2C listen error: {:?}", e);
            }
        }
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    defmt::info!("RP2040-Zero USB-MIDI Coprocessor starting...");

    // 1. Initialize USB driver
    let driver = Driver::new(p.USB, Irqs);

    // 2. Configure USB Device descriptor
    let mut config = UsbConfig::new(0x16c0, 0x27dd); // Open vendor test VID/PID
    config.manufacturer = Some("Expressive Systems");
    config.product = Some("Expressive Theremin MIDI");
    config.serial_number = Some("EXP-THM-01");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    // Use Audio Class for USB-MIDI compliant streaming
    config.composite_with_iads = false;
    config.device_class = 0x00;
    config.device_sub_class = 0x00;
    config.device_protocol = 0x00;

    // Buffers for USB builder
    // Safety: Borrowed once here at initialization before tasks start.
    let buffers = unsafe { &mut *USB_BUFFERS.0.get() };

    let mut builder = Builder::new(
        driver,
        config,
        &mut buffers.config_descriptor,
        &mut buffers.bos_descriptor,
        &mut buffers.msos_descriptor,
        &mut buffers.control_buf,
    );

    // 3. Create MIDI Class with 1 IN jack, 1 OUT jack, max_packet_size 64
    let midi_class = MidiClass::new(&mut builder, 1, 1, 64);
    let (midi_sender, _midi_receiver) = midi_class.split();

    let usb_device = builder.build();

    // 4. Initialize I2C0 Target on GPIO 4 (SDA) and GPIO 5 (SCL) at address 0x40
    let mut i2c_config = I2cSlaveConfig::default();
    i2c_config.addr = 0x40;

    let i2c_slave = I2cSlave::new(
        p.I2C0, p.PIN_5, // SCL
        p.PIN_4, // SDA
        Irqs, i2c_config,
    );

    // 5. Spawn embassy tasks
    if spawner.spawn(usb_device_task(usb_device)).is_err() {
        defmt::error!("Failed to spawn usb_device_task");
    }
    if spawner.spawn(midi_sender_task(midi_sender)).is_err() {
        defmt::error!("Failed to spawn midi_sender_task");
    }
    if spawner.spawn(i2c_target_task(i2c_slave)).is_err() {
        defmt::error!("Failed to spawn i2c_target_task");
    }

    defmt::info!("RP2040-Zero USB-MIDI initialization complete.");
}
