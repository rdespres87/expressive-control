use expressive_core::InstrumentEvent;
use theremin::TofThereminProcessor;

/// Pitch-bend value meaning "no bend".
pub const PITCH_BEND_CENTER: u16 = 8192;
/// Maximum 14-bit pitch-bend value.
pub const PITCH_BEND_MAX: u16 = 16383;
/// Maximum 7-bit MIDI value (velocity, pressure).
pub const MIDI_VALUE_MAX: u8 = 127;

const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// Strings and gauges displayed by the Slint `MainWindow`.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplaySnapshot {
    pub pitch_note: String,
    pub pitch_hz: String,
    pub pitch_cents: f32,
    pub pitch_progress: f32,
    pub volume_text: String,
    pub volume_percent: String,
    pub volume_progress: f32,
}

/// Tracks the theremin state from the MIDI events it emits, independently of any
/// UI toolkit so it can be unit-tested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThereminDisplay {
    base_note: u8,
    bend_range_semitones: u8,
    bend: u16,
    volume: u8,
}

impl ThereminDisplay {
    pub fn new(base_note: u8, bend_range_semitones: u8) -> Self {
        Self {
            base_note,
            bend_range_semitones,
            bend: PITCH_BEND_CENTER,
            volume: 0,
        }
    }

    /// Mirrors the tuning of a theremin processor.
    pub fn from_processor(processor: &TofThereminProcessor) -> Self {
        Self::new(
            processor.base_note.as_u8(),
            processor.pitch_bend_range_semitones,
        )
    }

    /// Updates the state from an instrument event. Returns `true` if the display changed.
    pub fn apply(&mut self, event: &InstrumentEvent) -> bool {
        let before = *self;
        match *event {
            InstrumentEvent::NoteOn { note, velocity, .. } => {
                self.base_note = note.as_u8();
                self.volume = velocity.as_u8();
            }
            InstrumentEvent::NoteOff { .. } => self.volume = 0,
            InstrumentEvent::PitchBend { bend, .. } => self.bend = bend.as_u16(),
            InstrumentEvent::ChannelPressure { pressure, .. } => self.volume = pressure.as_u8(),
            InstrumentEvent::Timbre { .. } => {}
        }
        *self != before
    }

    /// Pitch-bend offset in (fractional) semitones.
    pub fn semitone_offset(&self) -> f32 {
        let normalized =
            (f32::from(self.bend) - f32::from(PITCH_BEND_CENTER)) / f32::from(PITCH_BEND_CENTER);
        normalized * f32::from(self.bend_range_semitones)
    }

    /// Current pitch as a fractional MIDI note number.
    pub fn fractional_note(&self) -> f32 {
        f32::from(self.base_note) + self.semitone_offset()
    }

    /// Current pitch in Hz (A4 = MIDI 69 = 440 Hz).
    pub fn frequency_hz(&self) -> f32 {
        440.0 * 2f32.powf((self.fractional_note() - 69.0) / 12.0)
    }

    pub fn snapshot(&self) -> DisplaySnapshot {
        let volume_ratio = f32::from(self.volume) / f32::from(MIDI_VALUE_MAX);
        let fractional = self.fractional_note();
        let cents = (fractional - fractional.round()) * 100.0;

        DisplaySnapshot {
            pitch_note: note_name(fractional),
            pitch_hz: format!("{:.1} Hz", self.frequency_hz()),
            pitch_cents: cents,
            pitch_progress: f32::from(self.bend) / f32::from(PITCH_BEND_MAX),
            volume_text: match volume_db(self.volume) {
                Some(db) => format!("{db:.1} dB"),
                None => "-inf dB".to_string(),
            },
            volume_percent: format!("{:.0}%", volume_ratio * 100.0),
            volume_progress: volume_ratio,
        }
    }
}

/// Name of the nearest MIDI note, e.g. `60.2` -> `"C4"`. Out-of-range values are clamped.
pub fn note_name(note: f32) -> String {
    // `as u8` saturates (and maps NaN to 0), the clamp keeps us within MIDI range.
    let midi = note.round().clamp(0.0, 127.0) as u8;
    let name = NOTE_NAMES
        .get(usize::from(midi % 12))
        .copied()
        .unwrap_or("?");
    let octave = i16::from(midi / 12) - 1;
    format!("{name}{octave}")
}

