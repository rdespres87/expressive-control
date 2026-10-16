#![no_std]
#![no_main]

use defmt_rtt as _;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let uartdr = 0x4003_8000 as *mut u32;
    let uartfr = 0x4003_8018 as *mut u32;
    let put_str = |s: &str| {
        for &b in s.as_bytes() {
            while (unsafe { core::ptr::read_volatile(uartfr) } & (1 << 5)) != 0 {}
            unsafe { core::ptr::write_volatile(uartdr, b as u32); }
        }
    };
    put_str("\r\n\r\n!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!\r\n");
    put_str("[PANIC] RP2040-Zero panicked!\r\n");
    if let Some(loc) = info.location() {
        put_str("Location: ");
        put_str(loc.file());
        put_str(":");
        let mut line_buf = [0u8; 10];
        let mut l = loc.line();
        let mut idx = 10;
        if l == 0 {
            idx -= 1;
            line_buf[idx] = b'0';
        } else {
            while l > 0 {
                idx -= 1;
                line_buf[idx] = b'0' + (l % 10) as u8;
                l /= 10;
            }
        }
        for &b in &line_buf[idx..] {
            while (unsafe { core::ptr::read_volatile(uartfr) } & (1 << 5)) != 0 {}
            unsafe { core::ptr::write_volatile(uartdr, b as u32); }
        }
        put_str("\r\n");
    }
    put_str("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!\r\n");
    loop {}
}


use core::cell::{Cell, RefCell};
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler as UsbInterruptHandler};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::channel::Channel;
use embassy_time::Timer;
use embassy_usb::class::midi::MidiClass;
use embassy_usb::{Builder, Config as UsbConfig};
use expressive_core::input::{DebouncedButton, RotaryEncoder};
use expressive_core::rpe::{
    self, buttons, flags, xshut, RpeIncomingPacket, RpeInputReport,
};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => UsbInterruptHandler<USB>;
    I2C0_IRQ => embassy_rp::i2c::InterruptHandler<embassy_rp::peripherals::I2C0>;
});

#[no_mangle]
#[used]
static _KEEP_USBCTRL_IRQ: unsafe extern "C" fn() = USBCTRL_IRQ;

#[no_mangle]
#[used]
static _KEEP_I2C0_IRQ: unsafe extern "C" fn() = I2C0_IRQ;

#[cortex_m_rt::exception]
unsafe fn DefaultHandler(irqn: i16) {
    // IRQ 0 is TIMER_IRQ_0 on RP2040
    if irqn == 0 {
        let timer_intr = 0x4005_4034 as *mut u32;
        core::ptr::write_volatile(timer_intr, 1 << 0);
        return;
    }
    // IRQ 5 is USBCTRL_IRQ on RP2040
    if irqn == 5 {
        <UsbInterruptHandler<USB> as embassy_rp::interrupt::typelevel::Handler<embassy_rp::interrupt::typelevel::USBCTRL_IRQ>>::on_interrupt();
        return;
    }
    // IRQ 23 is I2C0_IRQ on RP2040
    if irqn == 23 {
        <embassy_rp::i2c::InterruptHandler<embassy_rp::peripherals::I2C0> as embassy_rp::interrupt::typelevel::Handler<embassy_rp::interrupt::typelevel::I2C0_IRQ>>::on_interrupt();
        return;
    }

    let uartdr = 0x4003_8000 as *mut u32;
    let uartfr = 0x4003_8018 as *mut u32;
    for &b in b"\r\n[WARN] Unhandled IRQ: " {
        while (core::ptr::read_volatile(uartfr) & (1 << 5)) != 0 {}
        core::ptr::write_volatile(uartdr, b as u32);
    }
    let mut num = irqn;
    if num < 0 {
        while (core::ptr::read_volatile(uartfr) & (1 << 5)) != 0 {}
        core::ptr::write_volatile(uartdr, b'-' as u32);
        num = -num;
    }
    let mut digits = [0u8; 5];
    let mut i = 5;
    if num == 0 {
        i -= 1;
        digits[i] = b'0';
    } else {
        while num > 0 {
            i -= 1;
            digits[i] = b'0' + (num % 10) as u8;
            num /= 10;
        }
    }
    for &b in &digits[i..] {
        while (core::ptr::read_volatile(uartfr) & (1 << 5)) != 0 {}
        core::ptr::write_volatile(uartdr, b as u32);
    }
    for &b in b"\r\n" {
        while (core::ptr::read_volatile(uartfr) & (1 << 5)) != 0 {}
        core::ptr::write_volatile(uartdr, b as u32);
    }
}


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

        let res = sender.write_packet(&buf[..count]).await;
        if res.is_ok() {
            USB_CONFIGURED.lock(|cell| cell.set(true));
        }
    }
}

