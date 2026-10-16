//! In-System Programming (ISP) Flasher for the Waveshare RP2040-Zero USB-MIDI coprocessor.
//!
//! Streams the embedded application binary over the internal shared I2C bus (@ 0x40)
//! to the RP2040-Zero secondary bootloader.

use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::i2c::I2cDriver;

/// RP2040-Zero I2C Target address.
pub const RP2040_I2C_ADDR: u8 = 0x40;

/// Embedded RP2040-Zero USB-MIDI application binary (relocated to 0x10004000).
pub static RP2040_APP_BIN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/rp2040-usb-midi.bin"));

/// Firmware version: v1.1.0 (Major: 1, Minor: 1).
pub const RP2040_APP_VERSION: u32 = 0x0001_0001;

/// I2C Bootloader protocol command codes.
pub mod cmd {
    pub const GET_STATUS: u8 = 0x01;
    pub const START_UPDATE: u8 = 0x02;
    pub const WRITE_BLOCK: u8 = 0x03;
    pub const FINALIZE: u8 = 0x04;
    pub const BOOT_APP: u8 = 0x05;
    pub const ENTER_BOOTLOADER: u8 = 0xFB;
}

pub const ACK: u8 = 0x06;
pub const NAK: u8 = 0x15;

#[derive(Debug)]
pub enum FlasherError {
    DeviceNotResponding,
    ProtocolError(&'static str),
    CrcMismatch,
    I2cError,
}

/// Calculate IEEE 802.3 CRC32 of a byte slice.
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

/// Inspect the RP2040 coprocessor state and update its firmware over I2C if necessary.
pub fn check_and_update_rp2040(driver: &mut I2cDriver<'_>) -> Result<bool, FlasherError> {
    let expected_crc = calculate_crc32(RP2040_APP_BIN);
    let expected_size = RP2040_APP_BIN.len() as u32;

    log::info!(
        "[RPE Flasher] Checking RP2040 coprocessor (@ 0x{:02X}). Embedded binary: {} bytes, CRC32: 0x{:08X}",
        RP2040_I2C_ADDR,
        expected_size,
        expected_crc
    );

    // 1. Handshake: query status
    let query = [cmd::GET_STATUS];
    if driver.write(RP2040_I2C_ADDR, &query, 50).is_err() {
        log::info!("[RPE Flasher] RP2040 did not ACK status query (not connected)");
        return Ok(false);
    }

    let mut status_buf = [0u8; 14];
    if driver.read(RP2040_I2C_ADDR, &mut status_buf, 50).is_err() {
        log::warn!("[RPE Flasher] Failed to read status from RP2040");
        return Err(FlasherError::I2cError);
    }

    let mut state = status_buf[0];
    let mut active_crc = u32::from_be_bytes([status_buf[6], status_buf[7], status_buf[8], status_buf[9]]);
    let active_ver = u32::from_be_bytes([status_buf[10], status_buf[11], status_buf[12], status_buf[13]]);

    log::info!(
        "[RPE Flasher] RP2040 Status: state=0x{:02X}, active_ver=0x{:08X}, active_crc=0x{:08X}",
        state,
        active_ver,
        active_crc
    );

    if state == 0xA0 && active_crc == expected_crc {
        log::info!("[RPE Flasher] RP2040 firmware is up to date. No update needed.");
        return Ok(true);
    }

    if state == 0xB0 && active_crc == expected_crc {
        log::info!("[RPE Flasher] RP2040 bootloader detected with up-to-date app. Commanding boot...");
        let boot_cmd = [cmd::BOOT_APP];
        let _ = driver.write(RP2040_I2C_ADDR, &boot_cmd, 50);
        return Ok(true);
    }

    // If application is running but outdated, request reboot into bootloader
    if state == 0xA0 && active_crc != expected_crc {
        log::info!(
            "[RPE Flasher] Outdated app detected (CRC 0x{:08X} != 0x{:08X}). Requesting reboot into bootloader...",
            active_crc,
            expected_crc
        );
        let reboot_cmd = [cmd::ENTER_BOOTLOADER];
        let _ = driver.write(RP2040_I2C_ADDR, &reboot_cmd, 50);
        FreeRtos::delay_ms(300); // Allow RP2040 time to reboot and re-arm I2C target

        // Re-query status with retries (up to 10 retries @ 100ms)
        let mut reboot_ok = false;
        for _ in 0..10 {
            FreeRtos::delay_ms(100);
            if driver.write(RP2040_I2C_ADDR, &query, 50).is_ok()
                && driver.read(RP2040_I2C_ADDR, &mut status_buf, 50).is_ok()
            {
                reboot_ok = true;
                break;
            }
        }
        if !reboot_ok {
            return Err(FlasherError::ProtocolError("RP2040 failed to respond after reboot into bootloader"));
        }
        state = status_buf[0];
        active_crc = u32::from_be_bytes([status_buf[6], status_buf[7], status_buf[8], status_buf[9]]);
        log::info!("[RPE Flasher] Post-reboot status: state=0x{:02X}, active_crc=0x{:08X}", state, active_crc);
    }

    log::info!("[RPE Flasher] Initiating I2C flash sequence...");

    // 2. Start Update: [0x02, size: 4B, crc: 4B, ver: 4B]
    let mut start_cmd = [0u8; 13];
    start_cmd[0] = cmd::START_UPDATE;
    start_cmd[1..5].copy_from_slice(&expected_size.to_be_bytes());
    start_cmd[5..9].copy_from_slice(&expected_crc.to_be_bytes());
    start_cmd[9..13].copy_from_slice(&RP2040_APP_VERSION.to_be_bytes());

    if driver.write(RP2040_I2C_ADDR, &start_cmd, 100).is_err() {
        return Err(FlasherError::I2cError);
    }

    // Wait for flash sector erase (7 sectors @ ~60ms/sector = ~420ms -> 800ms safety margin)
    FreeRtos::delay_ms(800);

    let mut ack_buf = [0u8; 1];
    if driver.read(RP2040_I2C_ADDR, &mut ack_buf, 100).is_err() || ack_buf[0] != ACK {
        return Err(FlasherError::ProtocolError("Start update rejected by bootloader"));
    }

    // 3. Stream binary chunks (RP2040 Flash page size is 256 bytes)
    const CHUNK_SIZE: usize = 256;
    let mut chunk_buf = [0xFFu8; 5 + CHUNK_SIZE];
    chunk_buf[0] = cmd::WRITE_BLOCK;

    for (chunk_idx, chunk) in RP2040_APP_BIN.chunks(CHUNK_SIZE).enumerate() {
        let offset = (chunk_idx * CHUNK_SIZE) as u32;
        chunk_buf[1..5].copy_from_slice(&offset.to_be_bytes());
        // Pad trailing bytes with 0xFF (erased flash state)
        chunk_buf[5..5 + CHUNK_SIZE].fill(0xFF);
        chunk_buf[5..5 + chunk.len()].copy_from_slice(chunk);

        let mut retries = 3;
        loop {
            if driver.write(RP2040_I2C_ADDR, &chunk_buf, 100).is_ok() {
                FreeRtos::delay_ms(2);
                if driver.read(RP2040_I2C_ADDR, &mut ack_buf, 50).is_ok() && ack_buf[0] == ACK {
                    break;
                }
            }
            retries -= 1;
            if retries == 0 {
                return Err(FlasherError::ProtocolError("Block write failed after 3 retries"));
            }
            FreeRtos::delay_ms(5);
        }
    }

    log::info!("[RPE Flasher] All blocks written. Finalizing and verifying CRC32...");

    // 4. Finalize & Verify CRC32
    let finalize_cmd = [cmd::FINALIZE];
    if driver.write(RP2040_I2C_ADDR, &finalize_cmd, 50).is_err() {
        return Err(FlasherError::I2cError);
    }

    FreeRtos::delay_ms(20);

    if driver.read(RP2040_I2C_ADDR, &mut ack_buf, 50).is_err() || ack_buf[0] != ACK {
        return Err(FlasherError::CrcMismatch);
    }

    log::info!("[RPE Flasher] CRC32 verified by bootloader! Booting new firmware...");

    // 5. Boot application
    let boot_cmd = [cmd::BOOT_APP];
    let _ = driver.write(RP2040_I2C_ADDR, &boot_cmd, 20);

    FreeRtos::delay_ms(20);
    log::info!("[RPE Flasher] RP2040 firmware update completed successfully!");

    Ok(true)
}
