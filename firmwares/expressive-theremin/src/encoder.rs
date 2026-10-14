//! Generic EC11 Quadrature Rotary Encoder Decoder
//!
//! Implements a 2-bit Gray-code state machine with sub-step accumulation and
//! detent filtering. Emits ±1 per mechanical detent (4 sub-steps per notch).
//! Rejects contact bounce and illegal transitions.
//!
//! Fully decoupled from hardware: can be driven by MCP23017 pins, onboard GPIOs,
//! or software simulation.

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
            self.substep_counter -= 4;
            1
        } else if self.substep_counter <= -4 {
            self.substep_counter += 4;
            -1
        } else {
            0
        }
    }
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;

    #[test]
    fn test_rotary_encoder_full_clockwise_cycle() {
        let mut enc = RotaryEncoder::new();

        // Resting state: 11
        // Step 1: 01 (CW)
        assert_eq!(enc.update(true, false), 0);
        // Step 2: 00 (CW)
        assert_eq!(enc.update(false, false), 0);
        // Step 3: 10 (CW)
        assert_eq!(enc.update(false, true), 0);
        // Step 4: 11 (CW return to resting state -> emit 1 detent)
        assert_eq!(enc.update(true, true), 1);
    }

    #[test]
    fn test_rotary_encoder_full_counter_clockwise_cycle() {
        let mut enc = RotaryEncoder::new();

        // Resting state: 11
        // Step 1: 10 (CCW)
        assert_eq!(enc.update(false, true), 0);
        // Step 2: 00 (CCW)
        assert_eq!(enc.update(false, false), 0);
        // Step 3: 01 (CCW)
        assert_eq!(enc.update(true, false), 0);
        // Step 4: 11 (CCW return to resting state -> emit -1 detent)
        assert_eq!(enc.update(true, true), -1);
    }

    #[test]
    fn test_rotary_encoder_bounce_jump_rejection() {
        let mut enc = RotaryEncoder::new();

        // Illegal diagonal jump directly from 11 to 00
        assert_eq!(enc.update(false, false), 0);
        // Returns to 11
        assert_eq!(enc.update(true, true), 0);
        // Total delta must remain 0
    }
}
