#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_rp::bind_interrupts;
use embassy_rp::flash::{Flash, ERASE_SIZE};
use embassy_rp::i2c::InterruptHandler as I2cInterruptHandler;
use embassy_rp::i2c_slave::{Command, Config as I2cSlaveConfig, I2cSlave};
use embassy_rp::peripherals::I2C0;
use embassy_time::Timer;

bind_interrupts!(struct Irqs {
    I2C0_IRQ => I2cInterruptHandler<I2C0>;
});

/// Base address of the application binary in Flash.
/// Vector table is relocated here (16 KB offset from 0x10000000).
pub const APP_BASE: u32 = 0x1000_4000;
pub const APP_OFFSET: u32 = 0x0000_4000;

/// Metadata sector at the end of the 2 MB flash (offset 0x001F_F000).
pub const METADATA_OFFSET: u32 = 0x001F_F000;
pub const METADATA_BASE: u32 = 0x101F_F000;

pub const MAGIC_VALID: u32 = 0x5641_4C49; // "VALI"
pub const MAGIC_RPEC: u32 = 0x5250_4543; // "RPEC"

/// Magic value in WATCHDOG_SCRATCH0 used to request staying in bootloader after soft reset.
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

/// Simple, robust IEEE 802.3 CRC32 implementation without lookup tables.
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

/// Read metadata from flash.
pub fn read_metadata() -> AppMetadata {
    unsafe {
        let ptr = METADATA_BASE as *const AppMetadata;
        core::ptr::read_volatile(ptr)
    }
}

/// Verify if the application currently in Flash is valid.
pub fn is_app_valid() -> bool {
    let meta = read_metadata();
    if meta.magic != MAGIC_RPEC
        || meta.valid != MAGIC_VALID
        || meta.size == 0
        || meta.size > (1024 * 1024)
    {
        return false;
    }

    // Check Vector Table initial SP and Reset Vector
    unsafe {
        let sp = core::ptr::read_volatile(APP_BASE as *const u32);
        let pc = core::ptr::read_volatile((APP_BASE + 4) as *const u32);

        // SP must point into 264K SRAM (0x20000000 .. 0x20042000)
        let sp_valid = (0x2000_0000..=0x2004_2000).contains(&sp);
        // PC must point into Flash application region (0x10002000 .. 0x10200000)
        let pc_valid = (0x1000_2000..=0x1020_0000).contains(&pc);

        if !sp_valid || !pc_valid {
            return false;
        }

        // Verify CRC32
        let app_bytes = core::slice::from_raw_parts(APP_BASE as *const u8, meta.size as usize);
        let crc = calculate_crc32(app_bytes);
        crc == meta.crc32
    }
}

/// Relocate VTOR, disable NVIC interrupts, and branch cleanly into application.
///
/// # Safety
/// The caller must ensure that `APP_BASE` points to a valid Cortex-M vector table.
pub unsafe fn jump_to_app() -> ! {
    defmt::info!("Jumping to application at 0x{:08x}...", APP_BASE);

    // 1. Disable all interrupts globally during transition
    cortex_m::interrupt::disable();

    // 2. Clear all NVIC enable and pending bits (RP2040 Cortex-M0+ has 32 IRQs)
    let nvic_icer = 0xE000_E180 as *mut u32;
    let nvic_icpr = 0xE000_E280 as *mut u32;
    core::ptr::write_volatile(nvic_icer, 0xFFFF_FFFF);
    core::ptr::write_volatile(nvic_icpr, 0xFFFF_FFFF);

    // 3. Relocate VTOR to application vector table
    let vtor = 0xE000_ED08 as *mut u32;
    core::ptr::write_volatile(vtor, APP_BASE);

    // 4. Branch into application
    cortex_m::asm::bootload(APP_BASE as *const u32);
}

#[derive(Default)]
pub struct UpdateState {
    pub target_size: u32,
    pub target_crc: u32,
    pub target_ver: u32,
}

