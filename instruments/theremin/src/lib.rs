#![no_std]

use expressive_core::{
    Channel, InstrumentEvent, SensorEvent, U7, U14,
    processor::{
        ConfigMessage, ControlProcessor, FilterSpeed, ProcessorError, ProcessorMode, RootNote,
        Scale,
    },
};

pub struct TofThereminProcessor {
    pub min_distance_mm: u16,
    pub max_distance_mm: u16,
    pub base_note: U7,
    pub pitch_bend_range_semitones: u8,
    is_playing: bool,   // True if pitch hand is present
    note_on_sent: bool, // True if a NoteOn was sent (volume > 0)
    current_volume: U7,
    current_timbre: U7,
    current_pitch_bend: U14,

    // Config state
    pub root_note: RootNote,
    pub scale: Scale,
    pub base_octave: i32,
    pub octave_range: i32,
    pub pitch_quantization: i32,
    pub filter_speed: FilterSpeed,
    pub processor_mode: ProcessorMode,
}

impl TofThereminProcessor {
    pub fn current_volume(&self) -> u8 {
        self.current_volume.as_u8()
    }

    pub fn current_timbre(&self) -> u8 {
        self.current_timbre.as_u8()
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
            min_distance_mm: 20,           // 2 cm: Treble (aigus)
            max_distance_mm: 600,          // 60 cm: Bass (graves)
            base_note: U7::new(60),        // Middle C
            pitch_bend_range_semitones: 2, // Standard MIDI +/- 2 semitones
            is_playing: false,
            note_on_sent: false,
            current_volume: U7::new(0),
            current_timbre: U7::new(0),
            current_pitch_bend: U14::new(8192), // Center pitch bend
            root_note: RootNote::C,
            scale: Scale::Chromatic,
            base_octave: 4,
            octave_range: 4, // 4 octaves default (4 to 7 octaves)
            pitch_quantization: 50,
            filter_speed: FilterSpeed::Fast,
            processor_mode: ProcessorMode::default(),
        }
    }
}

impl TofThereminProcessor {
    fn map_distance_to_note_and_bend(&self, z: u16) -> (U7, U14) {
        let root_offset = match self.root_note {
            RootNote::C => 0,
            RootNote::Cs => 1,
            RootNote::D => 2,
            RootNote::Ds => 3,
            RootNote::E => 4,
            RootNote::F => 5,
            RootNote::Fs => 6,
            RootNote::G => 7,
            RootNote::Gs => 8,
            RootNote::A => 9,
            RootNote::As => 10,
            RootNote::B => 11,
        };
        // Standard MIDI: C4 is 60. Octave 4 -> (4+1)*12 = 60.
        let base_midi_note = (self.base_octave + 1) * 12 + root_offset;
        let total_semitones = self.octave_range * 12;

        if z >= self.max_distance_mm {
            return (U7::new(base_midi_note.clamp(0, 127) as u8), U14::new(8192));
        }
        if z <= self.min_distance_mm {
            return (
                U7::new((base_midi_note + total_semitones).clamp(0, 127) as u8),
                U14::new(8192),
            );
        }

        let mm_from_base = (self.max_distance_mm - z) as f32;
        let mm_per_semitone =
            (self.max_distance_mm - self.min_distance_mm) as f32 / total_semitones as f32;
        let exact_semitones = mm_from_base / mm_per_semitone;

        let absolute_pitch = base_midi_note as f32 + exact_semitones;
        let semitones_from_prev = absolute_pitch - self.base_note.as_u8() as f32;

        let pb_range_f32 = self.pitch_bend_range_semitones as f32;
        let midi_note = if semitones_from_prev > pb_range_f32 || semitones_from_prev < -pb_range_f32
        {
            // Note is outside the pitch bend range, we must trigger a new note
            if absolute_pitch >= 0.0 {
                (absolute_pitch + 0.5) as i32
            } else {
                (absolute_pitch - 0.5) as i32
            }
        } else {
            // Stay on the same base note and just bend
            self.base_note.as_u8() as i32
        };

        let new_semitones_from_prev = absolute_pitch - midi_note as f32;

        // Pitchbend is 8192 at center, +/- 8191 for +/- PB range.
        let units_per_semitone = 8191.0 / self.pitch_bend_range_semitones as f32;
        let bend = 8192.0 + (new_semitones_from_prev * units_per_semitone);
        let bend = bend.clamp(0.0, 16383.0);

        (
            U7::new(midi_note.clamp(0, 127) as u8),
            U14::new(bend as u16),
        )
    }

