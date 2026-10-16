//! RP2040 Coprocessor Front-Panel & GPIO Driver.
//!
//! Communicates with the RP2040-Zero ("RPE") at I2C address 0x40 to:
//! 1. Query human interface inputs (rotary encoder delta, button states & click events).
//! 2. Control peripheral GPIOs (ToF sensor XSHUT enable lines).

use esp_idf_hal::i2c::I2cDriver;
use esp_idf_sys::EspError;
use expressive_core::rpe::{cmd, RpeInputReport, RPE_I2C_ADDR};

/// Set the ToF distance sensors XSHUT enable lines via the RP2040 coprocessor.
///
/// - `mask & xshut::LEFT_TOF (0x01)`: 1 = Awake, 0 = In Reset (GP2)
/// - `mask & xshut::RIGHT_TOF (0x02)`: 1 = Awake, 0 = In Reset (GP3)
pub fn set_tof_xshut(driver: &mut I2cDriver<'_>, mask: u8) -> Result<(), EspError> {
    let cmd_buf = [cmd::SET_XSHUT, mask];
    driver.write(RPE_I2C_ADDR, &cmd_buf, 50)
}

/// Poll the latest human interface input report from the RP2040 coprocessor.
///
/// Sends `cmd::READ_INPUTS (0x10)` and reads the 4-byte atomic report.
/// Reading this report atomically clears the encoder accumulator and click flags on the RP2040.
pub fn read_input_report(driver: &mut I2cDriver<'_>) -> Result<RpeInputReport, EspError> {
    let query = [cmd::READ_INPUTS];
    let mut rx_buf = [0u8; 4];
    driver.write_read(RPE_I2C_ADDR, &query, &mut rx_buf, 20)?;
    Ok(RpeInputReport::from_bytes(&rx_buf))
}

#[cfg(test)]
mod tests {
    use super::*;
    use expressive_core::rpe::buttons;

    #[test]
    fn test_report_decoding() {
        let raw = [(-2i8) as u8, buttons::OCTAVE_DOWN, buttons::OCTAVE_DOWN, 0x01];
        let report = RpeInputReport::from_bytes(&raw);
        assert_eq!(report.encoder_delta, -2);
        assert!(report.is_octave_down_pressed());
        assert!(report.has_button_clicked(buttons::OCTAVE_DOWN));
        assert!(!report.is_octave_up_pressed());
        assert!(report.is_usb_configured());
    }
}
