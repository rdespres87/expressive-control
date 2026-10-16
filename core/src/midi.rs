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

/// A 4-byte USB-MIDI 1.0 event packet as defined by the USB-MIDI Class Specification (Table 4-1).
/// `[header, midi_byte0, midi_byte1, midi_byte2]`
/// where `header = (cable_number << 4) | code_index_number`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsbMidiPacket(pub [u8; 4]);

impl UsbMidiPacket {
    /// Construct a USB-MIDI event packet from raw MIDI parameters.
    pub const fn from_midi(cable: u8, status: u8, data1: u8, data2: u8) -> Self {
        let cin = match status & 0xF0 {
            0x80 => 0x08, // NoteOff (3 bytes)
            0x90 => 0x09, // NoteOn (3 bytes)
            0xA0 => 0x0A, // Poly KeyPressure (3 bytes)
            0xB0 => 0x0B, // Control Change (3 bytes)
            0xC0 => 0x0C, // Program Change (2 bytes)
            0xD0 => 0x0D, // Channel Pressure (2 bytes)
            0xE0 => 0x0E, // Pitch Bend Change (3 bytes)
            _ => 0x0F,    // Single byte / undefined
        };
        let header = ((cable & 0x0F) << 4) | cin;
        Self([header, status, data1, data2])
    }

    /// Access the underlying 4-byte array.
    pub const fn as_bytes(&self) -> &[u8; 4] {
        &self.0
    }
}

pub trait ToUsbMidi {
    /// Converts the structure into a 4-byte USB-MIDI 1.0 event packet.
    fn to_usb_midi(&self, cable: u8) -> UsbMidiPacket;
}

impl ToUsbMidi for InstrumentEvent {
    fn to_usb_midi(&self, cable: u8) -> UsbMidiPacket {
        let midi = self.to_midi();
        UsbMidiPacket::from_midi(cable, midi.bytes[0], midi.bytes[1], midi.bytes[2])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Channel, U14, U7};

    #[test]
    fn test_note_on_usb_midi() {
        let event = InstrumentEvent::NoteOn {
            channel: Channel::new(1),
            note: U7::new(60), // Middle C
            velocity: U7::new(100),
        };
        let packet = event.to_usb_midi(0);
        // CIN 9 for NoteOn on Cable 0 -> 0x09
        // Status: 0x91 (NoteOn Channel 1)
        // Data1: 60 (0x3C), Data2: 100 (0x64)
        assert_eq!(packet.as_bytes(), &[0x09, 0x91, 0x3C, 0x64]);
    }

    #[test]
    fn test_note_off_usb_midi() {
        let event = InstrumentEvent::NoteOff {
            channel: Channel::new(0),
            note: U7::new(69), // A4
            velocity: U7::new(0),
        };
        let packet = event.to_usb_midi(0);
        // CIN 8 for NoteOff on Cable 0 -> 0x08
        // Status: 0x80 (NoteOff Channel 0)
        // Data1: 69 (0x45), Data2: 0 (0x00)
        assert_eq!(packet.as_bytes(), &[0x08, 0x80, 0x45, 0x00]);
    }

    #[test]
    fn test_pitch_bend_usb_midi() {
        let event = InstrumentEvent::PitchBend {
            channel: Channel::new(0),
            bend: U14::new(8192), // Center bend (0x2000 -> LSB: 0x00, MSB: 0x40)
        };
        let packet = event.to_usb_midi(0);
        // CIN E for PitchBend on Cable 0 -> 0x0E
        // Status: 0xE0
        // Data1 (LSB): 0, Data2 (MSB): 64 (0x40)
        assert_eq!(packet.as_bytes(), &[0x0E, 0xE0, 0x00, 0x40]);
    }

    #[test]
    fn test_channel_pressure_usb_midi() {
        let event = InstrumentEvent::ChannelPressure {
            channel: Channel::new(2),
            pressure: U7::new(80),
        };
        let packet = event.to_usb_midi(0);
        // CIN D for Channel Pressure -> 0x0D
        // Status: 0xD2
        // Data1: 80 (0x50), Data2: 0
        assert_eq!(packet.as_bytes(), &[0x0D, 0xD2, 0x50, 0x00]);
    }

    #[test]
    fn test_timbre_cc74_usb_midi() {
        let event = InstrumentEvent::Timbre {
            channel: Channel::new(0),
            value: U7::new(127),
        };
        let packet = event.to_usb_midi(1); // Test Cable 1
                                           // Cable 1 << 4 | CIN B -> 0x1B
                                           // Status: 0xB0 (CC)
                                           // Data1: 74 (0x4A), Data2: 127 (0x7F)
        assert_eq!(packet.as_bytes(), &[0x1B, 0xB0, 74, 0x7F]);
    }

    #[test]
    fn test_cc7_volume_usb_midi() {
        let packet = UsbMidiPacket::from_midi(0, 0xB0, 7, 100);
        // Cable 0, CIN 0x0B (CC), Status 0xB0, Data1: 7, Data2: 100
        assert_eq!(packet.as_bytes(), &[0x0B, 0xB0, 7, 100]);
    }

    #[test]
    fn test_cc11_expression_usb_midi() {
        let packet = UsbMidiPacket::from_midi(0, 0xB0, 11, 64);
        // Cable 0, CIN 0x0B (CC), Status 0xB0, Data1: 11, Data2: 64
        assert_eq!(packet.as_bytes(), &[0x0B, 0xB0, 11, 64]);
    }

    #[test]
    fn test_pitch_bend_extremes_usb_midi() {
        // Minimum bend (0) -> LSB: 0, MSB: 0
        let min_event = InstrumentEvent::PitchBend {
            channel: Channel::new(0),
            bend: U14::new(0),
        };
        let min_pkt = min_event.to_usb_midi(0);
        assert_eq!(min_pkt.as_bytes(), &[0x0E, 0xE0, 0x00, 0x00]);

        // Maximum bend (16383) -> LSB: 0x7F, MSB: 0x7F
        let max_event = InstrumentEvent::PitchBend {
            channel: Channel::new(0),
            bend: U14::new(16383),
        };
        let max_pkt = max_event.to_usb_midi(0);
        assert_eq!(max_pkt.as_bytes(), &[0x0E, 0xE0, 0x7F, 0x7F]);
    }

    #[test]
    fn test_cable_number_multiplexing() {
        for cable in 0..16 {
            let pkt = UsbMidiPacket::from_midi(cable, 0x90, 60, 100);
            assert_eq!(pkt.as_bytes()[0] >> 4, cable);
            assert_eq!(pkt.as_bytes()[0] & 0x0F, 0x09); // NoteOn CIN
        }
    }

    #[test]
    fn test_channel_pressure_boundaries() {
        let min_press = InstrumentEvent::ChannelPressure {
            channel: Channel::new(3),
            pressure: U7::new(0),
        };
        assert_eq!(
            min_press.to_usb_midi(0).as_bytes(),
            &[0x0D, 0xD3, 0x00, 0x00]
        );

        let max_press = InstrumentEvent::ChannelPressure {
            channel: Channel::new(3),
            pressure: U7::new(127),
        };
        assert_eq!(
            max_press.to_usb_midi(0).as_bytes(),
            &[0x0D, 0xD3, 0x7F, 0x00]
        );
    }
}