struct FrontPanelState {
    encoder_delta: i8,
    button_state: u8,
    button_clicks: u8,
    xshut_mask: u8,
}

static FRONT_PANEL: Mutex<CriticalSectionRawMutex, RefCell<FrontPanelState>> =
    Mutex::new(RefCell::new(FrontPanelState {
        encoder_delta: 0,
        button_state: 0,
        button_clicks: 0,
        xshut_mask: 0,
    }));

static USB_CONFIGURED: Mutex<CriticalSectionRawMutex, Cell<bool>> =
    Mutex::new(Cell::new(false));

fn update_front_panel(delta: i8, state: u8, clicks: u8) {
    FRONT_PANEL.lock(|cell| {
        let mut fp = cell.borrow_mut();
        fp.encoder_delta = fp.encoder_delta.saturating_add(delta);
        fp.button_state = state;
        fp.button_clicks |= clicks;
    });
}

fn read_and_clear_input_report(usb_configured: bool) -> RpeInputReport {
    FRONT_PANEL.lock(|cell| {
        let mut fp = cell.borrow_mut();
        let delta = fp.encoder_delta;
        let state = fp.button_state;
        let clicks = fp.button_clicks;
        let xshut = fp.xshut_mask;

        // Reset accumulators upon read
        fp.encoder_delta = 0;
        fp.button_clicks = 0;

        let mut status = 0u8;
        if usb_configured {
            status |= flags::USB_CONFIGURED;
        }
        if (xshut & xshut::LEFT_TOF) != 0 {
            status |= flags::XSHUT_LEFT_ACTIVE;
        }
        if (xshut & xshut::RIGHT_TOF) != 0 {
            status |= flags::XSHUT_RIGHT_ACTIVE;
        }

        RpeInputReport {
            encoder_delta: delta,
            button_state: state,
            button_clicks: clicks,
            status_flags: status,
        }
    })
}

fn apply_xshut(mask: u8) {
    FRONT_PANEL.lock(|cell| {
        cell.borrow_mut().xshut_mask = mask;
    });
    let sio_set = 0xD000_0014 as *mut u32;
    let sio_clr = 0xD000_0018 as *mut u32;
    unsafe {
        if (mask & xshut::LEFT_TOF) != 0 {
            core::ptr::write_volatile(sio_set, 1 << 2);
        } else {
            core::ptr::write_volatile(sio_clr, 1 << 2);
        }
        if (mask & xshut::RIGHT_TOF) != 0 {
            core::ptr::write_volatile(sio_set, 1 << 3);
        } else {
            core::ptr::write_volatile(sio_clr, 1 << 3);
        }
    }
}

pub const METADATA_BASE: u32 = 0x101F_F000;
pub const BOOTLOADER_MAGIC: u32 = 0xB007_10AD;
pub const WATCHDOG_SCRATCH0: *mut u32 = 0x4005_800C as *mut u32;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct AppMetadata {
    pub magic: u32,
    pub version: u32,
    pub size: u32,
    pub crc32: u32,
    pub valid: u32,
}

fn uart_print(s: &str) {
    let uartdr = 0x4003_8000 as *mut u32;
    let uartfr = 0x4003_8018 as *mut u32;
    for &b in s.as_bytes() {
        while (unsafe { core::ptr::read_volatile(uartfr) } & (1 << 5)) != 0 {}
        unsafe { core::ptr::write_volatile(uartdr, b as u32); }
    }
}