    pub fn map_volume_and_timbre(&self, z: u16) -> (U7, U7) {
        let min_d: u16 = 30; // 3 cm: Silence (Volume = 0)
        let vol_max_d: u16 = 280; // 28 cm: Full volume (CC7 = 127)
        let timbre_max_d: u16 = 480; // 48 cm: Max Timbre / Aftertouch (CC74 = 127)

        if z <= min_d {
            (U7::new(0), U7::new(0))
        } else if z <= vol_max_d {
            let range = vol_max_d - min_d;
            let pos = z - min_d;
            let vol = ((pos as u32 * 127) / range as u32) as u8;
            (U7::new(vol.clamp(0, 127)), U7::new(0))
        } else if z <= timbre_max_d {
            let range = timbre_max_d - vol_max_d;
            let pos = z - vol_max_d;
            let timbre = ((pos as u32 * 127) / range as u32) as u8;
            (U7::new(127), U7::new(timbre.clamp(0, 127)))
        } else {
            (U7::new(127), U7::new(127))
        }
    }

    pub fn map_distance_to_u7(&self, z: u16) -> U7 {
        self.map_volume_and_timbre(z).0
    }
}

impl ControlProcessor for TofThereminProcessor {
    fn handle_config(&mut self, config: ConfigMessage) {
        match config {
            ConfigMessage::SetRootNote(r) => self.root_note = r,
            ConfigMessage::SetScale(s) => self.scale = s,
            ConfigMessage::SetBaseOctave(o) => self.base_octave = o,
            ConfigMessage::SetOctaveRange(r) => self.octave_range = r,
            ConfigMessage::SetPitchQuantization(q) => self.pitch_quantization = q,
            ConfigMessage::SetFilterSpeed(s) => self.filter_speed = s,
            ConfigMessage::SetProcessorMode(m) => {
                self.processor_mode = m;
                match m {
                    ProcessorMode::Standard {
                        pitch_bend_range, ..
                    } => self.pitch_bend_range_semitones = pitch_bend_range,
                    ProcessorMode::Mpe {
                        pitch_bend_range, ..
                    } => self.pitch_bend_range_semitones = pitch_bend_range,
                }
            }
        }
    }

