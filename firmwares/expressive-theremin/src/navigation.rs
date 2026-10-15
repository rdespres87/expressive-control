//! Physical Navigation & Focus Management for Expressive Theremin UI
//!
//! Provides a deterministic, testable state machine managing:
//! - Dual-mode rotary encoder navigation: `Navigating` (focus cycling) vs `Editing` (value adjustment).
//! - Focus indices mapping to Slint MasterView cards.
//! - Tuner reference frequency calibration (A440 ± 10 Hz).
//! - Hardware button shortcuts (Octave Up/Down, Mute toggle).

use expressive_core::processor::{FilterSpeed, RootNote, Scale};

/// Available focusable cards in MasterView.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum FocusedCard {
    RootNote = 0,
    Scale = 1,
    BaseOctave = 2,
    OctaveRange = 3,
    PbRange = 4,
    MidiMode = 5,
    Quantization = 6,
    FilterSpeed = 7,
    TunerSwitch = 8,
}

impl FocusedCard {
    pub const COUNT: u8 = 9;

    pub const fn from_index(idx: u8) -> Self {
        match idx % Self::COUNT {
            0 => Self::RootNote,
            1 => Self::Scale,
            2 => Self::BaseOctave,
            3 => Self::OctaveRange,
            4 => Self::PbRange,
            5 => Self::MidiMode,
            6 => Self::Quantization,
            7 => Self::FilterSpeed,
            _ => Self::TunerSwitch,
        }
    }

    pub const fn as_index(self) -> u8 {
        self as u8
    }

    pub fn next(self) -> Self {
        Self::from_index((self as u8 + 1) % Self::COUNT)
    }

    pub fn prev(self) -> Self {
        Self::from_index((self as u8 + Self::COUNT - 1) % Self::COUNT)
    }
}

/// Navigation interaction mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavigationMode {
    /// Rotating encoder cycles through parameter cards (Gold outline).
    Navigating,
    /// Rotating encoder adjusts the focused parameter value (Cyan outline).
    Editing,
}