fn process_i2c_command(rx: &[u8], tx: &mut [u8; 16], should_reset: &mut bool) {
    if rx.is_empty() {
        return;
    }
    match rpe::classify_incoming_packet(rx) {
        RpeIncomingPacket::StatusQuery => {
            uart_print("[I2C] CMD_GET_STATUS (0x01)\r\n");
            let meta = unsafe { core::ptr::read_volatile(METADATA_BASE as *const AppMetadata) };
            tx[0] = 0xA0; // State: Application running
            tx[1] = 0x01; // Protocol v1
            tx[2..6].copy_from_slice(&meta.size.to_be_bytes());
            tx[6..10].copy_from_slice(&meta.crc32.to_be_bytes());
            tx[10..14].copy_from_slice(&meta.version.to_be_bytes());
        }
        RpeIncomingPacket::BootApp => {
            // Already running application
        }
        RpeIncomingPacket::ReadInputsRequest => {
            let usb_ok = USB_CONFIGURED.lock(|cell| cell.get());
            let report = read_and_clear_input_report(usb_ok);
            tx[0..4].copy_from_slice(&report.to_bytes());
        }
        RpeIncomingPacket::EnterBootloader => {
            uart_print("[I2C] CMD_ENTER_BOOTLOADER (0xFB) -> Soft Reset to Bootloader\r\n");
            tx[0] = 0x06; // ACK
            unsafe { core::ptr::write_volatile(WATCHDOG_SCRATCH0, BOOTLOADER_MAGIC) };
            *should_reset = true;
        }
        RpeIncomingPacket::SetXshut { mask } => {
            apply_xshut(mask);
        }
        RpeIncomingPacket::UsbMidi(midi_bytes) => {
            let valid_len = (midi_bytes.len() / 4) * 4;
            for &packet in midi_bytes[..valid_len].as_chunks::<4>().0 {
                let _ = MIDI_QUEUE.try_send(packet);
            }
        }
        RpeIncomingPacket::Unknown => {}
    }
}

#[embassy_executor::task]
async fn i2c_target_task(
    mut i2c_slave: embassy_rp::i2c_slave::I2cSlave<'static, embassy_rp::peripherals::I2C0>,
) -> ! {
    let mut rx_buf = [0u8; 64];
    let mut tx_buf = [0u8; 16];
    let mut should_reset = false;

    loop {
        match i2c_slave.listen(&mut rx_buf).await {
            Ok(embassy_rp::i2c_slave::Command::Write(len)) => {
                process_i2c_command(&rx_buf[..len], &mut tx_buf, &mut should_reset);
            }
            Ok(embassy_rp::i2c_slave::Command::WriteRead(len)) => {
                process_i2c_command(&rx_buf[..len], &mut tx_buf, &mut should_reset);
                let _ = i2c_slave.respond_to_read(&tx_buf).await;
            }
            Ok(embassy_rp::i2c_slave::Command::Read) => {
                let _ = i2c_slave.respond_to_read(&tx_buf).await;
            }
            Ok(embassy_rp::i2c_slave::Command::GeneralCall(_)) => {}
            Err(_) => {
                uart_print("[I2C] Error on bus -> resetting i2c_slave\r\n");
                i2c_slave.reset();
            }
        }
        if should_reset {
            uart_print("[WARN] should_reset is true! SCB::sys_reset() in 10ms...\r\n");
            Timer::after_millis(10).await;
            cortex_m::peripheral::SCB::sys_reset();
        }
    }
}

#[embassy_executor::task]
async fn front_panel_task(
    pin_enc_a: embassy_rp::gpio::Input<'static>,
    pin_enc_b: embassy_rp::gpio::Input<'static>,
    pin_sw: embassy_rp::gpio::Input<'static>,
    pin_oct_up: embassy_rp::gpio::Input<'static>,
    pin_oct_down: embassy_rp::gpio::Input<'static>,
    pin_mute: embassy_rp::gpio::Input<'static>,
) -> ! {
    let mut encoder = RotaryEncoder::new();
    let mut btn_sw = DebouncedButton::new();
    let mut btn_oct_up = DebouncedButton::new();
    let mut btn_oct_down = DebouncedButton::new();
    let mut btn_mute = DebouncedButton::new();

    loop {
        Timer::after_millis(1).await;

        let enc_clk = pin_enc_a.is_high();
        let enc_dt = pin_enc_b.is_high();
        let detent = encoder.update(enc_clk, enc_dt);

        let sw_state = btn_sw.update(pin_sw.is_low());
        let oct_up_state = btn_oct_up.update(pin_oct_up.is_low());
        let oct_down_state = btn_oct_down.update(pin_oct_down.is_low());
        let mute_state = btn_mute.update(pin_mute.is_low());

        let mut button_mask = 0u8;
        if sw_state.is_pressed {
            button_mask |= buttons::ENCODER_SW;
        }
        if oct_up_state.is_pressed {
            button_mask |= buttons::OCTAVE_UP;
        }
        if oct_down_state.is_pressed {
            button_mask |= buttons::OCTAVE_DOWN;
        }
        if mute_state.is_pressed {
            button_mask |= buttons::MUTE;
        }

        let mut clicks_mask = 0u8;
        if sw_state.just_pressed {
            clicks_mask |= buttons::ENCODER_SW;
        }
        if oct_up_state.just_pressed {
            clicks_mask |= buttons::OCTAVE_UP;
        }
        if oct_down_state.just_pressed {
            clicks_mask |= buttons::OCTAVE_DOWN;
        }
        if mute_state.just_pressed {
            clicks_mask |= buttons::MUTE;
        }

        if detent != 0 || clicks_mask != 0 || button_mask != 0 {
            update_front_panel(detent as i8, button_mask, clicks_mask);
        }
    }
}

