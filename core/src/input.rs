//! Generic Input Processing: Debounced Buttons and Quadrature Encoders.
//!
//! Fully decoupled from hardware: can be driven by physical GPIOs, I2C expanders,
//! coprocessor reports, or software simulation.

/// Generic EC11 Gray-Code Quadrature Rotary Encoder Decoder with Detent Filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RotaryEncoder {
    last_state: u8,
    substep_counter: i8,
}

impl Default for RotaryEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl RotaryEncoder {
    /// Create a new rotary encoder decoder with idle lines high (pull-up default).
    pub const fn new() -> Self {
        Self {
            last_state: 0b11,
            substep_counter: 0,
        }
    }

    /// Reset encoder state to given line readings.
    pub fn reset(&mut self, clk: bool, dt: bool) {
        let clk_bit = if clk { 1 } else { 0 };
        let dt_bit = if dt { 1 } else { 0 };
        self.last_state = (dt_bit << 1) | clk_bit;
        self.substep_counter = 0;
    }

    /// Update encoder state with current CLK (Phase A) and DT (Phase B) line readings.
    /// Returns +1 for clockwise physical detent, -1 for counter-clockwise detent, or 0.
    pub fn update(&mut self, clk: bool, dt: bool) -> i32 {
        let clk_bit = if clk { 1 } else { 0 };
        let dt_bit = if dt { 1 } else { 0 };
        let curr_state = (dt_bit << 1) | clk_bit;

        if curr_state == self.last_state {
            return 0;
        }

        // Standard 2-bit Gray-code transition lookup
        // Clockwise: 11 -> 01 -> 00 -> 10 -> 11
        // Counter-Clockwise: 11 -> 10 -> 00 -> 01 -> 11
        let step = match (self.last_state, curr_state) {
            (0b11, 0b01) | (0b01, 0b00) | (0b00, 0b10) | (0b10, 0b11) => 1,
            (0b11, 0b10) | (0b10, 0b00) | (0b00, 0b01) | (0b01, 0b11) => -1,
            _ => 0, // Illegal transition / contact bounce
        };

        self.last_state = curr_state;
        self.substep_counter += step;

        // When returning to idle resting state (0b11) or completing 4 sub-steps, emit full detent
        if curr_state == 0b11 {
            let detent = if self.substep_counter >= 2 {
                1
            } else if self.substep_counter <= -2 {
                -1
            } else {
                0
            };
            self.substep_counter = 0;
            detent
        } else if self.substep_counter >= 4 {
            self.substep_counter = 0;
            1
        } else if self.substep_counter <= -4 {
            self.substep_counter = 0;
            -1
        } else {
            0
        }
    }
}

/// Generic 3-Sample Integrator Debouncer for tactile switches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DebouncedButton {
    history: u8,
    state: bool,
}

impl Default for DebouncedButton {
    fn default() -> Self {
        Self::new()
    }
}

impl DebouncedButton {
    /// Create a new debouncer starting in the released state.
    pub const fn new() -> Self {
        Self {
            history: 0x00,
            state: false,
        }
    }

    /// Update debouncer with an active status (`is_active == true` means button is physically pressed).
    ///
    /// For active-low physical pins, pass `raw_reading == false`.
    pub fn update(&mut self, is_active: bool) -> ButtonState {
        self.history = (self.history << 1) | (if is_active { 1 } else { 0 });
        let prev = self.state;

        // 3 consecutive 1s (0b0000_0111) -> stable pressed
        if (self.history & 0x07) == 0x07 {
            self.state = true;
        } else if (self.history & 0x07) == 0x00 {
            // 3 consecutive 0s -> stable released
            self.state = false;
        }

        ButtonState {
            is_pressed: self.state,
            just_pressed: !prev && self.state,
            just_released: prev && !self.state,
        }
    }
}

/// Instantaneous and edge-triggered state of a debounced button.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonState {
    pub is_pressed: bool,
    pub just_pressed: bool,
    pub just_released: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_button_debouncer_transitions() {
        let mut btn = DebouncedButton::new();
        assert!(!btn.update(false).is_pressed);

        // 1 active sample is not enough
        assert!(!btn.update(true).is_pressed);
        // 2 active samples is not enough
        assert!(!btn.update(true).is_pressed);
        // 3rd active sample latches pressed
        let state = btn.update(true);
        assert!(state.is_pressed);
        assert!(state.just_pressed);

        // Subsequent active samples stay pressed
        let state2 = btn.update(true);
        assert!(state2.is_pressed);
        assert!(!state2.just_pressed);

        // 3 released samples to unlatch
        btn.update(false);
        btn.update(false);
        let state3 = btn.update(false);
        assert!(!state3.is_pressed);
        assert!(state3.just_released);
    }

    #[test]
    fn test_rotary_encoder_clockwise() {
        let mut enc = RotaryEncoder::new();
        // 11 -> 01 -> 00 -> 10 -> 11
        assert_eq!(enc.update(true, false), 0);
        assert_eq!(enc.update(false, false), 0);
        assert_eq!(enc.update(false, true), 0);
        assert_eq!(enc.update(true, true), 1);
    }

    #[test]
    fn test_rotary_encoder_counter_clockwise() {
        let mut enc = RotaryEncoder::new();
        // 11 -> 10 -> 00 -> 01 -> 11
        assert_eq!(enc.update(false, true), 0);
        assert_eq!(enc.update(false, false), 0);
        assert_eq!(enc.update(true, false), 0);
        assert_eq!(enc.update(true, true), -1);
    }
}
