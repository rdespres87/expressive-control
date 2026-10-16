#![no_std]
#![no_main]

use defmt_rtt as _;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    let uartdr = 0x4003_8000 as *mut u32;
    let uartfr = 0x4003_8018 as *mut u32;
    for &b in b"\r\n[PANIC] Bootloader Panic!\r\n" {
        while (unsafe { core::ptr::read_volatile(uartfr) } & (1 << 5)) != 0 {}
        unsafe { core::ptr::write_volatile(uartdr, b as u32); }
    }
    loop {}
}

use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::flash::{Flash, ERASE_SIZE};
use embassy_rp::i2c::InterruptHandler as I2cInterruptHandler;
use embassy_rp::i2c_slave::{Command, Config as I2cSlaveConfig, I2cSlave};
use embassy_rp::peripherals::I2C0;

bind_interrupts!(struct Irqs {
    I2C0_IRQ => I2cInterruptHandler<I2C0>;
});

#[no_mangle]
#[used]
static _KEEP_I2C0_IRQ: unsafe extern "C" fn() = I2C0_IRQ;

#[cortex_m_rt::exception]
unsafe fn DefaultHandler(irqn: i16) {
    if irqn == 0 {
        // Clear TIMER alarm 0 interrupt
        let timer_intr = 0x4005_4034 as *mut u32;
        core::ptr::write_volatile(timer_intr, 1 << 0);
    } else if irqn == 23 {
        <I2cInterruptHandler<I2C0> as embassy_rp::interrupt::typelevel::Handler<embassy_rp::interrupt::typelevel::I2C0_IRQ>>::on_interrupt();
    }
}

pub const APP_BASE: u32 = 0x1000_8000;
pub const APP_OFFSET: u32 = 0x0000_8000;

pub const METADATA_OFFSET: u32 = 0x001F_F000;
pub const METADATA_BASE: u32 = 0x101F_F000;

pub const MAGIC_VALID: u32 = 0x5641_4C49; // "VALI"
pub const MAGIC_RPEC: u32 = 0x5250_4543; // "RPEC"

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

pub fn calculate_crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            if (crc & 1) != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

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

        core::ptr::write_volatile(sio_gpio_clr, pin_mask);
        cortex_m::asm::delay(20_000);

        let grb: u32 = ((g as u32) << 16) | ((r as u32) << 8) | (b as u32);

        cortex_m::interrupt::free(|_| {
            for i in (0..24).rev() {
                if ((grb >> i) & 1) != 0 {
                    core::ptr::write_volatile(sio_gpio_set, pin_mask);
                    cortex_m::asm::delay(20);
                    core::ptr::write_volatile(sio_gpio_clr, pin_mask);
                    cortex_m::asm::delay(10);
                } else {
                    core::ptr::write_volatile(sio_gpio_set, pin_mask);
                    cortex_m::asm::delay(8);
                    core::ptr::write_volatile(sio_gpio_clr, pin_mask);
                    cortex_m::asm::delay(20);
                }
            }
        });

        core::ptr::write_volatile(sio_gpio_clr, pin_mask);
        cortex_m::asm::delay(20_000);
    }
}

pub fn read_metadata() -> AppMetadata {
    unsafe {
        let ptr = METADATA_BASE as *const AppMetadata;
        core::ptr::read_volatile(ptr)
    }
}

pub fn is_app_valid() -> bool {
    let meta = read_metadata();
    if meta.magic != MAGIC_RPEC
        || meta.valid != MAGIC_VALID
        || meta.size == 0
        || meta.size > (1024 * 1024)
    {
        return false;
    }

    unsafe {
        let sp = core::ptr::read_volatile(APP_BASE as *const u32);
        let pc = core::ptr::read_volatile((APP_BASE + 4) as *const u32);

        let sp_valid = (0x2000_0000..=0x2004_2000).contains(&sp);
        let pc_valid = (0x1000_8000..=0x1020_0000).contains(&pc) && (pc & 1) == 1;

        if !sp_valid || !pc_valid {
            return false;
        }

        let app_bytes = core::slice::from_raw_parts(APP_BASE as *const u8, meta.size as usize);
        let crc = calculate_crc32(app_bytes);
        crc == meta.crc32
    }
}