/// Drive the onboard WS2812 RGB LED on GPIO 16 (bitbang in critical section).
pub fn set_status_led(r: u8, g: u8, b: u8) {
    unsafe {
        let gpio16_ctrl = (0x4001_4000 + 16 * 8 + 4) as *mut u32;
        core::ptr::write_volatile(gpio16_ctrl, 5);

        let pad16 = (0x4001_c000 + 4 + 16 * 4) as *mut u32;
        core::ptr::write_volatile(pad16, 0x62);

        let sio_gpio_oe_set = 0xd000_0024 as *mut u32;
        let sio_gpio_set = 0xd000_0014 as *mut u32;
        let sio_gpio_clr = 0xd000_0018 as *mut u32;

        let pin_mask = 1u32 << 16;
        core::ptr::write_volatile(sio_gpio_oe_set, pin_mask);

        // Pre-latch reset (> 300 µs for WS2812B)
        core::ptr::write_volatile(sio_gpio_clr, pin_mask);
        cortex_m::asm::delay(20_000);

        let grb: u32 = ((g as u32) << 16) | ((r as u32) << 8) | (b as u32);

        cortex_m::interrupt::free(|_| {
            for i in (0..24).rev() {
                if ((grb >> i) & 1) != 0 {
                    // T1H: ~800ns (100 cycles @ 125MHz -> delay 20)
                    core::ptr::write_volatile(sio_gpio_set, pin_mask);
                    cortex_m::asm::delay(20);
                    // T1L: ~450ns (56 cycles @ 125MHz -> delay 10)
                    core::ptr::write_volatile(sio_gpio_clr, pin_mask);
                    cortex_m::asm::delay(10);
                } else {
                    // T0H: ~400ns (50 cycles @ 125MHz -> delay 8)
                    core::ptr::write_volatile(sio_gpio_set, pin_mask);
                    cortex_m::asm::delay(8);
                    // T0L: ~850ns (106 cycles @ 125MHz -> delay 20)
                    core::ptr::write_volatile(sio_gpio_clr, pin_mask);
                    cortex_m::asm::delay(20);
                }
            }
        });

        // Post-latch reset (> 300 µs for WS2812B)
        core::ptr::write_volatile(sio_gpio_clr, pin_mask);
        cortex_m::asm::delay(20_000);
    }
}

