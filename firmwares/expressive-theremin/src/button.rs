//! Generic Debounced Pushbutton Driver
//!
//! Provides a stateful 3-sample integrator debouncer with edge detection
//! (`is_pressed`, `just_pressed`, `just_released`).
//!
//! Fully decoupled from hardware: can be driven by MCP23017 pins, onboard GPIOs,
//! or software simulation.

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
#[allow(unused_imports)]
mod tests {
    use super::*;

    #[test]
    fn test_debounced_button_noise_glitch_rejection() {
        let mut btn = DebouncedButton::new();

        // 1-sample noise glitch (e.g. electromagnetic spike)
        let s1 = btn.update(true);
        assert!(!s1.is_pressed);
        assert!(!s1.just_pressed);

        // Immediate return to false
        let s2 = btn.update(false);
        assert!(!s2.is_pressed);
        assert!(!s2.just_pressed);
    }

    #[test]
    fn test_debounced_button_sustained_press_and_release() {
        let mut btn = DebouncedButton::new();

        // Needs 3 consecutive active samples to latch pressed
        assert!(!btn.update(true).is_pressed);
        assert!(!btn.update(true).is_pressed);
        let latched = btn.update(true);
        assert!(latched.is_pressed);
        assert!(latched.just_pressed);

        // Sustained: still pressed, but no new just_pressed trigger
        let sustained = btn.update(true);
        assert!(sustained.is_pressed);
        assert!(!sustained.just_pressed);

        // Release: needs 3 consecutive false samples to unlatch
        assert!(btn.update(false).is_pressed);
        assert!(btn.update(false).is_pressed);
        let released = btn.update(false);
        assert!(!released.is_pressed);
        assert!(released.just_released);
    }
}