/// Jumps execution to the user application located at `APP_BASE`.
///
/// # Safety
/// The caller must ensure that `APP_BASE` contains a valid vector table with
/// a valid Initial Stack Pointer in SRAM and a valid Reset Handler in Flash.
pub unsafe fn jump_to_app() -> ! {
    cortex_m::interrupt::disable();

    // Disable and reset SysTick
    let systick_ctrl = 0xE000_E010 as *mut u32;
    core::ptr::write_volatile(systick_ctrl, 0);

    // Disable all interrupts and clear all pending interrupts in NVIC
    let nvic_icer = 0xE000_E180 as *mut u32;
    let nvic_icpr = 0xE000_E280 as *mut u32;
    core::ptr::write_volatile(nvic_icer, 0xFFFF_FFFF);
    core::ptr::write_volatile(nvic_icpr, 0xFFFF_FFFF);

    let vtor = 0xE000_ED08 as *mut u32;
    core::ptr::write_volatile(vtor, APP_BASE);

    cortex_m::asm::bootload(APP_BASE as *const u32);
}

#[derive(Default)]
pub struct UpdateState {
    pub target_size: u32,
    pub target_crc: u32,
    pub target_ver: u32,
}

fn uart_print(s: &str) {
    let uartdr = 0x4003_8000 as *mut u32;
    let uartfr = 0x4003_8018 as *mut u32;
    for &b in s.as_bytes() {
        while (unsafe { core::ptr::read_volatile(uartfr) } & (1 << 5)) != 0 {}
        unsafe { core::ptr::write_volatile(uartdr, b as u32); }
    }
}

