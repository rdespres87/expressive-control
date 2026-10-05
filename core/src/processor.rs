use crate::{InstrumentEvent, SensorEvent, U7};

/// Represents a currently active MPE touch.
/// Note: The MPE channel is deliberately excluded to save memory.
/// It is inherently derived from the `ActiveVoice`'s index within the `MpeInstrument` array
/// (where index `i` corresponds to MPE member channel `i + 1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveVoice {
    pub touch_id: u16,
    pub base_note: U7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessorError {
    VoiceStealingFailed,
    BufferTooSmall,
}

pub trait ControlProcessor {
    /// Processes a single sensor event.
    /// Writes the resulting MPE events into `out_buffer`.
    /// Returns the number of events written to avoid dynamic allocation.
    fn process_event(
        &mut self,
        event: SensorEvent,
        out_buffer: &mut [InstrumentEvent],
    ) -> Result<usize, ProcessorError>;
}

pub struct MpeInstrument {
    /// Fixed array of 15 voices (for MPE channels 2 through 16).
    /// Using Option<T> leverages Rust's niche optimization and prevents
    /// accidental reads of inactive state variables.
    pub voices: [Option<ActiveVoice>; 15],
    pub pitch_bend_range: u8,
}

impl Default for MpeInstrument {
    fn default() -> Self {
        Self {
            voices: [None; 15],
            pitch_bend_range: 48, // Standard MPE pitch bend range (48 semitones)
        }
    }
}

impl ControlProcessor for MpeInstrument {
    fn process_event(
        &mut self,
        _event: SensorEvent,
        _out_buffer: &mut [InstrumentEvent],
    ) -> Result<usize, ProcessorError> {
        // Implementation logic will be added here
        Ok(0)
    }
}
