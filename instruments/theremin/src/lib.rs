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
