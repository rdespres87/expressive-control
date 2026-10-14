//! Theremin Front Panel Hardware Assembly
//!
//! Connects the generic `Mcp23017Driver`, standalone `DebouncedButton`s,
//! and standalone `RotaryEncoder` to the physical pins of the CrowPanel 5.0" ToF Theremin:
//! - `GPA0`: Left ToF XSHUT (Output)
//! - `GPA1`: Right ToF XSHUT (Output)
//! - `GPA4`: Octave + button (Input, Pull-up, Active LOW)
//! - `GPA5`: Octave - button (Input, Pull-up, Active LOW)
//! - `GPA6`: Mute button (Input, Pull-up, Active LOW)
//! - `GPB0`: Rotary Encoder CLK (Input, Pull-up)
//! - `GPB1`: Rotary Encoder DT (Input, Pull-up)
//! - `GPB2`: Rotary Encoder Switch / Click for UI navigation (Input, Pull-up, Active LOW)

use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::i2c::I2cDriver;
use esp_idf_svc::sys::EspError;

use crate::button::DebouncedButton;
use crate::encoder::RotaryEncoder;
use crate::mcp23017::{Direction, Mcp23017Driver, Pin, Port, MCP23017_DEFAULT_ADDR};

/// Aggregated front-panel readings and edge events.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrontPanelEvent {
    pub btn_mute: bool,
    pub btn_octave_up: bool,
    pub btn_octave_down: bool,
    pub encoder_switch: bool,
    pub just_mute: bool,
    pub just_octave_up: bool,
    pub just_octave_down: bool,
    pub just_encoder_switch: bool,
    pub encoder_delta: i32,
}

/// Hardware controller managing the sequential boot and dynamic addressing of dual VL53L4CD sensors.
pub struct DualTofResetController {
    pin_left_xshut: Pin,
    pin_right_xshut: Pin,
}

impl DualTofResetController {
    pub const fn new(pin_left_xshut: Pin, pin_right_xshut: Pin) -> Self {
        Self {
            pin_left_xshut,
            pin_right_xshut,
        }
    }

    /// Configure XSHUT pins as outputs and hold both in hardware shutdown.
    pub fn init(
        &self,
        expander: &mut Mcp23017Driver,
        i2c: &mut I2cDriver,
    ) -> Result<(), EspError> {
        expander.set_pin_direction(i2c, self.pin_left_xshut, Direction::Output)?;
        expander.set_pin_direction(i2c, self.pin_right_xshut, Direction::Output)?;
        expander.write_pin(i2c, self.pin_left_xshut, false)?;
        expander.write_pin(i2c, self.pin_right_xshut, false)
    }

    /// Release Left (Volume) ToF sensor from reset while keeping Right sensor in reset.
    pub fn release_left(
        &self,
        expander: &mut Mcp23017Driver,
        i2c: &mut I2cDriver,
    ) -> Result<(), EspError> {
        log::info!("[DualToF] Releasing Left (Volume) ToF sensor reset line (active HIGH)...");
        expander.write_pin(i2c, self.pin_left_xshut, true)?;
        FreeRtos::delay_ms(5); // Wait for sensor firmware boot (1.2 ms datasheet requirement)
        Ok(())
    }

    /// Release Right (Pitch) ToF sensor from reset.
    pub fn release_right(
        &self,
        expander: &mut Mcp23017Driver,
        i2c: &mut I2cDriver,
    ) -> Result<(), EspError> {
        log::info!("[DualToF] Releasing Right (Pitch) ToF sensor reset line (active HIGH)...");
        expander.write_pin(i2c, self.pin_right_xshut, true)?;
        FreeRtos::delay_ms(5); // Wait for sensor firmware boot
        Ok(())
    }
}

/// Theremin Front Panel hardware manager.
pub struct ThereminFrontPanel {
    pub expander: Mcp23017Driver,
    pub tof_controller: DualTofResetController,
    pub btn_octave_up: DebouncedButton,
    pub btn_octave_down: DebouncedButton,
    pub btn_mute: DebouncedButton,
    pub btn_encoder_sw: DebouncedButton,
    pub encoder: RotaryEncoder,
}