#[cortex_m_rt::exception]
unsafe fn HardFault(_ef: &cortex_m_rt::ExceptionFrame) -> ! {
    let uartdr = 0x4003_8000 as *mut u32;
    let uartfr = 0x4003_8018 as *mut u32;
    for &b in b"\r\n[FATAL] HardFault Exception occurred!\r\n" {
        while (core::ptr::read_volatile(uartfr) & (1 << 5)) != 0 {}
        core::ptr::write_volatile(uartdr, b as u32);
    }
    loop {
        cortex_m::asm::wfi();
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Relocate Vector Table to Flash at 0x10008000 so interrupts (USBCTRL_IRQ, etc.) are serviced
    unsafe {
        let vtor = 0xE000_ED08 as *mut u32;
        core::ptr::write_volatile(vtor, 0x1000_8000);
    }

    let p = embassy_rp::init(Default::default());

    // Explicitly re-enable interrupts so that USBCTRL_IRQ and timers are serviced
    unsafe { cortex_m::interrupt::enable(); }

    // Initialize UART1 on GP4 (TX) and GP5 (RX) at 115200 baud for FTDI debug
    let mut uart_config = embassy_rp::uart::Config::default();
    uart_config.baudrate = 115200;
    let mut uart = embassy_rp::uart::Uart::new_blocking(p.UART1, p.PIN_4, p.PIN_5, uart_config);

    let _ = uart.blocking_write(b"\r\n\r\n========================================\r\n");
    let _ = uart.blocking_write(b"  RP2040-Zero USB-MIDI Debug Console  \r\n");
    let _ = uart.blocking_write(b"  UART1 @ 115200 baud (GP4=TX, GP5=RX) \r\n");
    let _ = uart.blocking_write(b"========================================\r\n");
    let _ = uart.blocking_write(b"[BOOT] VTOR set to 0x10008000\r\n");
    let _ = uart.blocking_write(b"[BOOT] embassy_rp::init() completed successfully\r\n");

    // Soft magenta LED: indicates USB-MIDI app is active (Red=16, Green=0, Blue=16)
    set_status_led(16, 0, 16);
    let _ = uart.blocking_write(b"[LED] Status LED set to Magenta (R=16, G=0, B=16)\r\n");

    defmt::info!("RP2040-Zero USB-MIDI Coprocessor starting...");

    // 1. Initialize USB driver
    let _ = uart.blocking_write(b"[USB] Initializing USB driver on p.USB...\r\n");
    let driver = Driver::new(p.USB, Irqs);
    let _ = uart.blocking_write(b"[USB] Driver::new() OK\r\n");
    let _ = uart.blocking_flush();

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

    // 4. Initialize I2C0 Target on GP0 (SDA) and GP1 (SCL) at address 0x40
    let mut i2c_config = embassy_rp::i2c_slave::Config::default();
    i2c_config.addr = 0x40;
    i2c_config.general_call = false; // Never respond to other devices (GT911 @ 0x14, etc.)

    let i2c_slave = embassy_rp::i2c_slave::I2cSlave::new(
        p.I2C0,
        p.PIN_1, // SCL
        p.PIN_0, // SDA
        Irqs,
        i2c_config,
    );

    // 5. Initialize GPIO outputs for ToF XSHUT (GP2, GP3) - default LOW (held in reset)
    let _xshut_l = embassy_rp::gpio::Output::new(p.PIN_2, embassy_rp::gpio::Level::Low);
    let _xshut_r = embassy_rp::gpio::Output::new(p.PIN_3, embassy_rp::gpio::Level::Low);

    // 6. Initialize GPIO inputs with pull-ups for Front Panel (GP6..GP8 for encoder, GP14, GP15, GP26 for buttons)
    let pin_enc_a = embassy_rp::gpio::Input::new(p.PIN_6, embassy_rp::gpio::Pull::Up);
    let pin_enc_b = embassy_rp::gpio::Input::new(p.PIN_7, embassy_rp::gpio::Pull::Up);
    let pin_sw = embassy_rp::gpio::Input::new(p.PIN_8, embassy_rp::gpio::Pull::Up);
    let pin_oct_up = embassy_rp::gpio::Input::new(p.PIN_14, embassy_rp::gpio::Pull::Up);
    let pin_oct_down = embassy_rp::gpio::Input::new(p.PIN_15, embassy_rp::gpio::Pull::Up);
    let pin_mute = embassy_rp::gpio::Input::new(p.PIN_26, embassy_rp::gpio::Pull::Up);

    // 7. Spawn embassy tasks
    let _ = uart.blocking_write(b"[BOOT] Spawning midi_sender_task...\r\n");
    let _ = uart.blocking_flush();
    if spawner.spawn(midi_sender_task(midi_sender)).is_err() {
        let _ = uart.blocking_write(b"[ERR] Failed to spawn midi_sender_task\r\n");
    } else {
        let _ = uart.blocking_write(b"[OK] midi_sender_task spawned\r\n");
    }
    let _ = uart.blocking_flush();

    let _ = uart.blocking_write(b"[BOOT] Spawning i2c_target_task (Slave @ 0x40)...\r\n");
    let _ = uart.blocking_flush();
    if spawner.spawn(i2c_target_task(i2c_slave)).is_err() {
        let _ = uart.blocking_write(b"[ERR] Failed to spawn i2c_target_task\r\n");
    } else {
        let _ = uart.blocking_write(b"[OK] i2c_target_task spawned\r\n");
    }
    let _ = uart.blocking_flush();

    let _ = uart.blocking_write(b"[BOOT] Spawning front_panel_task...\r\n");
    let _ = uart.blocking_flush();
    if spawner
        .spawn(front_panel_task(
            pin_enc_a,
            pin_enc_b,
            pin_sw,
            pin_oct_up,
            pin_oct_down,
            pin_mute,
        ))
        .is_err()
    {
        let _ = uart.blocking_write(b"[ERR] Failed to spawn front_panel_task\r\n");
    } else {
        let _ = uart.blocking_write(b"[OK] front_panel_task spawned\r\n");
    }
    let _ = uart.blocking_flush();

    let _ = uart.blocking_write(b"=== READY: Starting USB Device Engine ===\r\n");
    let _ = uart.blocking_flush();

    let mut device = usb_device;
    device.run().await
}

