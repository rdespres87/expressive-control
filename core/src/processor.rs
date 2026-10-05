use crate::{Channel, InstrumentEvent, SensorEvent, U14, U7};

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
        event: SensorEvent,
        out_buffer: &mut [InstrumentEvent],
    ) -> Result<usize, ProcessorError> {
        match event {
            SensorEvent::Touch { id, x, y: _, force } => {
                if out_buffer.len() < 2 {
                    return Err(ProcessorError::BufferTooSmall);
                }

                let mut assigned_index = None;
                for i in 0..15 {
                    if self.voices[i].is_none() {
                        assigned_index = Some(i);
                        break;
                    }
                }

                let index = assigned_index.ok_or(ProcessorError::VoiceStealingFailed)?;
                let channel = Channel::new(index as u8 + 1);

                // We use the initial X coordinate as the base MIDI Note (0-127).
                // The hardware abstraction layer (HAL) will be responsible for scaling physical position to a base note.
                let base_note = U7::new(x);

                self.voices[index] = Some(ActiveVoice {
                    touch_id: id,
                    base_note,
                });

                out_buffer[0] = InstrumentEvent::NoteOn {
                    channel,
                    note: base_note,
                    velocity: U7::new(force >> 1),
                };
                out_buffer[1] = InstrumentEvent::ChannelPressure {
                    channel,
                    pressure: U7::new(force >> 1),
                };

                Ok(2)
            }
            SensorEvent::Position { id, x, y } => {
                if out_buffer.len() < 2 {
                    return Err(ProcessorError::BufferTooSmall);
                }

                if let Some(index) = self
                    .voices
                    .iter()
                    .position(|v| v.is_some_and(|active| active.touch_id == id))
                {
                    let channel = Channel::new(index as u8 + 1);

                    // Precise 8-bit to 14-bit scaling:
                    // x << 6 maps the top 8 bits. We fill the lower 6 bits with the top 6 bits of x.
                    let scaled_bend = ((x as u16) << 6) | ((x as u16) >> 2);

                    out_buffer[0] = InstrumentEvent::PitchBend {
                        channel,
                        bend: U14::new(scaled_bend),
                    };
                    out_buffer[1] = InstrumentEvent::Timbre {
                        channel,
                        value: U7::new(y >> 1),
                    };
                    Ok(2)
                } else {
                    Ok(0)
                }
            }
            SensorEvent::Released { id } => {
                if out_buffer.is_empty() {
                    return Err(ProcessorError::BufferTooSmall);
                }

                if let Some(index) = self
                    .voices
                    .iter()
                    .position(|v| v.is_some_and(|active| active.touch_id == id))
                {
                    let channel = Channel::new(index as u8 + 1);
                    let base_note = self.voices[index].unwrap().base_note;

                    self.voices[index] = None;

                    out_buffer[0] = InstrumentEvent::NoteOff {
                        channel,
                        note: base_note,
                        velocity: U7::new(0),
                    };
                    Ok(1)
                } else {
                    Ok(0)
                }
            }
        }
    }
}