async fn process_i2c_command(
    cmd_res: Result<Command, embassy_rp::i2c_slave::Error>,
    i2c_slave: &mut I2cSlave<'static, I2C0>,
    flash: &mut Flash<'_, embassy_rp::peripherals::FLASH, embassy_rp::flash::Blocking, { 2 * 1024 * 1024 }>,
    rx_buf: &[u8],
    tx_buf: &mut [u8; 16],
    state: &mut UpdateState,
) {
    match cmd_res {
        Ok(Command::Write(len)) => {
            if len == 0 {
                return;
            }
            let cmd = rx_buf[0];
            match cmd {
                0x01 => {
                    uart_print("[BOOTLOADER] CMD_GET_STATUS\r\n");
                    let meta = read_metadata();
                    tx_buf[0] = 0xB0; // State: Bootloader active
                    tx_buf[1] = 0x01; // Protocol v1
                    tx_buf[2..6].copy_from_slice(&meta.size.to_be_bytes());
                    tx_buf[6..10].copy_from_slice(&meta.crc32.to_be_bytes());
                    tx_buf[10..14].copy_from_slice(&meta.version.to_be_bytes());
                }
                0xFB => {
                    uart_print("[BOOTLOADER] CMD_ENTER_BOOTLOADER -> ACK\r\n");
                    tx_buf[0] = 0x06; // ACK
                }
                0x02 if len >= 13 => {
                    uart_print("[BOOTLOADER] CMD_START_UPDATE -> Erasing flash...\r\n");
                    state.target_size = u32::from_be_bytes([rx_buf[1], rx_buf[2], rx_buf[3], rx_buf[4]]);
                    state.target_crc = u32::from_be_bytes([rx_buf[5], rx_buf[6], rx_buf[7], rx_buf[8]]);
                    state.target_ver = u32::from_be_bytes([rx_buf[9], rx_buf[10], rx_buf[11], rx_buf[12]]);

                    let _ = flash.blocking_erase(METADATA_OFFSET, METADATA_OFFSET + ERASE_SIZE as u32);
                    let erase_len = (state.target_size as usize).div_ceil(ERASE_SIZE) * ERASE_SIZE;
                    let erase_to = APP_OFFSET + erase_len as u32;
                    let _ = flash.blocking_erase(APP_OFFSET, erase_to);

                    tx_buf[0] = 0x06; // ACK
                    uart_print("[BOOTLOADER] Flash erased OK\r\n");
                }
                0x03 if len >= 5 => {
                    let offset = u32::from_be_bytes([rx_buf[1], rx_buf[2], rx_buf[3], rx_buf[4]]);
                    let data = &rx_buf[5..len];
                    let flash_offset = APP_OFFSET + offset;
                    let write_res = flash.blocking_write(flash_offset, data);
                    tx_buf[0] = if write_res.is_ok() { 0x06 } else { 0x15 };
                }
                0x04 => {
                    uart_print("[BOOTLOADER] CMD_FINALIZE -> Checking CRC...\r\n");
                    let app_bytes = unsafe {
                        core::slice::from_raw_parts(APP_BASE as *const u8, state.target_size as usize)
                    };
                    let computed_crc = calculate_crc32(app_bytes);

                    if computed_crc == state.target_crc {
                        uart_print("[BOOTLOADER] CRC OK! Writing metadata...\r\n");
                        let meta = AppMetadata {
                            magic: MAGIC_RPEC,
                            version: state.target_ver,
                            size: state.target_size,
                            crc32: state.target_crc,
                            valid: MAGIC_VALID,
                        };
                        let meta_bytes = unsafe {
                            core::slice::from_raw_parts(
                                &meta as *const _ as *const u8,
                                core::mem::size_of::<AppMetadata>(),
                            )
                        };
                        let _ = flash.blocking_write(METADATA_OFFSET, meta_bytes);
                        tx_buf[0] = 0x06; // ACK
                    } else {
                        uart_print("[BOOTLOADER] CRC MISMATCH!\r\n");
                        tx_buf[0] = 0x15; // NAK
                    }
                }
                0x05 => {
                    uart_print("[BOOTLOADER] CMD_BOOT_APP -> Jumping to application...\r\n");
                    if is_app_valid() {
                        set_status_led(16, 0, 16); // Soft Magenta
                        unsafe { jump_to_app() };
                    } else {
                        uart_print("[BOOTLOADER] App invalid!\r\n");
                        tx_buf[0] = 0x15; // NAK
                    }
                }
                _ => {
                    tx_buf[0] = 0x15; // NAK
                }
            }
        }
        Ok(Command::WriteRead(_len)) => {
            let _ = i2c_slave.respond_to_read(tx_buf).await;
        }
        Ok(Command::Read) => {
            let _ = i2c_slave.respond_to_read(tx_buf).await;
        }
        Ok(Command::GeneralCall(_)) => {}
        Err(_) => {
            i2c_slave.reset();
        }
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    unsafe { cortex_m::interrupt::enable(); }

    // Initialize UART1 immediately on GP4 (TX) / GP5 (RX) for robust debug logging
    let mut uart_config = embassy_rp::uart::Config::default();
    uart_config.baudrate = 115200;
    let mut uart = embassy_rp::uart::Uart::new_blocking(p.UART1, p.PIN_4, p.PIN_5, uart_config);
    let _ = uart.blocking_write(b"\r\n\r\n========================================\r\n");
    let _ = uart.blocking_write(b"  RP2040-Zero I2C Bootloader @ 0x40   \r\n");
    let _ = uart.blocking_write(b"========================================\r\n");

    let scratch = unsafe { core::ptr::read_volatile(WATCHDOG_SCRATCH0) };
    let forced_bootloader = scratch == BOOTLOADER_MAGIC;
    if forced_bootloader {
        unsafe { core::ptr::write_volatile(WATCHDOG_SCRATCH0, 0) };
        let _ = uart.blocking_write(b"[BOOT] Forced into bootloader via WATCHDOG_SCRATCH0\r\n");
    } else if is_app_valid() {
        let _ = uart.blocking_write(b"[BOOT] Valid app detected -> Auto-booting 0x10008000...\r\n");
        set_status_led(16, 0, 16); // Soft Magenta
        unsafe { jump_to_app() };
    } else {
        let _ = uart.blocking_write(b"[BOOT] No valid app found -> Remaining in bootloader\r\n");
    }

    let _ = uart.blocking_write(b"[BOOT] Listening for I2C flash commands on 0x40...\r\n");
    // Status LED: Soft Blue (R=0, G=0, B=16) indicating Bootloader mode
    set_status_led(0, 0, 16);

    let mut flash =
        Flash::<_, embassy_rp::flash::Blocking, { 2 * 1024 * 1024 }>::new_blocking(p.FLASH);

    let mut i2c_config = I2cSlaveConfig::default();
    i2c_config.addr = 0x40;
    i2c_config.general_call = false;

    let mut i2c_slave = I2cSlave::new(
        p.I2C0,
        p.PIN_1, // SCL
        p.PIN_0, // SDA
        Irqs,
        i2c_config,
    );

    let mut rx_buf = [0u8; 300];
    let mut tx_buf = [0u8; 16];
    let mut update_state = UpdateState::default();

    loop {
        let cmd_res = i2c_slave.listen(&mut rx_buf).await;
        process_i2c_command(
            cmd_res,
            &mut i2c_slave,
            &mut flash,
            &rx_buf,
            &mut tx_buf,
            &mut update_state,
        )
        .await;
    }
}