    fn process_event<T: expressive_core::telemetry::TelemetryHandler>(
        &mut self,
        event: SensorEvent,
        out_buffer: &mut [InstrumentEvent],
        telemetry: &mut T,
    ) -> Result<usize, ProcessorError> {
        telemetry.handle(expressive_core::telemetry::TelemetryEvent::SensorReceived(
            &event,
        ));

        let channel = match self.processor_mode {
            ProcessorMode::Standard { channel, .. } => channel,
            ProcessorMode::Mpe {
                manager_channel, ..
            } => Channel::new(manager_channel.as_u8() + 1), // Theremin uses first member channel
        };

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
                                channel,
                                note: self.base_note,
                                velocity: self.current_volume,
                            };
                            telemetry.handle(expressive_core::telemetry::TelemetryEvent::InstrumentEventGenerated(&out_buffer[idx]));
                            idx += 1;
                            self.note_on_sent = true;
                        } else if note != prev_note {
                            if out_buffer.len() < idx + 2 {
                                return Err(ProcessorError::BufferTooSmall);
                            }
                            out_buffer[idx] = InstrumentEvent::NoteOff {
                                channel,
                                note: prev_note,
                                velocity: U7::new(0),
                            };
                            telemetry.handle(expressive_core::telemetry::TelemetryEvent::InstrumentEventGenerated(&out_buffer[idx]));
                            idx += 1;
                            out_buffer[idx] = InstrumentEvent::NoteOn {
                                channel,
                                note: self.base_note,
                                velocity: self.current_volume,
                            };
                            telemetry.handle(expressive_core::telemetry::TelemetryEvent::InstrumentEventGenerated(&out_buffer[idx]));
                            idx += 1;
                        }

                        if out_buffer.len() < idx + 1 {
                            return Err(ProcessorError::BufferTooSmall);
                        }
                        out_buffer[idx] = InstrumentEvent::PitchBend {
                            channel,
                            bend: self.current_pitch_bend,
                        };
                        telemetry.handle(
                            expressive_core::telemetry::TelemetryEvent::InstrumentEventGenerated(
                                &out_buffer[idx],
                            ),
                        );
                        idx += 1;
                    }
                    Ok(idx)
                } else if id == 1 {
                    // Left hand (Volume & Timbre / Aftertouch)
                    let prev_vol = self.current_volume.as_u8();
                    let (vol, timbre) = self.map_volume_and_timbre(z);
                    self.current_volume = vol;
                    let prev_timbre = self.current_timbre.as_u8();
                    self.current_timbre = timbre;

                    let mut idx = 0;
                    if self.is_playing {
                        if prev_vol == 0 && self.current_volume.as_u8() > 0 {
                            if out_buffer.len() < idx + 2 {
                                return Err(ProcessorError::BufferTooSmall);
                            }
                            out_buffer[idx] = InstrumentEvent::NoteOn {
                                channel,
                                note: self.base_note,
                                velocity: self.current_volume,
                            };
                            telemetry.handle(expressive_core::telemetry::TelemetryEvent::InstrumentEventGenerated(&out_buffer[idx]));
                            idx += 1;
                            out_buffer[idx] = InstrumentEvent::PitchBend {
                                channel,
                                bend: self.current_pitch_bend,
                            };
                            telemetry.handle(expressive_core::telemetry::TelemetryEvent::InstrumentEventGenerated(&out_buffer[idx]));
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
                                channel,
                                note: self.base_note,
                                velocity: U7::new(0),
                            };
                            telemetry.handle(expressive_core::telemetry::TelemetryEvent::InstrumentEventGenerated(&out_buffer[idx]));
                            idx += 1;
                            self.note_on_sent = false;
                        }

                        // Forward ChannelPressure (Aftertouch) on every update, including 0,
                        // so MPE synths drop to silence at volume 0.
                        if out_buffer.len() < idx + 1 {
                            return Err(ProcessorError::BufferTooSmall);
                        }
                        out_buffer[idx] = InstrumentEvent::ChannelPressure {
                            channel,
                            pressure: self.current_volume,
                        };
                        telemetry.handle(
                            expressive_core::telemetry::TelemetryEvent::InstrumentEventGenerated(
                                &out_buffer[idx],
                            ),
                        );
                        idx += 1;

                        // CC74 (Brightness / Timbre / Filter cutoff above max volume)
                        if self.current_timbre.as_u8() != prev_timbre {
                            if out_buffer.len() < idx + 1 {
                                return Err(ProcessorError::BufferTooSmall);
                            }
                            out_buffer[idx] = InstrumentEvent::Timbre {
                                channel,
                                value: self.current_timbre,
                            };
                            telemetry.handle(expressive_core::telemetry::TelemetryEvent::InstrumentEventGenerated(&out_buffer[idx]));
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
                            channel,
                            note: self.base_note,
                            velocity: U7::new(0),
                        };
                        telemetry.handle(
                            expressive_core::telemetry::TelemetryEvent::InstrumentEventGenerated(
                                &out_buffer[idx],
                            ),
                        );
                        idx += 1;
                        self.note_on_sent = false;
                    }
                    Ok(idx)
                } else if id == 1 {
                    self.current_volume = U7::new(0);
                    self.current_timbre = U7::new(0);
                    if self.note_on_sent {
                        if out_buffer.is_empty() {
                            return Err(ProcessorError::BufferTooSmall);
                        }
                        out_buffer[idx] = InstrumentEvent::NoteOff {
                            channel,
                            note: self.base_note,
                            velocity: U7::new(0),
                        };
                        telemetry.handle(
                            expressive_core::telemetry::TelemetryEvent::InstrumentEventGenerated(
                                &out_buffer[idx],
                            ),
                        );
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
        let mut processor = TofThereminProcessor::default();
        processor.pitch_bend_range_semitones = 1;

        // 600mm (or above) -> Base note C4 (60), center bend
        let (n, b) = processor.map_distance_to_note_and_bend(600);
        assert_eq!(n.as_u8(), 60);
        assert_eq!(b.as_u16(), 8192);

        // 20mm (or below) -> C4 + 4 octaves (48 semitones) = C8 (108), center bend
        let (n, b) = processor.map_distance_to_note_and_bend(20);
        assert_eq!(n.as_u8(), 108);
        assert_eq!(b.as_u16(), 8192);

        // mm_per_semitone = (600 - 20) / 48 = 12.083 mm/semitone
        // 600 - (2 * 12.083) = ~576mm -> D4 (62)
        let (n, b) = processor.map_distance_to_note_and_bend(576);
        assert_eq!(n.as_u8(), 62);
        assert!((b.as_u16() as i32 - 8192).abs() < 300);

        // 600 - (4 * 12.083) = ~552mm -> E4 (64)
        let (n, b) = processor.map_distance_to_note_and_bend(552);
        assert_eq!(n.as_u8(), 64);
        assert!((b.as_u16() as i32 - 8192).abs() < 300);
    }

    #[test]
    fn test_map_distance_to_volume_and_timbre() {
        let processor = TofThereminProcessor::default();
        // Distance <= 30mm: Volume = 0, Timbre = 0
        let (v, t) = processor.map_volume_and_timbre(20);
        assert_eq!(v.as_u8(), 0);
        assert_eq!(t.as_u8(), 0);
        let (v, t) = processor.map_volume_and_timbre(30);
        assert_eq!(v.as_u8(), 0);
        assert_eq!(t.as_u8(), 0);

        // Mid volume: 30mm + 125mm = 155mm -> ~63 volume, 0 timbre
        let (v, t) = processor.map_volume_and_timbre(155);
        assert_eq!(v.as_u8(), 63);
        assert_eq!(t.as_u8(), 0);

        // Max volume threshold (280mm): Volume = 127, Timbre = 0
        let (v, t) = processor.map_volume_and_timbre(280);
        assert_eq!(v.as_u8(), 127);
        assert_eq!(t.as_u8(), 0);

        // Above max volume: Aftertouch zone! (280mm + 100mm = 380mm) -> Volume = 127, Timbre ~63
        let (v, t) = processor.map_volume_and_timbre(380);
        assert_eq!(v.as_u8(), 127);
        assert_eq!(t.as_u8(), 63);

        // Max timbre (480mm and above): Volume = 127, Timbre = 127
        let (v, t) = processor.map_volume_and_timbre(480);
        assert_eq!(v.as_u8(), 127);
        assert_eq!(t.as_u8(), 127);
    }

    #[test]
    fn test_process_event_pitch() {
        let mut processor = TofThereminProcessor::default();
        let mut buffer = [InstrumentEvent::NoteOn {
            channel: Channel::new(1),
            note: U7::new(0),
            velocity: U7::new(0),
        }; 4];
        let mut t = expressive_core::telemetry::NoopTelemetry;

        // Set volume hand at 280mm (Full Volume)
        processor
            .process_event(
                SensorEvent::Position3D {
                    id: 1,
                    x: 0,
                    y: 0,
                    z: 280,
                },
                &mut buffer,
                &mut t,
            )
            .unwrap();

        // 455mm is 145mm from 600mm. 145 / 12.083 = 12 semitones -> Note should be 60 + 12 = 72 (C5).
        let count = processor
            .process_event(
                SensorEvent::Position3D {
                    id: 0,
                    x: 0,
                    y: 0,
                    z: 455,
                },
                &mut buffer,
                &mut t,
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

    #[test]
    fn test_mute_behavior_sends_note_off() {
        let mut processor = TofThereminProcessor::default();
        let mut buffer = [InstrumentEvent::NoteOff {
            channel: Channel::new(0),
            note: U7::new(0),
            velocity: U7::new(0),
        }; 4];
        let mut t = expressive_core::telemetry::NoopTelemetry;

        // 1. Establish pitch hand at 500mm and volume hand at 400mm (full volume = 127)
        processor
            .process_event(
                SensorEvent::Position3D {
                    id: 0,
                    x: 0,
                    y: 0,
                    z: 500,
                },
                &mut buffer,
                &mut t,
            )
            .unwrap();
        let count = processor
            .process_event(
                SensorEvent::Position3D {
                    id: 1,
                    x: 0,
                    y: 0,
                    z: 400,
                },
                &mut buffer,
                &mut t,
            )
            .unwrap();
        assert!(count >= 1);
        assert!(processor.is_playing());
        assert_eq!(processor.current_volume(), 127);

        // 2. Mute: hand moves close to antenna <= 30mm (silence)
        let count = processor
            .process_event(
                SensorEvent::Position3D {
                    id: 1,
                    x: 0,
                    y: 0,
                    z: 20,
                },
                &mut buffer,
                &mut t,
            )
            .unwrap();
        assert!(count >= 1);
        match buffer[0] {
            InstrumentEvent::NoteOff { velocity, .. } => {
                assert_eq!(velocity.as_u8(), 0);
            }
            _ => panic!("Expected NoteOff upon volume reaching 0"),
        }
        assert_eq!(processor.current_volume(), 0);

        // 3. Pitch changes while muted: no new NoteOn should be emitted
        let count = processor
            .process_event(
                SensorEvent::Position3D {
                    id: 0,
                    x: 0,
                    y: 0,
                    z: 600,
                },
                &mut buffer,
                &mut t,
            )
            .unwrap();
        assert_eq!(count, 0);

        // 4. Un-mute: hand returns to 200mm -> NoteOn must be re-triggered
        let count = processor
            .process_event(
                SensorEvent::Position3D {
                    id: 1,
                    x: 0,
                    y: 0,
                    z: 200,
                },
                &mut buffer,
                &mut t,
            )
            .unwrap();
        assert!(count >= 1);
        match buffer[0] {
            InstrumentEvent::NoteOn { velocity, .. } => {
                assert!(velocity.as_u8() > 0);
            }
            _ => panic!("Expected NoteOn upon unmuting"),
        }
    }

    #[test]
    fn test_sensor_event_released_volume_and_pitch() {
        let mut processor = TofThereminProcessor::default();
        let mut buffer = [InstrumentEvent::NoteOff {
            channel: Channel::new(0),
            note: U7::new(0),
            velocity: U7::new(0),
        }; 4];
        let mut t = expressive_core::telemetry::NoopTelemetry;

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
                &mut t,
            )
            .unwrap();
        processor
            .process_event(
                SensorEvent::Position3D {
                    id: 1,
                    x: 0,
                    y: 0,
                    z: 100,
                },
                &mut buffer,
                &mut t,
            )
            .unwrap();
        assert!(processor.current_volume() > 0);

        // Release volume hand -> sends NoteOff
        let count = processor
            .process_event(SensorEvent::Released { id: 1 }, &mut buffer, &mut t)
            .unwrap();
        assert_eq!(count, 1);
        assert!(matches!(buffer[0], InstrumentEvent::NoteOff { .. }));
        assert_eq!(processor.current_volume(), 0);

        // Release pitch hand -> marks is_playing false
        let _ = processor
            .process_event(SensorEvent::Released { id: 0 }, &mut buffer, &mut t)
            .unwrap();
        assert!(!processor.is_playing());
    }

    #[test]
    fn test_configuration_updates() {
        let mut processor = TofThereminProcessor::default();

        // Base octave
        processor.handle_config(ConfigMessage::SetBaseOctave(5));
        assert_eq!(processor.base_octave, 5);
        let (n, _) = processor.map_distance_to_note_and_bend(1000);
        assert_eq!(n.as_u8(), 72);

        // Scale and root note
        processor.handle_config(ConfigMessage::SetScale(Scale::Minor));
        assert_eq!(processor.scale, Scale::Minor);
        processor.handle_config(ConfigMessage::SetRootNote(RootNote::A));
        assert_eq!(processor.root_note, RootNote::A);

        // Quantization and filter
        processor.handle_config(ConfigMessage::SetPitchQuantization(85));
        assert_eq!(processor.pitch_quantization, 85);
        processor.handle_config(ConfigMessage::SetFilterSpeed(FilterSpeed::Slow));
        assert_eq!(processor.filter_speed, FilterSpeed::Slow);
    }

    #[test]
    fn test_buffer_too_small_returns_error() {
        let mut processor = TofThereminProcessor::default();
        let mut tiny_buffer = [];
        let mut t = expressive_core::telemetry::NoopTelemetry;

        let result = processor.process_event(
            SensorEvent::Position3D {
                id: 0,
                x: 0,
                y: 0,
                z: 500,
            },
            &mut tiny_buffer,
            &mut t,
        );
        // id=0 without volume does not write, but when volume > 0, it requires buffer
        assert!(result.is_ok());

        // Now set volume > 0
        processor.current_volume = U7::new(100);
        let result = processor.process_event(
            SensorEvent::Position3D {
                id: 0,
                x: 0,
                y: 0,
                z: 500,
            },
            &mut tiny_buffer,
            &mut t,
        );
        assert_eq!(
            result,
            Err(expressive_core::processor::ProcessorError::BufferTooSmall)
        );
    }

    #[test]
    fn test_release_emits_note_off_on_configured_channel() {
        let mut processor = TofThereminProcessor::default();
        let target_ch = Channel::new(5);
        processor.processor_mode = ProcessorMode::Standard {
            channel: target_ch,
            pitch_bend_range: 2,
        };

        let mut buffer = [InstrumentEvent::NoteOff {
            channel: Channel::new(0),
            note: U7::new(0),
            velocity: U7::new(0),
        }; 4];
        let mut t = expressive_core::telemetry::NoopTelemetry;

        // Establish pitch and volume
        processor
            .process_event(
                SensorEvent::Position3D {
                    id: 0,
                    x: 0,
                    y: 0,
                    z: 500,
                },
                &mut buffer,
                &mut t,
            )
            .unwrap();
        processor
            .process_event(
                SensorEvent::Position3D {
                    id: 1,
                    x: 0,
                    y: 0,
                    z: 280,
                },
                &mut buffer,
                &mut t,
            )
            .unwrap();

        // Release pitch hand (id 0) -> NoteOff must be on target_ch (5)
        let count = processor
            .process_event(SensorEvent::Released { id: 0 }, &mut buffer, &mut t)
            .unwrap();
        assert_eq!(count, 1);
        match buffer[0] {
            InstrumentEvent::NoteOff { channel, .. } => {
                assert_eq!(channel, target_ch);
            }
            _ => panic!("Expected NoteOff"),
        }
    }
}