/// Commands emitted by the navigation state machine to be applied to Slint UI and Audio Core.
#[derive(Clone, Debug, PartialEq)]
pub enum NavigationAction {
    None,
    SwitchToTuner,
    SwitchToMaster,
    UpdateRoot { note: RootNote, label: &'static str },
    UpdateScale { scale: Scale, label: &'static str },
    UpdateOctave(i32),
    UpdateOctaveRange(i32),
    UpdatePbRange(i32),
    UpdateMidiMode(bool), // true = MPE, false = Standard
    UpdateQuantization(i32),
    UpdateFilterSpeed { speed: FilterSpeed, label: &'static str },
    UpdateReferencePitch(f32),
    ToggleMute(bool),
}

/// State machine tracking physical UI navigation.
#[derive(Clone, Debug)]
pub struct NavigationController {
    pub mode: NavigationMode,
    pub focused_card: FocusedCard,
    pub is_muted: bool,
    pub ref_pitch_hz: f32, // Default 440.0 Hz (clamped between 430.0 and 450.0 Hz)
    
    // Internal mirrored parameter states for stepping
    pub root_idx: u8,       // 0..11 (C..B)
    pub scale_idx: u8,      // 0..3 (Chromatic, Major, Minor, Pentatonic)
    pub octave: i32,        // 0..8
    pub octave_range: i32,  // 1..4
    pub pb_range_idx: u8,   // 0=2, 1=12, 2=24, 3=48
    pub is_mpe: bool,       // false = Standard, true = MPE
    pub quantization: i32,  // 0, 25, 50, 75, 100
    pub filter_speed_idx: u8, // 0=Slow, 1=Med, 2=Fast
}

impl Default for NavigationController {
    fn default() -> Self {
        Self {
            mode: NavigationMode::Navigating,
            focused_card: FocusedCard::RootNote,
            is_muted: false,
            ref_pitch_hz: 440.0,
            root_idx: 0, // C
            scale_idx: 0, // Chromatic
            octave: 4,
            octave_range: 2,
            pb_range_idx: 0, // 2 semitones
            is_mpe: false,
            quantization: 50,
            filter_speed_idx: 2, // Fast
        }
    }
}

/// Parameters used to synchronize NavigationController with Slint UI properties.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NavigationStateSync {
    pub root_idx: u8,
    pub scale_idx: u8,
    pub octave: i32,
    pub octave_range: i32,
    pub pb_range: i32,
    pub is_mpe: bool,
    pub quantization: i32,
    pub filter_speed_idx: u8,
}

impl NavigationController {
    pub const PB_RANGES: [i32; 4] = [2, 12, 24, 48];
    pub const ROOTS: [(RootNote, &'static str); 12] = [
        (RootNote::C, "C"),
        (RootNote::Cs, "C#"),
        (RootNote::D, "D"),
        (RootNote::Ds, "D#"),
        (RootNote::E, "E"),
        (RootNote::F, "F"),
        (RootNote::Fs, "F#"),
        (RootNote::G, "G"),
        (RootNote::Gs, "G#"),
        (RootNote::A, "A"),
        (RootNote::As, "A#"),
        (RootNote::B, "B"),
    ];
    pub const SCALES: [(Scale, &'static str); 4] = [
        (Scale::Chromatic, "Chromatic"),
        (Scale::Major, "Major"),
        (Scale::Minor, "Minor"),
        (Scale::Pentatonic, "Penta"),
    ];
    pub const FILTER_SPEEDS: [(FilterSpeed, &'static str); 3] = [
        (FilterSpeed::Slow, "Slow"),
        (FilterSpeed::Med, "Medium"),
        (FilterSpeed::Fast, "Fast"),
    ];

    /// Synchronize local state with values coming from Slint or Core defaults.
    pub fn sync_state(&mut self, sync: NavigationStateSync) {
        self.root_idx = sync.root_idx % 12;
        self.scale_idx = sync.scale_idx % 4;
        self.octave = sync.octave.clamp(0, 8);
        self.octave_range = sync.octave_range.clamp(1, 4);
        self.pb_range_idx = match sync.pb_range {
            12 => 1,
            24 => 2,
            48 => 3,
            _ => 0,
        };
        self.is_mpe = sync.is_mpe;
        self.quantization = sync.quantization.clamp(0, 100);
        self.filter_speed_idx = sync.filter_speed_idx % 3;
    }

    /// Handle rotary encoder rotation.
    pub fn handle_encoder_delta(&mut self, delta: i8, is_master_view: bool) -> NavigationAction {
        if delta == 0 {
            return NavigationAction::None;
        }

        if !is_master_view {
            // Tuner view: adjust reference pitch (0.5 Hz per tick)
            let new_ref = (self.ref_pitch_hz + (delta as f32) * 0.5).clamp(430.0, 450.0);
            self.ref_pitch_hz = new_ref;
            return NavigationAction::UpdateReferencePitch(new_ref);
        }

        match self.mode {
            NavigationMode::Navigating => {
                if delta > 0 {
                    for _ in 0..delta {
                        self.focused_card = self.focused_card.next();
                    }
                } else {
                    for _ in 0..(-delta) {
                        self.focused_card = self.focused_card.prev();
                    }
                }
                NavigationAction::None
            }
            NavigationMode::Editing => {
                match self.focused_card {
                    FocusedCard::RootNote => {
                        let new_idx = if delta > 0 {
                            (self.root_idx + delta as u8) % 12
                        } else {
                            (self.root_idx as i16 + delta as i16).rem_euclid(12) as u8
                        };
                        self.root_idx = new_idx;
                        let (note, label) = Self::ROOTS[new_idx as usize];
                        NavigationAction::UpdateRoot { note, label }
                    }
                    FocusedCard::Scale => {
                        let new_idx = if delta > 0 {
                            (self.scale_idx + delta as u8) % 4
                        } else {
                            (self.scale_idx as i16 + delta as i16).rem_euclid(4) as u8
                        };
                        self.scale_idx = new_idx;
                        let (scale, label) = Self::SCALES[new_idx as usize];
                        NavigationAction::UpdateScale { scale, label }
                    }
                    FocusedCard::BaseOctave => {
                        let new_oct = (self.octave + delta as i32).clamp(0, 8);
                        self.octave = new_oct;
                        NavigationAction::UpdateOctave(new_oct)
                    }
                    FocusedCard::OctaveRange => {
                        let new_range = (self.octave_range + delta as i32).clamp(1, 4);
                        self.octave_range = new_range;
                        NavigationAction::UpdateOctaveRange(new_range)
                    }
                    FocusedCard::PbRange => {
                        let new_idx = if delta > 0 {
                            (self.pb_range_idx + 1).min(3)
                        } else {
                            self.pb_range_idx.saturating_sub(1)
                        };
                        self.pb_range_idx = new_idx;
                        NavigationAction::UpdatePbRange(Self::PB_RANGES[new_idx as usize])
                    }
                    FocusedCard::MidiMode => {
                        self.is_mpe = !self.is_mpe;
                        NavigationAction::UpdateMidiMode(self.is_mpe)
                    }
                    FocusedCard::Quantization => {
                        // Steps of 25: 0, 25, 50, 75, 100
                        let step = (delta as i32) * 25;
                        let new_q = (self.quantization + step).clamp(0, 100);
                        self.quantization = new_q;
                        NavigationAction::UpdateQuantization(new_q)
                    }
                    FocusedCard::FilterSpeed => {
                        let new_idx = if delta > 0 {
                            (self.filter_speed_idx + 1) % 3
                        } else {
                            (self.filter_speed_idx + 2) % 3
                        };
                        self.filter_speed_idx = new_idx;
                        let (speed, label) = Self::FILTER_SPEEDS[new_idx as usize];
                        NavigationAction::UpdateFilterSpeed { speed, label }
                    }
                    FocusedCard::TunerSwitch => {
                        // Cannot edit the tuner switch card, return to navigation
                        self.mode = NavigationMode::Navigating;
                        NavigationAction::None
                    }
                }
            }
        }
    }

    /// Handle rotary encoder click / push-button.
    pub fn handle_encoder_click(&mut self, is_master_view: bool) -> NavigationAction {
        if !is_master_view {
            // In Tuner view, clicking encoder returns to MasterView
            return NavigationAction::SwitchToMaster;
        }

        if self.focused_card == FocusedCard::TunerSwitch {
            // Clicking on the TunerSwitch card transitions to Tuner view
            self.mode = NavigationMode::Navigating;
            return NavigationAction::SwitchToTuner;
        }

        // Toggle between Navigating and Editing mode
        self.mode = match self.mode {
            NavigationMode::Navigating => NavigationMode::Editing,
            NavigationMode::Editing => NavigationMode::Navigating,
        };

        NavigationAction::None
    }

    /// Handle dedicated hardware Octave + button.
    pub fn handle_octave_up(&mut self) -> NavigationAction {
        if self.octave < 8 {
            self.octave += 1;
            NavigationAction::UpdateOctave(self.octave)
        } else {
            NavigationAction::None
        }
    }

    /// Handle dedicated hardware Octave - button.
    pub fn handle_octave_down(&mut self) -> NavigationAction {
        if self.octave > 0 {
            self.octave -= 1;
            NavigationAction::UpdateOctave(self.octave)
        } else {
            NavigationAction::None
        }
    }

    /// Handle dedicated hardware Mute button.
    pub fn handle_mute_toggle(&mut self) -> NavigationAction {
        self.is_muted = !self.is_muted;
        NavigationAction::ToggleMute(self.is_muted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_focused_card_cycling() {
        let mut card = FocusedCard::RootNote;
        assert_eq!(card.as_index(), 0);

        card = card.next();
        assert_eq!(card, FocusedCard::Scale);

        card = card.prev();
        assert_eq!(card, FocusedCard::RootNote);

        // Wrap around backward
        card = card.prev();
        assert_eq!(card, FocusedCard::TunerSwitch);
        assert_eq!(card.as_index(), 8);

        // Wrap around forward
        card = card.next();
        assert_eq!(card, FocusedCard::RootNote);
    }

    #[test]
    fn test_navigation_mode_toggle() {
        let mut nav = NavigationController::default();
        assert_eq!(nav.mode, NavigationMode::Navigating);

        // Click on RootNote card -> enters Editing mode
        let action = nav.handle_encoder_click(true);
        assert_eq!(action, NavigationAction::None);
        assert_eq!(nav.mode, NavigationMode::Editing);

        // Click again -> returns to Navigating mode
        let action = nav.handle_encoder_click(true);
        assert_eq!(action, NavigationAction::None);
        assert_eq!(nav.mode, NavigationMode::Navigating);
    }

    #[test]
    fn test_encoder_rotation_in_navigating_mode() {
        let mut nav = NavigationController::default();
        assert_eq!(nav.focused_card, FocusedCard::RootNote);

        // Rotate +1 -> Scale
        nav.handle_encoder_delta(1, true);
        assert_eq!(nav.focused_card, FocusedCard::Scale);

        // Rotate +2 -> OctaveRange
        nav.handle_encoder_delta(2, true);
        assert_eq!(nav.focused_card, FocusedCard::OctaveRange);

        // Rotate -1 -> BaseOctave
        nav.handle_encoder_delta(-1, true);
        assert_eq!(nav.focused_card, FocusedCard::BaseOctave);
    }

    #[test]
    fn test_encoder_editing_root_note() {
        let mut nav = NavigationController::default();
        nav.focused_card = FocusedCard::RootNote;
        nav.handle_encoder_click(true); // Enter Editing

        // Rotate +1 from C -> C#
        let action = nav.handle_encoder_delta(1, true);
        assert_eq!(action, NavigationAction::UpdateRoot { note: RootNote::Cs, label: "C#" });

        // Rotate -1 -> C
        let action = nav.handle_encoder_delta(-1, true);
        assert_eq!(action, NavigationAction::UpdateRoot { note: RootNote::C, label: "C" });

        // Rotate -1 -> B (wrap)
        let action = nav.handle_encoder_delta(-1, true);
        assert_eq!(action, NavigationAction::UpdateRoot { note: RootNote::B, label: "B" });
    }

    #[test]
    fn test_encoder_editing_octave() {
        let mut nav = NavigationController::default();
        nav.focused_card = FocusedCard::BaseOctave;
        nav.octave = 4;
        nav.handle_encoder_click(true); // Enter Editing

        let action = nav.handle_encoder_delta(1, true);
        assert_eq!(action, NavigationAction::UpdateOctave(5));

        let action = nav.handle_encoder_delta(-2, true);
        assert_eq!(action, NavigationAction::UpdateOctave(3));
    }

    #[test]
    fn test_tuner_view_interaction() {
        let mut nav = NavigationController::default();
        nav.ref_pitch_hz = 440.0;

        // In Tuner view (is_master_view = false): delta adjusts ref pitch
        let action = nav.handle_encoder_delta(2, false);
        assert_eq!(action, NavigationAction::UpdateReferencePitch(441.0));
        assert_eq!(nav.ref_pitch_hz, 441.0);

        // In Tuner view: click switches to MasterView
        let action = nav.handle_encoder_click(false);
        assert_eq!(action, NavigationAction::SwitchToMaster);
    }

    #[test]
    fn test_hardware_buttons() {
        let mut nav = NavigationController::default();
        nav.octave = 4;
        nav.is_muted = false;

        assert_eq!(nav.handle_octave_up(), NavigationAction::UpdateOctave(5));
        assert_eq!(nav.handle_octave_down(), NavigationAction::UpdateOctave(4));
        assert_eq!(nav.handle_mute_toggle(), NavigationAction::ToggleMute(true));
        assert_eq!(nav.handle_mute_toggle(), NavigationAction::ToggleMute(false));
    }
}
