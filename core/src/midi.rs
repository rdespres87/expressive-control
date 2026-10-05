use crate::InstrumentEvent;

/// A stack-allocated buffer for a single MIDI 1.0 message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MidiMessage {
    pub bytes: [u8; 3],
    pub len: usize,
}

impl MidiMessage {
    /// Returns the exact slice of valid bytes for this message.
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

pub trait ToMidi {
    /// Converts the structure into raw MIDI 1.0 bytes.
    fn to_midi(&self) -> MidiMessage;
}

impl ToMidi for InstrumentEvent {
    fn to_midi(&self) -> MidiMessage {
        match self {
            InstrumentEvent::NoteOn {
                channel,
                note,
                velocity,
            } => MidiMessage {
                bytes: [
                    0x90 | (channel.as_u8() & 0x0F),
                    note.as_u8() & 0x7F,
                    velocity.as_u8() & 0x7F,
                ],
                len: 3,
            },
            InstrumentEvent::NoteOff {
                channel,
                note,
                velocity,
            } => MidiMessage {
                bytes: [
                    0x80 | (channel.as_u8() & 0x0F),
                    note.as_u8() & 0x7F,
                    velocity.as_u8() & 0x7F,
                ],
                len: 3,
            },
            InstrumentEvent::ChannelPressure { channel, pressure } => MidiMessage {
                bytes: [0xD0 | (channel.as_u8() & 0x0F), pressure.as_u8() & 0x7F, 0],
                len: 2, // The 3rd byte is unused and ignored due to len: 2
            },
            InstrumentEvent::PitchBend { channel, bend } => {
                let val = bend.as_u16();
                MidiMessage {
                    bytes: [
                        0xE0 | (channel.as_u8() & 0x0F),
                        (val & 0x7F) as u8,        // LSB: lower 7 bits
                        ((val >> 7) & 0x7F) as u8, // MSB: upper 7 bits
                    ],
                    len: 3,
                }
            }
            InstrumentEvent::Timbre { channel, value } => MidiMessage {
                // CC74 (Control Change 74)
                bytes: [0xB0 | (channel.as_u8() & 0x0F), 74, value.as_u8() & 0x7F],
                len: 3,
            },
        }
    }
}
