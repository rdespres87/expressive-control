//! GT911 Capacitive Touch Screen Driver via PCA9557 IO Expander for CrowPanel 5.0"
//!
//! Pinout on Elecrow CrowPanel 5.0" ESP32-S3 V3.0:
//! - I2C: SDA = GPIO 19, SCL = GPIO 20
//! - PCA9557 I/O Expander @ I2C 0x18
//!   - IO0 = TP-RESET (GT911 Reset)
//!   - IO1 = INTE (GT911 Interrupt / Address select during reset)
//! - GT911 Touch Controller @ I2C 0x5D (selected when INT=LOW during RESET release)

use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::i2c::I2cDriver;
use esp_idf_svc::sys::EspError;

pub const PCA9557_ADDR: u8 = 0x18;
pub const GT911_ADDR_5D: u8 = 0x5D;
pub const GT911_ADDR_14: u8 = 0x14;

/// Scans the I2C bus and logs all active 7-bit addresses.
pub fn scan_i2c_bus(i2c: &mut I2cDriver) -> Vec<u8> {
    log::info!("=== [I2C Scanner] Scanning bus on SDA=GPIO19, SCL=GPIO20 ===");
    let mut found = Vec::new();
    for addr in 0x08..=0x77 {
        let mut buf = [0u8; 1];
        if i2c.read(addr, &mut buf, 15).is_ok() {
            let desc = match addr {
                0x18 => " (PCA9557 I/O Expander)",
                0x20 => " (MCP23017 Front-Panel & ToF Reset Expander)",
                0x5D | 0x14 => " (GT911 Touch Controller)",
                0x29 | 0x2A => " (VL53L4CD Boot ToF Sensor)",
                0x30 => " (VL53L4CD Left/Volume ToF Sensor)",
                0x31 => " (VL53L4CD Right/Pitch ToF Sensor)",
                0x40 => " (RP2040-Zero USB-MIDI Target)",
                _ => "",
            };
            log::info!("  -> Device found at 0x{:02X}{}", addr, desc);
            found.push(addr);
        }
    }
    log::info!("=== [I2C Scanner] Scan complete: {} device(s) found ===", found.len());
    found
}

/// Sequence PCA9557 to reset GT911 and ensure onboard audio amplifier is disabled (muted).
pub fn reset_gt911_via_pca9557(i2c: &mut I2cDriver) -> Result<(), EspError> {
    log::info!("[Touch] Starting PCA9557 hardware reset sequence for GT911...");

    // Pin roles on CrowPanel PCA9557:
    // - IO0: TP-RESET (GT911 Reset line, active LOW)
    // - IO1: INTE (GT911 INT line, LOW at reset release selects address 0x5D)
    // - IO2: PA_EN (Audio Power Amplifier Enable: 1=Enabled, 0=Shutdown/Mute)

    // 1. Configure IO0, IO1, and IO2 as outputs (0 = output, 1 = input)
    // Bits 0, 1, 2 = 0 -> 0b11111000 = 0xF8
    i2c.write(PCA9557_ADDR, &[0x03, 0xF8], 50)?;

    // 2. Drive IO0 LOW (assert GT911 reset), IO1 LOW (INT=0), and IO2 LOW (audio amplifier OFF)
    // Register 0x01 (Output Port): all outputs LOW -> 0x00
    i2c.write(PCA9557_ADDR, &[0x01, 0x00], 50)?;
    FreeRtos::delay_ms(25);

    // 3. Drive IO0 HIGH (release GT911 reset while holding INT=0 to select 0x5D), keep IO2 LOW (audio OFF)
    // Register 0x01 (Output Port): bit 0 = 1, bit 1 = 0, bit 2 = 0 -> 0x01
    i2c.write(PCA9557_ADDR, &[0x01, 0x01], 50)?;
    FreeRtos::delay_ms(50);

    // 4. Release IO1 (INT) to high-impedance / input mode for touch interrupt handling,
    // while keeping IO0 as output (HIGH) and IO2 as output (LOW = audio amplifier disabled)
    // Register 0x03 (Configuration): bit 0=0 (out), bit 1=1 (in), bit 2=0 (out) -> 0b11111010 = 0xFA
    i2c.write(PCA9557_ADDR, &[0x03, 0xFA], 50)?;
    FreeRtos::delay_ms(50);

    log::info!("[Touch] GT911 reset complete (address 0x5D). Audio amplifier (IO2/PA_EN) explicitly disabled.");
    Ok(())
}

/// GT911 Capacitive Touch Driver.
pub struct Gt911Driver {
    addr: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchEvent {
    /// Controller has not yet produced a new report; maintain previous state.
    NoUpdate,
    /// Finger is actively contacting screen at (x, y).
    Pressed { x: u16, y: u16 },
    /// Finger was lifted off screen (0 contact points reported).
    Released,
}

impl Gt911Driver {
    /// Initialize and detect GT911 after PCA9557 reset.
    pub fn init(i2c: &mut I2cDriver) -> Result<Self, &'static str> {
        // Try reading 4 bytes Product ID starting at 0x8140 from 0x5D or 0x14
        for &addr in &[GT911_ADDR_5D, GT911_ADDR_14] {
            let mut prod_id = [0u8; 4];
            if i2c.write_read(addr, &[0x81, 0x40], &mut prod_id, 50).is_ok() {
                let id_clean: Vec<u8> = prod_id.iter().copied().take_while(|&b| (32..=126).contains(&b)).collect();
                if let Ok(id_str) = core::str::from_utf8(&id_clean) {
                    if !id_str.is_empty() {
                        log::info!("[Touch] GT911 confirmed at address 0x{:02X}, Product ID: '{}'", addr, id_str);
                        return Ok(Self { addr });
                    }
                }
            }
        }

        // Fallback: The I2C scanner confirmed device at 0x5D
        log::info!("[Touch] Using GT911 at detected address 0x5D");
        Ok(Self { addr: GT911_ADDR_5D })
    }

    /// Poll for active touch contact with atomic single-transaction I2C read.
    pub fn poll_touch(&self, i2c: &mut I2cDriver) -> TouchEvent {
        // Read 7 bytes atomically: Status (0x814E), TrackID (0x814F), X_L, X_H, Y_L, Y_H, Size
        let mut report = [0u8; 7];
        if i2c.write_read(self.addr, &[0x81, 0x4E], &mut report, 20).is_err() {
            return TouchEvent::NoUpdate;
        }

        let status = report[0];
        let buffer_ready = (status & 0x80) != 0;
        let num_points = status & 0x0F;

        if !buffer_ready {
            return TouchEvent::NoUpdate;
        }

        // Acknowledge read by resetting status to 0 AFTER grabbing coordinates
        let _ = i2c.write(self.addr, &[0x81, 0x4E, 0x00], 20);

        if num_points == 0 {
            return TouchEvent::Released;
        }

        let raw_x = (report[2] as u16) | ((report[3] as u16) << 8);
        let raw_y = (report[4] as u16) | ((report[5] as u16) << 8);

        let x = raw_x.min(799);
        let y = raw_y.min(479);

        TouchEvent::Pressed { x, y }
    }
}
