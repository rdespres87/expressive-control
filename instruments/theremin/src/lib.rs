#![no_std]

use expressive_core::{
    Channel, InstrumentEvent, SensorEvent, U7, U14,
    processor::{ControlProcessor, ProcessorError},
};

pub struct TofThereminProcessor {
    pub min_distance_mm: u16,
    pub max_distance_mm: u16,
    pub base_note: U7,
    pub pitch_bend_range_semitones: u8,
    is_playing: bool,   // True if pitch hand is present
    note_on_sent: bool, // True if a NoteOn was sent (volume > 0)
    current_volume: U7,
    current_pitch_bend: U14,
}

impl TofThereminProcessor {
    pub fn current_volume(&self) -> u8 {
        self.current_volume.as_u8()
    }

    pub fn current_pitch_bend(&self) -> u16 {
        self.current_pitch_bend.as_u16()
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing
    }
}

impl Default for TofThereminProcessor {
    fn default() -> Self {
        Self {
            min_distance_mm: 20,
            max_distance_mm: 1000,
            base_note: U7::new(60),        // Middle C
            pitch_bend_range_semitones: 2, // Standard +/- 2 semitones
            is_playing: false,
            note_on_sent: false,
            current_volume: U7::new(0),
            current_pitch_bend: U14::new(8192), // Center pitch bend
        }
    }
}

impl TofThereminProcessor {
    fn map_distance_to_note_and_bend(&self, z: u16) -> (U7, U14) {
        if z >= self.max_distance_mm {
            return (U7::new(60), U14::new(8192));
        }
        if z <= self.min_distance_mm {
            return (U7::new(84), U14::new(8192));
        }

        let mm_from_c4 = (self.max_distance_mm - z) as f32;
        let semitones = mm_from_c4 / 40.83333; // 980mm / 24 semitones

        let rounded_semitones = (semitones + 0.5) as i32;
        let midi_note = 60 + rounded_semitones;

        let remainder = semitones - rounded_semitones as f32;

        // Pitchbend is 8192 at center, +/- 8191 for +/- 2 semitones.
        // 1 semitone = 4095.5 units.
        let bend = 8192.0 + (remainder * 4095.5);
        let bend = bend.clamp(0.0, 16383.0);

        (
            U7::new(midi_note.clamp(0, 127) as u8),
            U14::new(bend as u16),
        )
    }

    fn map_distance_to_u7(&self, z: u16) -> U7 {
        if z <= self.min_distance_mm {
            return U7::new(127); // Max volume
        }
        if z >= self.max_distance_mm {
            return U7::new(0); // Min volume
        }
        let range = self.max_distance_mm - self.min_distance_mm;
        let pos = self.max_distance_mm - z;
        let vol = ((pos as u32 * 127) / range as u32) as u8;
        U7::new(vol)
    }
}