/// Gain in dB relative to full scale, `None` for silence.
pub fn volume_db(volume: u8) -> Option<f32> {
    if volume == 0 {
        return None;
    }
    Some(20.0 * (f32::from(volume) / f32::from(MIDI_VALUE_MAX)).log10())
}

#[cfg(test)]
mod tests {
    use super::*;
    use expressive_core::{Channel, U14, U7};

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.05
    }

    fn bend(value: u16) -> InstrumentEvent {
        InstrumentEvent::PitchBend {
            channel: Channel::new(1),
            bend: U14::new(value),
        }
    }

    fn pressure(value: u8) -> InstrumentEvent {
        InstrumentEvent::ChannelPressure {
            channel: Channel::new(1),
            pressure: U7::new(value),
        }
    }

    #[test]
    fn note_names() {
        assert_eq!(note_name(60.0), "C4");
        assert_eq!(note_name(69.0), "A4");
        assert_eq!(note_name(61.4), "C#4");
        assert_eq!(note_name(61.6), "D4");
        assert_eq!(note_name(0.0), "C-1");
        assert_eq!(note_name(127.0), "G9");
    }

    #[test]
    fn note_name_clamps_out_of_range_values() {
        assert_eq!(note_name(-12.0), "C-1");
        assert_eq!(note_name(500.0), "G9");
        assert_eq!(note_name(f32::NAN), "C-1");
    }

    #[test]
    fn volume_in_db() {
        assert_eq!(volume_db(0), None);
        assert!(volume_db(127).is_some_and(|db| close(db, 0.0)));
        assert!(volume_db(64).is_some_and(|db| close(db, -5.95)));
    }

    #[test]
    fn initial_state_matches_default_processor() {
        let display = ThereminDisplay::from_processor(&TofThereminProcessor::default());
        let snap = display.snapshot();
        assert_eq!(snap.pitch_note, "C4");
        assert_eq!(snap.pitch_hz, "261.6 Hz");
        assert!(close(snap.pitch_progress, 0.5));
        assert_eq!(snap.volume_text, "-inf dB");
        assert_eq!(snap.volume_percent, "0%");
        assert!(close(snap.volume_progress, 0.0));
    }

    #[test]
    fn full_bend_reaches_two_octaves_up() {
        let mut display = ThereminDisplay::new(60, 24);
        assert!(display.apply(&bend(PITCH_BEND_MAX)));
        assert!(close(display.semitone_offset(), 24.0));
        let snap = display.snapshot();
        assert_eq!(snap.pitch_note, "C6");
        // 8191/8192 of the range: a fraction of a cent below C6 (1046.5 Hz).
        assert!((display.frequency_hz() - 1046.5).abs() < 0.5);
        assert!(close(snap.pitch_progress, 1.0));
    }

    #[test]
    fn pressure_updates_volume() {
        let mut display = ThereminDisplay::new(60, 24);
        assert!(display.apply(&pressure(127)));
        let snap = display.snapshot();
        assert_eq!(snap.volume_text, "0.0 dB");
        assert_eq!(snap.volume_percent, "100%");
        assert!(close(snap.volume_progress, 1.0));
        // Same value again: nothing to redraw.
        assert!(!display.apply(&pressure(127)));
    }

    #[test]
    fn note_on_and_off() {
        let mut display = ThereminDisplay::new(60, 24);
        assert!(display.apply(&InstrumentEvent::NoteOn {
            channel: Channel::new(1),
            note: U7::new(69),
            velocity: U7::new(100),
        }));
        assert_eq!(display.snapshot().pitch_note, "A4");
        assert_eq!(display.snapshot().pitch_hz, "440.0 Hz");

        assert!(display.apply(&InstrumentEvent::NoteOff {
            channel: Channel::new(1),
            note: U7::new(69),
            velocity: U7::new(0),
        }));
        assert_eq!(display.snapshot().volume_text, "-inf dB");
    }

    #[test]
    fn timbre_is_ignored() {
        let mut display = ThereminDisplay::new(60, 24);
        assert!(!display.apply(&InstrumentEvent::Timbre {
            channel: Channel::new(1),
            value: U7::new(10),
        }));
    }
}
