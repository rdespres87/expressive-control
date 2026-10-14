//! Theremin Front Panel Hardware Assembly
//!
//! Connects the generic `Mcp23017Driver`, standalone `DebouncedButton`s,
//! and standalone `RotaryEncoder` to the physical pins of the CrowPanel 5.0" ToF Theremin:
//! - `GPA0`: Left ToF XSHUT (Output)
//! - `GPA1`: Right ToF XSHUT (Output)
//! - `GPA4`: Preset Up button (Input, Pull-up, Active LOW)
//! - `GPA5`: Preset Down button (Input, Pull-up, Active LOW)
//! - `GPA6`: Octave shift button (Input, Pull-up, Active LOW)
//! - `GPB0`: Rotary Encoder CLK (Input, Pull-up)
//! - `GPB1`: Rotary Encoder DT (Input, Pull-up)
//! - `GPB2`: Rotary Encoder Switch (Input, Pull-up, Active LOW)

use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::i2c::I2cDriver;
use esp_idf_svc::sys::EspError;

use crate::button::DebouncedButton;
use crate::encoder::RotaryEncoder;
use crate::mcp23017::{Direction, Mcp23017Driver, Pin, Port, MCP23017_DEFAULT_ADDR};

/// Aggregated front-panel readings and edge events.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrontPanelEvent {
    pub btn_preset_up: bool,
    pub btn_preset_down: bool,
    pub btn_octave: bool,
    pub encoder_switch: bool,
    pub just_preset_up: bool,
    pub just_preset_down: bool,
    pub just_octave: bool,
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
    pub btn_preset_up: DebouncedButton,
    pub btn_preset_down: DebouncedButton,
    pub btn_octave: DebouncedButton,
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
            btn_preset_up: DebouncedButton::new(),
            btn_preset_down: DebouncedButton::new(),
            btn_octave: DebouncedButton::new(),
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
        let raw_up = (porta & Pin::A4.mask()) == 0;
        let raw_down = (porta & Pin::A5.mask()) == 0;
        let raw_oct = (porta & Pin::A6.mask()) == 0;
        let raw_sw = (portb & Pin::B2.mask()) == 0;

        let up_state = self.btn_preset_up.update(raw_up);
        let down_state = self.btn_preset_down.update(raw_down);
        let oct_state = self.btn_octave.update(raw_oct);
        let sw_state = self.btn_encoder_sw.update(raw_sw);

        let clk = (portb & Pin::B0.mask()) != 0;
        let dt = (portb & Pin::B1.mask()) != 0;
        let encoder_delta = self.encoder.update(clk, dt);

        Ok(FrontPanelEvent {
            btn_preset_up: up_state.is_pressed,
            btn_preset_down: down_state.is_pressed,
            btn_octave: oct_state.is_pressed,
            encoder_switch: sw_state.is_pressed,
            just_preset_up: up_state.just_pressed,
            just_preset_down: down_state.just_pressed,
            just_octave: oct_state.just_pressed,
            just_encoder_switch: sw_state.just_pressed,
            encoder_delta,
        })
    }
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;

    #[test]
    fn test_front_panel_event_default_state() {
        let ev = FrontPanelEvent::default();
        assert!(!ev.btn_preset_up);
        assert!(!ev.btn_preset_down);
        assert!(!ev.btn_octave);
        assert!(!ev.encoder_switch);
        assert!(!ev.just_preset_up);
        assert!(!ev.just_preset_down);
        assert!(!ev.just_octave);
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
