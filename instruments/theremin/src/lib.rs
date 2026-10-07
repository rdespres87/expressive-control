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
    is_playing: bool,
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
            base_note: U7::new(60), // Middle C
            pitch_bend_range_semitones: 24,
            is_playing: false,
            current_volume: U7::new(0),
            current_pitch_bend: U14::new(8192), // Center pitch bend
        }
    }
}

impl TofThereminProcessor {
    fn map_distance_to_u14(&self, z: u16) -> U14 {
        if z <= self.min_distance_mm {
            return U14::new(16383); // Highest pitch
        }
        if z >= self.max_distance_mm {
            return U14::new(8192); // Base note (center bend)
        }
        let range = self.max_distance_mm - self.min_distance_mm;
        let pos = self.max_distance_mm - z; // Closer = higher pos

        let bend = 8192 + ((pos as u32 * 8191) / range as u32) as u16;
        U14::new(bend)
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
                    if !self.is_playing {
                        // Start playing
                        if out_buffer.len() < 2 {
                            return Err(ProcessorError::BufferTooSmall);
                        }
                        self.is_playing = true;
                        self.current_pitch_bend = self.map_distance_to_u14(z);

                        out_buffer[0] = InstrumentEvent::NoteOn {
                            channel: Channel::new(1),
                            note: self.base_note,
                            velocity: self.current_volume,
                        };
                        out_buffer[1] = InstrumentEvent::PitchBend {
                            channel: Channel::new(1),
                            bend: self.current_pitch_bend,
                        };
                        Ok(2)
                    } else {
                        if out_buffer.is_empty() {
                            return Err(ProcessorError::BufferTooSmall);
                        }
                        self.current_pitch_bend = self.map_distance_to_u14(z);
                        out_buffer[0] = InstrumentEvent::PitchBend {
                            channel: Channel::new(1),
                            bend: self.current_pitch_bend,
                        };
                        Ok(1)
                    }
                } else if id == 1 {
                    // Left hand (Volume)
                    if out_buffer.is_empty() {
                        return Err(ProcessorError::BufferTooSmall);
                    }
                    self.current_volume = self.map_distance_to_u7(z);
                    out_buffer[0] = InstrumentEvent::ChannelPressure {
                        channel: Channel::new(1),
                        pressure: self.current_volume,
                    };
                    Ok(1)
                } else {
                    Ok(0)
                }
            }
            SensorEvent::Released { id } => {
                if id == 0 && self.is_playing {
                    if out_buffer.is_empty() {
                        return Err(ProcessorError::BufferTooSmall);
                    }
                    self.is_playing = false;
                    out_buffer[0] = InstrumentEvent::NoteOff {
                        channel: Channel::new(1),
                        note: self.base_note,
                        velocity: U7::new(0),
                    };
                    Ok(1)
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
    fn test_map_distance_to_pitch_bend() {
        let processor = TofThereminProcessor::default();

        // At minimum distance (20mm), pitch bend should be max (16383)
        assert_eq!(processor.map_distance_to_u14(20).as_u16(), 16383);
        assert_eq!(processor.map_distance_to_u14(10).as_u16(), 16383); // Below min

        // At maximum distance (1000mm), pitch bend should be center (8192)
        assert_eq!(processor.map_distance_to_u14(1000).as_u16(), 8192);
        assert_eq!(processor.map_distance_to_u14(1200).as_u16(), 8192); // Above max

        // At midpoint (510mm), pitch bend should be halfway between 8192 and 16383 (~12287)
        let mid = processor.map_distance_to_u14(510).as_u16();
        assert!(mid > 12000 && mid < 12500, "Midpoint bend was {}", mid);
    }

    #[test]
    fn test_map_distance_to_volume() {
        let processor = TofThereminProcessor::default();

        // At minimum distance (20mm), volume should be max (127)
        assert_eq!(processor.map_distance_to_u7(20).as_u8(), 127);
        assert_eq!(processor.map_distance_to_u7(0).as_u8(), 127); // Below min

        // At maximum distance (1000mm), volume should be min (0)
        assert_eq!(processor.map_distance_to_u7(1000).as_u8(), 0);
        assert_eq!(processor.map_distance_to_u7(2000).as_u8(), 0); // Above max

        // At midpoint (510mm), volume should be halfway (63)
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

        // First pitch event (id = 0) triggers NoteOn + PitchBend
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
            InstrumentEvent::NoteOn { note, velocity, .. } => {
                assert_eq!(note.as_u8(), 60);
                assert_eq!(velocity.as_u8(), 0); // Default volume is 0 until left hand moves
            }
            _ => panic!("Expected NoteOn"),
        }

        match buffer[1] {
            InstrumentEvent::PitchBend { bend, .. } => {
                assert!(bend.as_u16() > 8192); // Hand is closer than 1000mm
            }
            _ => panic!("Expected PitchBend"),
        }

        // Second pitch event only triggers PitchBend update
        let count2 = processor
            .process_event(
                SensorEvent::Position3D {
                    id: 0,
                    x: 0,
                    y: 0,
                    z: 200,
                },
                &mut buffer,
            )
            .unwrap();
        assert_eq!(count2, 1);

        match buffer[0] {
            InstrumentEvent::PitchBend { bend, .. } => {
                assert!(bend.as_u16() > 12000); // Hand is much closer now
            }
            _ => panic!("Expected PitchBend"),
        }
    }

    #[test]
    fn test_process_event_release() {
        let mut processor = TofThereminProcessor::default();
        let mut buffer = [InstrumentEvent::NoteOn {
            channel: Channel::new(1),
            note: U7::new(0),
            velocity: U7::new(0),
        }; 2];

        // Start playing
        processor
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
        assert!(processor.is_playing);

        // Release hand
        let count = processor
            .process_event(SensorEvent::Released { id: 0 }, &mut buffer)
            .unwrap();
        assert_eq!(count, 1);
        assert!(!processor.is_playing);

        match buffer[0] {
            InstrumentEvent::NoteOff { note, .. } => {
                assert_eq!(note.as_u8(), 60);
            }
            _ => panic!("Expected NoteOff"),
        }
    }
}