impl Default for ThereminFrontPanel {
    fn default() -> Self {
        Self::new(MCP23017_DEFAULT_ADDR)
    }
}

impl ThereminFrontPanel {
    pub const fn new(addr: u8) -> Self {
        Self {
            expander: Mcp23017Driver::new(addr),
            tof_controller: DualTofResetController::new(Pin::A0, Pin::A1),
            btn_octave_up: DebouncedButton::new(),
            btn_octave_down: DebouncedButton::new(),
            btn_mute: DebouncedButton::new(),
            btn_encoder_sw: DebouncedButton::new(),
            encoder: RotaryEncoder::new(),
        }
    }

    /// Initialize the MCP23017 expander with the Theremin pin direction and pull-up configuration.
    pub fn init(&mut self, i2c: &mut I2cDriver) -> Result<(), EspError> {
        self.expander.init(i2c)?;

        // Port A: GPA0, GPA1 as outputs (0), GPA2..7 as inputs (1) -> 0b1111_1100 = 0xFC
        self.expander.set_port_direction(i2c, Port::A, 0xFC)?;
        // Enable pull-ups on Port A inputs
        self.expander.set_port_pullup(i2c, Port::A, 0xFC)?;

        // Port B: all pins inputs (0xFF) with pull-ups
        self.expander.set_port_direction(i2c, Port::B, 0xFF)?;
        self.expander.set_port_pullup(i2c, Port::B, 0xFF)?;

        // Initialize ToF sensors into hardware reset
        self.tof_controller.init(&mut self.expander, i2c)?;

        // Prime encoder state
        let portb = self.expander.read_port(i2c, Port::B).unwrap_or(0xFF);
        let clk = (portb & Pin::B0.mask()) != 0;
        let dt = (portb & Pin::B1.mask()) != 0;
        self.encoder.reset(clk, dt);

        Ok(())
    }

    /// Read front panel inputs in a single batch I2C transaction and return debounced events.
    pub fn update(&mut self, i2c: &mut I2cDriver) -> Result<FrontPanelEvent, EspError> {
        let porta = self.expander.read_port(i2c, Port::A)?;
        let portb = self.expander.read_port(i2c, Port::B)?;

        // Active LOW inputs: bit = 0 means button is pressed
        let raw_oct_up = (porta & Pin::A4.mask()) == 0;
        let raw_oct_down = (porta & Pin::A5.mask()) == 0;
        let raw_mute = (porta & Pin::A6.mask()) == 0;
        let raw_sw = (portb & Pin::B2.mask()) == 0;

        let oct_up_state = self.btn_octave_up.update(raw_oct_up);
        let oct_down_state = self.btn_octave_down.update(raw_oct_down);
        let mute_state = self.btn_mute.update(raw_mute);
        let sw_state = self.btn_encoder_sw.update(raw_sw);

        let clk = (portb & Pin::B0.mask()) != 0;
        let dt = (portb & Pin::B1.mask()) != 0;
        let encoder_delta = self.encoder.update(clk, dt);

        Ok(FrontPanelEvent {
            btn_mute: mute_state.is_pressed,
            btn_octave_up: oct_up_state.is_pressed,
            btn_octave_down: oct_down_state.is_pressed,
            encoder_switch: sw_state.is_pressed,
            just_mute: mute_state.just_pressed,
            just_octave_up: oct_up_state.just_pressed,
            just_octave_down: oct_down_state.just_pressed,
            just_encoder_switch: sw_state.just_pressed,
            encoder_delta,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_front_panel_event_default_state() {
        let ev = FrontPanelEvent::default();
        assert!(!ev.btn_mute);
        assert!(!ev.btn_octave_up);
        assert!(!ev.btn_octave_down);
        assert!(!ev.encoder_switch);
        assert!(!ev.just_mute);
        assert!(!ev.just_octave_up);
        assert!(!ev.just_octave_down);
        assert!(!ev.just_encoder_switch);
        assert_eq!(ev.encoder_delta, 0);
    }

    #[test]
    fn test_tof_controller_pin_assignments() {
        let ctrl = DualTofResetController::new(Pin::A0, Pin::A1);
        assert_eq!(ctrl.pin_left_xshut, Pin::A0);
        assert_eq!(ctrl.pin_right_xshut, Pin::A1);
    }
}