/// Process a single I2C command received from the master.
async fn process_i2c_command(
    cmd_res: Result<Command, embassy_rp::i2c_slave::Error>,
    i2c_slave: &mut I2cSlave<'_, I2C0>,
    flash: &mut Flash<'_, embassy_rp::peripherals::FLASH, embassy_rp::flash::Blocking, { 2 * 1024 * 1024 }>,
    rx_buf: &[u8],
    tx_buf: &mut [u8],
    state: &mut UpdateState,
) {
    match cmd_res {
        Ok(Command::Write(len)) => {
            if len == 0 {
                return;
            }
            let cmd = rx_buf[0];
            match cmd {
                // CMD_GET_STATUS (0x01)
                0x01 => {
                    let meta = read_metadata();
                    tx_buf[0] = if is_app_valid() { 0xA0 } else { 0xB0 }; // State
                    tx_buf[1] = 0x01; // Protocol v1
                    tx_buf[2..6].copy_from_slice(&meta.size.to_be_bytes());
                    tx_buf[6..10].copy_from_slice(&meta.crc32.to_be_bytes());
                    tx_buf[10..14].copy_from_slice(&meta.version.to_be_bytes());
                }
                // CMD_START_UPDATE (0x02): [0x02, size: u32_be, crc: u32_be, ver: u32_be]
                0x02 if len >= 13 => {
                    state.target_size = u32::from_be_bytes([rx_buf[1], rx_buf[2], rx_buf[3], rx_buf[4]]);
                    state.target_crc = u32::from_be_bytes([rx_buf[5], rx_buf[6], rx_buf[7], rx_buf[8]]);
                    state.target_ver = u32::from_be_bytes([rx_buf[9], rx_buf[10], rx_buf[11], rx_buf[12]]);

                    defmt::info!(
                        "Start update: size={}, crc=0x{:08x}",
                        state.target_size,
                        state.target_crc
                    );

                    // 1. Invalidate existing metadata sector first
                    let _ = flash.blocking_erase(METADATA_OFFSET, METADATA_OFFSET + ERASE_SIZE as u32);

                    // 2. Erase required sectors for new app
                    let erase_len = (state.target_size as usize).div_ceil(ERASE_SIZE) * ERASE_SIZE;
                    let erase_to = APP_OFFSET + erase_len as u32;
                    let _ = flash.blocking_erase(APP_OFFSET, erase_to);

                    tx_buf[0] = 0x06; // ACK
                }
                // CMD_WRITE_BLOCK (0x03): [0x03, offset: u32_be, block_data: 256 bytes]
                0x03 if len >= 5 => {
                    let offset = u32::from_be_bytes([rx_buf[1], rx_buf[2], rx_buf[3], rx_buf[4]]);
                    let data = &rx_buf[5..len];

                    let flash_offset = APP_OFFSET + offset;
                    // RP2040 Flash page programming requires 256-byte alignment
                    let write_res = flash.blocking_write(flash_offset, data);
                    tx_buf[0] = if write_res.is_ok() { 0x06 } else { 0x15 };
                }
                // CMD_FINALIZE (0x04)
                0x04 => {
                    let app_bytes = unsafe {
                        core::slice::from_raw_parts(APP_BASE as *const u8, state.target_size as usize)
                    };
                    let computed_crc = calculate_crc32(app_bytes);

                    if computed_crc == state.target_crc {
                        defmt::info!("CRC32 verified successfully! Writing metadata...");
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
                        defmt::error!(
                            "CRC32 mismatch: computed 0x{:08x} != expected 0x{:08x}",
                            computed_crc,
                            state.target_crc
                        );
                        tx_buf[0] = 0x15; // NAK
                    }
                }
                // CMD_BOOT_APP (0x05)
                0x05 if is_app_valid() => {
                    unsafe { jump_to_app() };
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
        Err(e) => {
            defmt::warn!("I2C listen error: {:?}", e);
        }
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    defmt::info!("RP2040 I2C Bootloader starting...");

    // Check WATCHDOG_SCRATCH0 for forced bootloader request from app
    let scratch = unsafe { core::ptr::read_volatile(WATCHDOG_SCRATCH0) };
    let forced_bootloader = scratch == BOOTLOADER_MAGIC;
    if forced_bootloader {
        unsafe { core::ptr::write_volatile(WATCHDOG_SCRATCH0, 0) };
        defmt::info!("Watchdog scratch magic detected: staying in bootloader mode!");
    }

    // Initialize Flash driver (2 MB capacity)
    let mut flash =
        Flash::<_, embassy_rp::flash::Blocking, { 2 * 1024 * 1024 }>::new_blocking(p.FLASH);

    // Initialize I2C0 Target on GPIO 4 (SDA) and GPIO 5 (SCL) at address 0x40
    let mut i2c_config = I2cSlaveConfig::default();
    i2c_config.addr = 0x40;

    let mut i2c_slave = I2cSlave::new(
        p.I2C0, p.PIN_5, // SCL
        p.PIN_4, // SDA
        Irqs, i2c_config,
    );

    let app_ready = is_app_valid();
    defmt::info!("Initial app check: valid = {}, forced = {}", app_ready, forced_bootloader);

    let mut rx_buf = [0u8; 300];
    let mut tx_buf = [0u8; 16];
    let mut update_state = UpdateState::default();

    // Boot window: If app is valid and not forced by watchdog, listen for 100ms.
    // If master sends any I2C command, stay in bootloader; otherwise, branch to app!
    if app_ready && !forced_bootloader {
        defmt::info!("Listening for host handshake in 100ms boot window...");
        match select(i2c_slave.listen(&mut rx_buf), Timer::after_millis(100)).await {
            Either::First(cmd_res) => {
                defmt::info!("Handshake command received during boot window! Remaining in bootloader.");
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
            Either::Second(_) => {
                defmt::info!("Boot window timed out without host handshake. Booting application...");
                unsafe { jump_to_app() };
            }
        }
    }

    defmt::info!("Entering I2C Bootloader listener loop on address 0x40");

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