impl ControlProcessor for TofThereminProcessor {
    fn process_event(
        &mut self,
        event: SensorEvent,
        out_buffer: &mut [InstrumentEvent],
    ) -> Result<usize, ProcessorError> {
        match event {
            SensorEvent::Position3D { id, x: _, y: _, z } => {
                if id == 0 {
                    // Right hand (Pitch)
                    let (note, bend) = self.map_distance_to_note_and_bend(z);
                    let mut idx = 0;

                    let prev_note = self.base_note;
                    self.is_playing = true;
                    self.base_note = note;
                    self.current_pitch_bend = bend;

                    if self.current_volume.as_u8() > 0 {
                        if !self.note_on_sent {
                            if out_buffer.len() < idx + 2 {
                                return Err(ProcessorError::BufferTooSmall);
                            }
                            out_buffer[idx] = InstrumentEvent::NoteOn {
                                channel: Channel::new(1),
                                note: self.base_note,
                                velocity: self.current_volume,
                            };
                            idx += 1;
                            self.note_on_sent = true;
                        } else if note != prev_note {
                            if out_buffer.len() < idx + 2 {
                                return Err(ProcessorError::BufferTooSmall);
                            }
                            out_buffer[idx] = InstrumentEvent::NoteOff {
                                channel: Channel::new(1),
                                note: prev_note,
                                velocity: U7::new(0),
                            };
                            idx += 1;
                            out_buffer[idx] = InstrumentEvent::NoteOn {
                                channel: Channel::new(1),
                                note: self.base_note,
                                velocity: self.current_volume,
                            };
                            idx += 1;
                        }

                        if out_buffer.len() < idx + 1 {
                            return Err(ProcessorError::BufferTooSmall);
                        }
                        out_buffer[idx] = InstrumentEvent::PitchBend {
                            channel: Channel::new(1),
                            bend: self.current_pitch_bend,
                        };
                        idx += 1;
                    }
                    Ok(idx)
                } else if id == 1 {
                    // Left hand (Volume)
                    let prev_vol = self.current_volume.as_u8();
                    self.current_volume = self.map_distance_to_u7(z);

                    let mut idx = 0;
                    if self.is_playing {
                        if prev_vol == 0 && self.current_volume.as_u8() > 0 {
                            if out_buffer.len() < idx + 2 {
                                return Err(ProcessorError::BufferTooSmall);
                            }
                            out_buffer[idx] = InstrumentEvent::NoteOn {
                                channel: Channel::new(1),
                                note: self.base_note,
                                velocity: self.current_volume,
                            };
                            idx += 1;
                            out_buffer[idx] = InstrumentEvent::PitchBend {
                                channel: Channel::new(1),
                                bend: self.current_pitch_bend,
                            };
                            idx += 1;
                            self.note_on_sent = true;
                        } else if prev_vol > 0
                            && self.current_volume.as_u8() == 0
                            && self.note_on_sent
                        {
                            if out_buffer.len() < idx + 1 {
                                return Err(ProcessorError::BufferTooSmall);
                            }
                            out_buffer[idx] = InstrumentEvent::NoteOff {
                                channel: Channel::new(1),
                                note: self.base_note,
                                velocity: U7::new(0),
                            };
                            idx += 1;
                            self.note_on_sent = false;
                        }

                        if self.current_volume.as_u8() > 0 {
                            if out_buffer.len() < idx + 1 {
                                return Err(ProcessorError::BufferTooSmall);
                            }
                            out_buffer[idx] = InstrumentEvent::ChannelPressure {
                                channel: Channel::new(1),
                                pressure: self.current_volume,
                            };
                            idx += 1;
                        }
                    }
                    Ok(idx)
                } else {
                    Ok(0)
                }
            }
            SensorEvent::Released { id } => {
                let mut idx = 0;
                if id == 0 && self.is_playing {
                    self.is_playing = false;
                    if self.note_on_sent {
                        if out_buffer.is_empty() {
                            return Err(ProcessorError::BufferTooSmall);
                        }
                        out_buffer[idx] = InstrumentEvent::NoteOff {
                            channel: Channel::new(1),
                            note: self.base_note,
                            velocity: U7::new(0),
                        };
                        idx += 1;
                        self.note_on_sent = false;
                    }
                    Ok(idx)
                } else if id == 1 {
                    self.current_volume = U7::new(0);
                    if self.note_on_sent {
                        if out_buffer.is_empty() {
                            return Err(ProcessorError::BufferTooSmall);
                        }
                        out_buffer[idx] = InstrumentEvent::NoteOff {
                            channel: Channel::new(1),
                            note: self.base_note,
                            velocity: U7::new(0),
                        };
                        idx += 1;
                        self.note_on_sent = false;
                    }
                    Ok(idx)
                } else {
                    Ok(0)
                }
            }
            _ => Ok(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_distance_to_note_and_bend() {
        let processor = TofThereminProcessor::default();

        // 1000mm -> C4 (60), center bend
        let (n, b) = processor.map_distance_to_note_and_bend(1000);
        assert_eq!(n.as_u8(), 60);
        assert_eq!(b.as_u16(), 8192);

        // 918mm -> ~2 semitones -> D4 (62), center bend
        let (n, b) = processor.map_distance_to_note_and_bend(918);
        assert_eq!(n.as_u8(), 62);
        assert!((b.as_u16() as i32 - 8192).abs() < 100);

        // 837mm -> ~4 semitones -> E4 (64), center bend
        let (n, b) = processor.map_distance_to_note_and_bend(837);
        assert_eq!(n.as_u8(), 64);
        assert!((b.as_u16() as i32 - 8192).abs() < 100);

        // 938mm -> ~1.5 semitones -> between Db4 and D4
        // Closer to D4 (62) - 0.5 semitones bend
        let (n, b) = processor.map_distance_to_note_and_bend(938);
        assert_eq!(n.as_u8(), 62); // 1.5 rounds to 2
        assert!(b.as_u16() < 8192);
    }

    #[test]
    fn test_map_distance_to_volume() {
        let processor = TofThereminProcessor::default();
        assert_eq!(processor.map_distance_to_u7(20).as_u8(), 127);
        assert_eq!(processor.map_distance_to_u7(1000).as_u8(), 0);
        assert_eq!(processor.map_distance_to_u7(510).as_u8(), 63);
    }

    #[test]
    fn test_process_event_pitch() {
        let mut processor = TofThereminProcessor::default();
        let mut buffer = [InstrumentEvent::NoteOn {
            channel: Channel::new(1),
            note: U7::new(0),
            velocity: U7::new(0),
        }; 2];

        // 500mm is ~12.2 semitones from 1000mm. Note should be 60 + 12 = 72 (C5).
        let count = processor
            .process_event(
                SensorEvent::Position3D {
                    id: 0,
                    x: 0,
                    y: 0,
                    z: 500,
                },
                &mut buffer,
            )
            .unwrap();
        assert_eq!(count, 2);

        match buffer[0] {
            InstrumentEvent::NoteOn { note, .. } => {
                assert_eq!(note.as_u8(), 72);
            }
            _ => panic!("Expected NoteOn"),
        }
    }
}
